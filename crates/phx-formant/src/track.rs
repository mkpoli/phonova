use phx_audio::{Audio, AudioView, ResampleQuality};
use phx_dsp::{FrameGrid, Window, preemphasis_in_place, window_samples};

use crate::burg::burg_lpc;
use crate::params::{frame_grid, validate_params};
use crate::roots::lpc_roots_to_formants;
use crate::{FormantFrame, FormantParams, FormantPoint, FormantTrack};

const DEFAULT_BANDWIDTH_WEIGHT: f64 = 1.0e-6;
const DEFAULT_FREQUENCY_WEIGHT: f64 = 1.0;
const DEFAULT_TRANSITION_WEIGHT: f64 = 1.0e-6;

/// Neutral-reference and cost weights for formant Viterbi tracking.
///
/// Xia & Espy-Wilson (2000) define local bandwidth, neutral-reference, and
/// transition costs. The cited excerpt gives neutral references for F1-F4 as
/// 500, 1500, 2500, and 3500 Hz, while numeric weights are tuned empirically
/// without listed values. The default weights here are crate constants chosen
/// to put typical bandwidth and transition penalties on a comparable scale to
/// normalized neutral-frequency deviation. The cited sources give values
/// through F4; higher slots extend the sequence by 1000 Hz as a crate
/// implementation choice.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackingRefs {
    /// Neutral formant frequencies in hertz, starting at F1.
    pub neutral_hz: Vec<f64>,
    /// Weight for each `bandwidth²` local-cost term.
    pub bandwidth_weight: f64,
    /// Weight for normalized neutral-frequency deviation.
    pub frequency_weight: f64,
    /// Weight for squared frame-to-frame frequency changes.
    pub transition_weight: f64,
}

impl Default for TrackingRefs {
    fn default() -> Self {
        Self {
            neutral_hz: vec![500.0, 1500.0, 2500.0, 3500.0],
            bandwidth_weight: DEFAULT_BANDWIDTH_WEIGHT,
            frequency_weight: DEFAULT_FREQUENCY_WEIGHT,
            transition_weight: DEFAULT_TRANSITION_WEIGHT,
        }
    }
}

impl TrackingRefs {
    /// Returns the neutral frequency for a zero-based formant slot.
    ///
    /// Values above the supplied reference list continue with 1000 Hz spacing,
    /// matching the F1-F4 spacing in the cited Xia & Espy-Wilson references.
    #[must_use]
    pub fn neutral_for_slot(&self, slot: usize) -> f64 {
        if let Some(&neutral) = self.neutral_hz.get(slot) {
            return neutral;
        }
        let last = self.neutral_hz.last().copied().unwrap_or(3500.0);
        last + 1000.0 * (slot + 1 - self.neutral_hz.len()) as f64
    }
}

/// Computes Burg LPC formant candidates for an audio view.
///
/// The input is mixed to mono, resampled to `2 * params.ceiling_hz`, framed on
/// [`FrameGrid`] with the physical Gaussian window length, pre-emphasized,
/// analysed with Burg LPC order `2 * params.max_formants`, and converted from
/// LPC roots to frequency-bandwidth candidates. Each frame retains the
/// candidates surviving the 50 Hz and `ceiling_hz - 50 Hz` gate, sorted by
/// frequency and capped at `params.max_formants`.
#[must_use]
pub fn formant_track(audio: AudioView<'_>, params: &FormantParams) -> FormantTrack {
    validate_params(params);

    let mono = audio.mono_mix();
    let owned = Audio::new(vec![mono.iter().copied().collect()], audio.sample_rate())
        .expect("AudioView mono mix has a valid sample rate and channel length");
    let target_hz = 2.0 * params.ceiling_hz;
    let resampled = owned
        .resampled(target_hz, ResampleQuality::Best)
        .expect("resampling to the finite positive formant ceiling rate should succeed");
    let mut samples = resampled
        .mono_mix()
        .iter()
        .map(|&sample| f64::from(sample))
        .collect::<Vec<_>>();
    preemphasis_in_place(
        &mut samples,
        params.preemphasis_from_hz,
        resampled.sample_rate(),
    );

    // Frame count follows the resampled signal Praat actually analyses: its
    // sample count times its sampling period, matching Praat's `dx * nx`
    // discrete duration rather than a rounded `duration()` quotient.
    let grid_duration = samples.len() as f64 * (1.0 / resampled.sample_rate());
    let analysis = Analysis::new(resampled.sample_rate(), grid_duration, params);
    let frames = analysis
        .grid
        .centers()
        .map(|time| FormantFrame {
            time,
            formants: analysis.frame_formants(&samples, time),
        })
        .collect();

    FormantTrack {
        frames,
        params: *params,
        duration: audio.duration(),
        frame_grid: analysis.grid,
    }
}

