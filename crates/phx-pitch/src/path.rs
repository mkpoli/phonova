use crate::candidates::FrameCandidates;
use crate::params::PitchParams;
use crate::types::{PitchFrame, PitchTrack};

pub(crate) fn viterbi_track(frames: Vec<FrameCandidates>, params: &PitchParams) -> PitchTrack {
    if frames.is_empty() {
        return PitchTrack::new(Vec::new());
    }

    let first_costs = frames[0]
        .candidates
        .iter()
        .map(|candidate| -candidate.strength)
        .collect();
    let mut costs: Vec<Vec<f64>> = vec![first_costs];
    let mut backpointers: Vec<Vec<usize>> = Vec::with_capacity(frames.len().saturating_sub(1));

    for frame_index in 1..frames.len() {
        let previous = &frames[frame_index - 1].candidates;
        let current = &frames[frame_index].candidates;
        let mut frame_costs = vec![f64::INFINITY; current.len()];
        let mut frame_backpointers = vec![0; current.len()];

        for (current_index, current_candidate) in current.iter().enumerate() {
            for (previous_index, previous_candidate) in previous.iter().enumerate() {
                let cost = costs[frame_index - 1][previous_index]
                    + transition_cost(
                        previous_candidate.frequency,
                        current_candidate.frequency,
                        params,
                    )
                    - current_candidate.strength;
                if cost < frame_costs[current_index] {
                    frame_costs[current_index] = cost;
                    frame_backpointers[current_index] = previous_index;
                }
            }
        }

        costs.push(frame_costs);
        backpointers.push(frame_backpointers);
    }

    let mut chosen = vec![0; frames.len()];
    chosen[frames.len() - 1] = costs[frames.len() - 1]
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
        .unwrap_or(0);

    for frame_index in (1..frames.len()).rev() {
        chosen[frame_index - 1] = backpointers[frame_index - 1][chosen[frame_index]];
    }

    let pitch_frames = frames
        .into_iter()
        .zip(chosen)
        .map(|(frame, selected)| {
            let candidate = &frame.candidates[selected];
            PitchFrame {
                time: frame.time,
                f0: (candidate.frequency > 0.0).then_some(candidate.frequency),
                strength: candidate.strength,
                candidates: frame.candidates,
            }
        })
        .collect();

    PitchTrack::new(pitch_frames)
}

pub(crate) fn transition_cost(f1: f64, f2: f64, params: &PitchParams) -> f64 {
    match (f1 == 0.0, f2 == 0.0) {
        (true, true) => 0.0,
        (true, false) | (false, true) => params.voiced_unvoiced_cost,
        (false, false) => params.octave_jump_cost * (f1 / f2).log2().abs(),
    }
}
