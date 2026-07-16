use phx_dsp::{RealFftPlan, Window, sinc_interpolate_max, window_samples};

use crate::acf::{WINDOW_ACF_EPSILON, corrected_autocorrelation, window_autocorrelation};
use crate::params::PitchParams;
use crate::types::PitchCandidate;

pub(crate) const SINC_DEPTH: usize = 30;

#[derive(Debug, Clone)]
pub(crate) struct FrameCandidates {
    pub(crate) time: f64,
    pub(crate) candidates: Vec<PitchCandidate>,
}

pub(crate) struct CandidateContext<'a> {
    pub(crate) signal: &'a [f64],
    pub(crate) sample_rate: f64,
    pub(crate) params: &'a PitchParams,
    pub(crate) physical_window_seconds: f64,
    pub(crate) window: &'a [f64],
    pub(crate) global_peak: f64,
}

pub(crate) fn analysis_window(params: &PitchParams, sample_rate: f64) -> (f64, Vec<f64>) {
    let seconds = params.window_seconds();
    let samples = (seconds * sample_rate).round().max(1.0) as usize;
    let shape = if params.very_accurate {
        // §1.2 postscript is represented with the workspace's centred Gaussian
        // window convention and its standard effective-length factor.
        Window::Gaussian {
            effective_len_factor: 2.0,
        }
    } else {
        Window::Hanning
    };
    (seconds, window_samples(shape, samples))
}

impl CandidateContext<'_> {
    pub(crate) fn candidates_for_frame(
        &self,
        time: f64,
        plan: &mut RealFftPlan,
    ) -> FrameCandidates {
        let segment = centered_segment(self.signal, time, self.sample_rate, self.window.len());
        let local_peak = segment
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0, f64::max);
        let mut candidates = vec![PitchCandidate {
            frequency: 0.0,
            strength: unvoiced_strength(self.params, local_peak, self.global_peak),
        }];

        if self.params.max_candidates > 1 {
            let rx = corrected_autocorrelation(
                &segment,
                self.window,
                self.physical_window_seconds,
                self.sample_rate,
                plan,
            );
            let mut voiced = voiced_candidates(
                &rx,
                self.sample_rate,
                self.params,
                self.physical_window_seconds,
                self.params.max_candidates - 1,
            );
            voiced.sort_by(|a, b| b.strength.total_cmp(&a.strength));
            voiced.truncate(self.params.max_candidates - 1);
            candidates.extend(voiced);
        }

        FrameCandidates { time, candidates }
    }
}

pub(crate) fn centered_segment(
    signal: &[f64],
    center_seconds: f64,
    sample_rate: f64,
    len: usize,
) -> Vec<f64> {
    if len == 0 {
        return Vec::new();
    }
    let half_span = (len.saturating_sub(1)) as f64 * 0.5;
    let rounded_start = (center_seconds * sample_rate - half_span).round();
    let max_start = signal.len().saturating_sub(len);
    let start = if rounded_start.is_sign_negative() {
        0
    } else {
        (rounded_start as usize).min(max_start)
    };
    signal[start..start + len].to_vec()
}

pub(crate) fn unvoiced_strength(params: &PitchParams, local_peak: f64, global_peak: f64) -> f64 {
    let denominator = params.silence_threshold / (1.0 + params.voicing_threshold);
    let level = if global_peak > 0.0 && denominator > 0.0 {
        (local_peak / global_peak) / denominator
    } else {
        0.0
    };
    params.voicing_threshold + (2.0 - level).max(0.0)
}

pub(crate) fn voiced_strength(params: &PitchParams, rx_value: f64, lag_seconds: f64) -> f64 {
    rx_value - params.octave_cost * (params.floor_hz * lag_seconds).log2()
}

fn voiced_candidates(
    rx: &[f64],
    sample_rate: f64,
    params: &PitchParams,
    physical_window_seconds: f64,
    max_voiced: usize,
) -> Vec<PitchCandidate> {
    if rx.len() < 3 || max_voiced == 0 {
        return Vec::new();
    }
    let min_lag = (sample_rate / params.ceiling_hz).ceil().max(1.0) as usize;
    let max_lag = (sample_rate / params.floor_hz).floor() as usize;
    let last = rx.len().saturating_sub(2);
    let start = min_lag.max(1);
    let end = max_lag.min(last);
    if start > end {
        return Vec::new();
    }

    // Rank the integer-lag ACF maxima by their unrefined strength and refine
    // only the strongest `max_voiced`. A transient or near-silent frame can
    // raise hundreds of low ripples, and depth-30 sinc refinement per maximum
    // dominates the whole analysis; scoring the raw sample first keeps the cost
    // proportional to the candidates the frame can actually retain. The raw
    // score uses the same eq. 24 form (`voiced_strength`) that ranks the
    // refined candidates, so on any frame with at most `max_voiced` maxima the
    // retained set is identical to refining every maximum. Sub-sample position
    // and height still come from the sinc interpolation for each kept lag.
    let mut ranked: Vec<(usize, f64)> = Vec::new();
    for lag in start..=end {
        if !(rx[lag - 1] < rx[lag] && rx[lag] > rx[lag + 1]) {
            continue;
        }
        let rw = window_autocorrelation(lag as f64 / sample_rate, physical_window_seconds);
        // §1.3 candidate search guards eq. 9 against near-zero window ACF values.
        if !rw.is_finite() || rw.abs() < WINDOW_ACF_EPSILON {
            continue;
        }
        ranked.push((
            lag,
            voiced_strength(params, rx[lag], lag as f64 / sample_rate),
        ));
    }

    if ranked.len() > max_voiced {
        ranked.select_nth_unstable_by(max_voiced, |a, b| b.1.total_cmp(&a.1));
        ranked.truncate(max_voiced);
    }

    let mut out = Vec::with_capacity(ranked.len());
    for (lag, _) in ranked {
        // §1.2 eq. 22 uses windowed-sinc lag interpolation; depth 30 balances
        // speed and sub-percent peak accuracy.
        let (position, value) = sinc_interpolate_max(rx, lag, SINC_DEPTH);
        if !(position.is_finite() && value.is_finite() && position > 0.0) {
            continue;
        }
        let lag_seconds = position / sample_rate;
        let frequency = 1.0 / lag_seconds;
        if frequency < params.floor_hz || frequency > params.ceiling_hz {
            continue;
        }
        out.push(PitchCandidate {
            frequency,
            strength: voiced_strength(params, value, lag_seconds),
        });
    }
    out
}
