//! Window-corrected autocorrelation candidates + Viterbi path finder
//! (Boersma 1993); full parameter surface with Praat-documented defaults.
#![warn(missing_docs)]

mod acf;
mod candidates;
mod params;
mod path;
mod types;

use phx_audio::AudioView;
use phx_dsp::{FrameGrid, RealFftPlan};

pub use params::PitchParams;
pub use types::{PitchCandidate, PitchFrame, PitchTrack, TimeSpan};

/// Computes a window-corrected autocorrelation pitch track.
#[must_use]
pub fn pitch_track(audio: AudioView<'_>, params: &PitchParams) -> PitchTrack {
    if !params.is_valid_for_analysis() {
        return PitchTrack::new(Vec::new());
    }

    let sample_rate = audio.sample_rate();
    let Some(step) = params.resolved_step() else {
        return PitchTrack::new(Vec::new());
    };
    let (window_seconds, window) = candidates::analysis_window(params, sample_rate);
    let grid = FrameGrid::new(audio.duration(), window_seconds, step);
    if grid.is_empty() {
        return PitchTrack::new(Vec::new());
    }

    let mono = audio.mono_mix();
    let mut plan = RealFftPlan::new();
    let signal = acf::soft_lowpass(mono.as_ref(), sample_rate, &mut plan);
    let global_peak = signal.iter().map(|sample| sample.abs()).fold(0.0, f64::max);
    let context = candidates::CandidateContext {
        signal: &signal,
        sample_rate,
        params,
        physical_window_seconds: window_seconds,
        window: &window,
        global_peak,
    };
    let frame_candidates = grid
        .centers()
        .map(|time| context.candidates_for_frame(time, &mut plan))
        .collect();

    path::viterbi_track(frame_candidates, params)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use phx_audio::Audio;

    use super::*;
    use crate::acf::window_autocorrelation;
    use crate::candidates::{unvoiced_strength, voiced_strength};
    use crate::path::transition_cost;
    use crate::types::hz_to_semitones;

    fn audio_from_signal(signal: Vec<f32>, sample_rate: f64) -> Audio {
        Audio::new(vec![signal], sample_rate).expect("valid synthetic audio")
    }

    fn sine(freq: f64, sample_rate: f64, duration: f64) -> Vec<f32> {
        let n = (duration * sample_rate).round() as usize;
        (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate).sin() as f32)
            .collect()
    }

    fn analyse_signal(signal: Vec<f32>, sample_rate: f64, params: PitchParams) -> PitchTrack {
        let audio = audio_from_signal(signal, sample_rate);
        pitch_track(audio.slice_samples(0..audio.frames()), &params)
    }

    fn assert_relative_close(actual: f64, expected: f64, tolerance: f64) {
        let relative = (actual - expected).abs() / expected;
        assert!(
            relative <= tolerance,
            "got {actual}, want {expected}, relative error {relative}"
        );
    }

    #[test]
    fn pure_tone_mean_f0_is_accurate() {
        let sample_rate = 44_100.0;
        let f0 = 150.0;
        let track = analyse_signal(
            sine(f0, sample_rate, 0.5),
            sample_rate,
            PitchParams::default(),
        );
        let mean = track.mean_hz(TimeSpan::new(0.1, 0.4)).unwrap();
        assert_relative_close(mean, f0, 0.001);
    }

    #[test]
    fn am_tone_mean_f0_is_accurate() {
        let sample_rate = 44_100.0;
        let f0 = 180.0;
        let n = (0.6_f64 * sample_rate).round() as usize;
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sample_rate;
                let envelope = 0.8 + 0.35 * (2.0 * PI * 5.0 * t).sin();
                (envelope * (2.0 * PI * f0 * t).sin()) as f32
            })
            .collect();
        let track = analyse_signal(signal, sample_rate, PitchParams::default());
        let mean = track.mean_hz(TimeSpan::new(0.1, 0.5)).unwrap();
        assert_relative_close(mean, f0, 0.001);
    }

    #[test]
    fn tone_plus_deterministic_noise_mean_f0_is_accurate() {
        let sample_rate = 44_100.0;
        let f0 = 220.0;
        let n = (0.6_f64 * sample_rate).round() as usize;
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next_noise = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let unit = (seed >> 11) as f64 / ((1_u64 << 53) as f64);
            2.0 * unit - 1.0
        };
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sample_rate;
                let tone = (2.0 * PI * f0 * t).sin();
                (tone + 0.08 * next_noise()) as f32
            })
            .collect();
        let track = analyse_signal(signal, sample_rate, PitchParams::default());
        let mean = track.mean_hz(TimeSpan::new(0.1, 0.5)).unwrap();
        assert_relative_close(mean, f0, 0.001);
    }

    #[test]
    fn viterbi_path_suppresses_isolated_octave_error() {
        let sample_rate = 44_100.0;
        let f0 = 150.0;
        let n = (0.7_f64 * sample_rate).round() as usize;
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sample_rate;
                let fundamental = (2.0 * PI * f0 * t).sin();
                let even_weight = if (0.31..=0.36).contains(&t) { 5.0 } else { 2.0 };
                let even = even_weight * (2.0 * PI * 2.0 * f0 * t).sin();
                (fundamental + even) as f32
            })
            .collect();
        let params = PitchParams {
            octave_cost: 0.08,
            octave_jump_cost: 1.0,
            ..PitchParams::default()
        };
        let track = analyse_signal(signal, sample_rate, params);
        let wrong_raw_frames = track
            .frames()
            .iter()
            .filter(|frame| {
                frame
                    .candidates
                    .iter()
                    .filter(|candidate| candidate.frequency > 0.0)
                    .max_by(|a, b| a.strength.total_cmp(&b.strength))
                    .is_some_and(|candidate| (candidate.frequency - 2.0 * f0).abs() < 0.03 * f0)
            })
            .count();
        assert!(
            wrong_raw_frames > 0,
            "stress case must contain raw octave errors"
        );

        let voiced: Vec<f64> = track.frames().iter().filter_map(|frame| frame.f0).collect();
        let close = voiced
            .iter()
            .filter(|&&f| (f - f0).abs() <= 0.03 * f0)
            .count();
        assert!(
            close as f64 >= 0.9 * voiced.len() as f64,
            "{close} of {} voiced frames near {f0}",
            voiced.len()
        );
    }

    #[test]
    fn strength_formulas_match_equations() {
        let params = PitchParams::default();
        let unvoiced = unvoiced_strength(&params, 0.03, 1.0);
        let expected_unvoiced = params.voicing_threshold
            + (2.0 - (0.03 / 1.0) / (params.silence_threshold / (1.0 + params.voicing_threshold)))
                .max(0.0);
        assert!((unvoiced - expected_unvoiced).abs() < 1e-12);

        let lag_seconds = 1.0 / 150.0;
        let voiced = voiced_strength(&params, 0.9, lag_seconds);
        let expected_voiced = 0.9 - params.octave_cost * (params.floor_hz * lag_seconds).log2();
        assert!((voiced - expected_voiced).abs() < 1e-12);
    }

    #[test]
    fn transition_cost_matches_equation() {
        let params = PitchParams::default();
        assert_eq!(transition_cost(0.0, 0.0, &params), 0.0);
        assert_eq!(
            transition_cost(0.0, 150.0, &params),
            params.voiced_unvoiced_cost
        );
        assert_eq!(
            transition_cost(150.0, 0.0, &params),
            params.voiced_unvoiced_cost
        );
        let jump = transition_cost(100.0, 200.0, &params);
        assert!((jump - params.octave_jump_cost).abs() < 1e-12);
    }

    #[test]
    fn window_acf_closed_form_matches_reference_points() {
        let t = 0.04;
        assert!((window_autocorrelation(0.0, t) - 1.0).abs() < 1e-12);
        assert!(window_autocorrelation(t, t).abs() < 1e-12);
        let half = window_autocorrelation(0.5 * t, t);
        let expected_half = 0.5 * (2.0 / 3.0 - 1.0 / 3.0);
        assert!((half - expected_half).abs() < 1e-12);
    }

    #[test]
    fn time_span_stats_use_voiced_frames_inside_span() {
        let track = PitchTrack::new(vec![
            PitchFrame {
                time: 0.0,
                f0: Some(100.0),
                strength: 0.9,
                candidates: Vec::new(),
            },
            PitchFrame {
                time: 0.5,
                f0: None,
                strength: 0.2,
                candidates: Vec::new(),
            },
            PitchFrame {
                time: 1.0,
                f0: Some(200.0),
                strength: 0.8,
                candidates: Vec::new(),
            },
            PitchFrame {
                time: 1.5,
                f0: Some(300.0),
                strength: 0.7,
                candidates: Vec::new(),
            },
        ]);
        let span = TimeSpan::new(0.0, 1.0);
        assert_eq!(track.mean_hz(span), Some(150.0));
        assert_eq!(track.median_hz(span), Some(150.0));
        assert_eq!(track.min_hz(span), Some(100.0));
        assert_eq!(track.max_hz(span), Some(200.0));
        assert_eq!(
            track.mean_semitones(TimeSpan::new(1.0, 1.0)),
            Some(hz_to_semitones(200.0))
        );
        assert_eq!(track.mean_hz(TimeSpan::new(0.25, 0.75)), None);
    }

    #[test]
    #[should_panic(expected = "TimeSpan start must be <= end")]
    fn time_span_rejects_reversed_bounds() {
        let _ = TimeSpan::new(1.0, 0.0);
    }

    #[test]
    fn degenerate_inputs_return_empty_tracks() {
        let sample_rate = 44_100.0;
        let audio = audio_from_signal(sine(150.0, sample_rate, 0.5), sample_rate);
        let view = audio.slice_samples(0..audio.frames());

        let mut params = PitchParams {
            floor_hz: 0.0,
            ..PitchParams::default()
        };
        assert!(pitch_track(view.clone(), &params).frames().is_empty());

        params = PitchParams {
            ceiling_hz: 75.0,
            ..PitchParams::default()
        };
        assert!(pitch_track(view.clone(), &params).frames().is_empty());

        params = PitchParams {
            max_candidates: 0,
            ..PitchParams::default()
        };
        assert!(pitch_track(view.clone(), &params).frames().is_empty());

        let short = audio_from_signal(sine(150.0, sample_rate, 0.01), sample_rate);
        assert!(
            pitch_track(
                short.slice_samples(0..short.frames()),
                &PitchParams::default()
            )
            .frames()
            .is_empty()
        );
    }
}