/// Smooths raw formant candidates with Xia & Espy-Wilson-style Viterbi costs.
///
/// The tracker assigns each frame's sorted candidate list to increasing formant
/// slots, allowing empty slots when fewer roots survive the LPC frequency gate.
/// Local costs combine bandwidth and neutral-reference deviation; transition
/// costs penalize squared frequency changes for slots present in consecutive
/// frames.
#[must_use]
pub fn track_smoothed(raw: &FormantTrack, refs: &TrackingRefs) -> FormantTrack {
    validate_tracking_refs(refs);
    let slots = raw.params.max_formants;
    if raw.frames.is_empty() || slots == 0 {
        return raw.clone();
    }

    let states_by_frame = raw
        .frames
        .iter()
        .map(|frame| states_for_frame(&frame.formants, slots))
        .collect::<Vec<_>>();

    if states_by_frame.iter().any(Vec::is_empty) {
        return FormantTrack {
            frames: raw
                .frames
                .iter()
                .map(|frame| FormantFrame {
                    time: frame.time,
                    formants: Vec::new(),
                })
                .collect(),
            params: raw.params,
            duration: raw.duration,
            frame_grid: raw.frame_grid,
        };
    }

    let mut costs = Vec::with_capacity(states_by_frame.len());
    let mut backpointers = Vec::with_capacity(states_by_frame.len());

    for (frame_index, states) in states_by_frame.iter().enumerate() {
        let mut frame_costs = vec![f64::INFINITY; states.len()];
        let mut frame_back = vec![0usize; states.len()];
        for (state_index, state) in states.iter().enumerate() {
            let local = local_cost(state, refs);
            if frame_index == 0 {
                frame_costs[state_index] = local;
                continue;
            }

            let previous_costs: &Vec<f64> = &costs[frame_index - 1];
            let previous_states = &states_by_frame[frame_index - 1];
            let (best_previous, best_cost) = previous_states
                .iter()
                .enumerate()
                .map(|(previous_index, previous)| {
                    (
                        previous_index,
                        previous_costs[previous_index] + transition_cost(state, previous, refs),
                    )
                })
                .min_by(|left, right| left.1.total_cmp(&right.1))
                .expect("previous frame has at least one state");
            frame_costs[state_index] = local + best_cost;
            frame_back[state_index] = best_previous;
        }
        costs.push(frame_costs);
        backpointers.push(frame_back);
    }

    let mut state_indices = vec![0usize; states_by_frame.len()];
    state_indices[states_by_frame.len() - 1] = costs
        .last()
        .and_then(|frame_costs| {
            frame_costs
                .iter()
                .enumerate()
                .min_by(|left, right| left.1.total_cmp(right.1))
                .map(|(index, _)| index)
        })
        .unwrap_or(0);

    for frame_index in (1..states_by_frame.len()).rev() {
        state_indices[frame_index - 1] = backpointers[frame_index][state_indices[frame_index]];
    }

    let frames = raw
        .frames
        .iter()
        .enumerate()
        .map(|(frame_index, frame)| {
            let state = &states_by_frame[frame_index][state_indices[frame_index]];
            FormantFrame {
                time: frame.time,
                formants: state.iter().filter_map(|candidate| *candidate).collect(),
            }
        })
        .collect();

    FormantTrack {
        frames,
        params: raw.params,
        duration: raw.duration,
        frame_grid: raw.frame_grid,
    }
}

struct Analysis {
    sample_rate: f64,
    params: FormantParams,
    grid: FrameGrid,
    window: Vec<f64>,
}

impl Analysis {
    fn new(sample_rate: f64, duration: f64, params: &FormantParams) -> Self {
        let physical_window = params.window_length * 2.0;
        let window_len = ((physical_window * sample_rate).round() as usize).saturating_add(1);
        Self {
            sample_rate,
            params: *params,
            grid: frame_grid(duration, params),
            window: window_samples(
                Window::Gaussian {
                    effective_len_factor: 2.0,
                },
                window_len.max(1),
            ),
        }
    }

    fn frame_formants(&self, samples: &[f64], time: f64) -> Vec<FormantPoint> {
        if self.window.is_empty() {
            return Vec::new();
        }

        let mut frame = vec![0.0; self.window.len()];
        let midpoint = (self.window.len().saturating_sub(1)) as f64 / 2.0;
        let center_sample = time * self.sample_rate;
        for (i, (&weight, dst)) in self.window.iter().zip(frame.iter_mut()).enumerate() {
            let sample_index = (center_sample + i as f64 - midpoint).round();
            if sample_index >= 0.0 {
                let sample_index = sample_index as usize;
                if let Some(&sample) = samples.get(sample_index) {
                    *dst = sample * weight;
                }
            }
        }

        let order = 2 * self.params.max_formants;
        let Some(coeffs) = burg_lpc(&frame, order) else {
            return Vec::new();
        };
        let mut formants = lpc_roots_to_formants(&coeffs, self.sample_rate, self.params.ceiling_hz);
        formants.truncate(self.params.max_formants);
        formants
    }
}

type TrackState = Vec<Option<FormantPoint>>;

fn states_for_frame(candidates: &[FormantPoint], slots: usize) -> Vec<TrackState> {
    let candidate_count = candidates.len().min(slots);
    if candidate_count == 0 {
        return vec![vec![None; slots]];
    }

    let mut states = Vec::new();
    let mut state = vec![None; slots];
    assign_candidates(candidates, 0, 0, candidate_count, &mut state, &mut states);
    states
}

fn assign_candidates(
    candidates: &[FormantPoint],
    candidate_index: usize,
    min_slot: usize,
    candidate_count: usize,
    state: &mut TrackState,
    states: &mut Vec<TrackState>,
) {
    if candidate_index == candidate_count {
        states.push(state.clone());
        return;
    }

    let remaining = candidate_count - candidate_index - 1;
    let max_slot = state.len() - remaining;
    for slot in min_slot..max_slot {
        state[slot] = Some(candidates[candidate_index]);
        assign_candidates(
            candidates,
            candidate_index + 1,
            slot + 1,
            candidate_count,
            state,
            states,
        );
        state[slot] = None;
    }
}

fn local_cost(state: &TrackState, refs: &TrackingRefs) -> f64 {
    state
        .iter()
        .enumerate()
        .filter_map(|(slot, candidate)| candidate.map(|candidate| (slot, candidate)))
        .map(|(slot, candidate)| {
            let neutral = refs.neutral_for_slot(slot);
            refs.bandwidth_weight * candidate.bandwidth * candidate.bandwidth
                + refs.frequency_weight * (candidate.frequency - neutral).abs() / neutral
        })
        .sum()
}

fn transition_cost(current: &TrackState, previous: &TrackState, refs: &TrackingRefs) -> f64 {
    current
        .iter()
        .zip(previous)
        .filter_map(|(current, previous)| current.zip(*previous))
        .map(|(current, previous)| {
            let delta = current.frequency - previous.frequency;
            refs.transition_weight * delta * delta
        })
        .sum()
}

fn validate_tracking_refs(refs: &TrackingRefs) {
    assert!(
        refs.neutral_hz
            .iter()
            .all(|value| value.is_finite() && *value > 0.0),
        "neutral_hz values must be finite and positive"
    );
    assert!(
        refs.bandwidth_weight.is_finite() && refs.bandwidth_weight >= 0.0,
        "bandwidth_weight must be finite and non-negative"
    );
    assert!(
        refs.frequency_weight.is_finite() && refs.frequency_weight >= 0.0,
        "frequency_weight must be finite and non-negative"
    );
    assert!(
        refs.transition_weight.is_finite() && refs.transition_weight >= 0.0,
        "transition_weight must be finite and non-negative"
    );
}

#[cfg(test)]
#[must_use]
pub(crate) fn grid_for_params(duration: f64, params: &FormantParams) -> FrameGrid {
    frame_grid(duration, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_neutral_slots_extend_by_one_kilohertz() {
        let refs = TrackingRefs::default();
        assert_eq!(refs.neutral_for_slot(0), 500.0);
        assert_eq!(refs.neutral_for_slot(4), 4500.0);
        assert_eq!(refs.neutral_for_slot(6), 6500.0);
    }

    #[test]
    fn states_place_missing_candidate_in_best_slot() {
        let raw = FormantTrack {
            frames: vec![FormantFrame {
                time: 0.0,
                formants: vec![FormantPoint {
                    frequency: 1500.0,
                    bandwidth: 90.0,
                }],
            }],
            params: FormantParams {
                max_formants: 3,
                ..FormantParams::default()
            },
            duration: 0.025,
            frame_grid: grid_for_params(0.025, &FormantParams::default()),
        };
        let smoothed = track_smoothed(&raw, &TrackingRefs::default());
        assert_eq!(smoothed.frames[0].formants[0].frequency, 1500.0);
    }
}
