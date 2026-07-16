//! Ignored timing harness for the pitch hot path. Run with
//! `cargo test -p phx-pitch --release -- --ignored --nocapture perf_`.

use std::f64::consts::PI;
use std::time::Instant;

use phx_audio::Audio;
use phx_dsp::{FrameGrid, RealFftPlan};

use crate::acf::{corrected_autocorrelation, soft_lowpass};
use crate::candidates::{CandidateContext, analysis_window};
use crate::params::PitchParams;

const SAMPLE_RATE: f64 = 44_100.0;
const DURATION: f64 = 2.0;

fn beep_bursts(gap_dither: f64) -> Vec<f32> {
    let n = (DURATION * SAMPLE_RATE).round() as usize;
    let mut seed = 0x1234_5678_9abc_def0_u64;
    (0..n)
        .map(|i| {
            let t = i as f64 / SAMPLE_RATE;
            // 120 ms tone burst, 80 ms gap.
            let phase = (t / 0.2).fract();
            if phase < 0.6 {
                0.5 * (2.0 * PI * 1000.0 * t).sin() as f32
            } else if gap_dither > 0.0 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let unit = (seed >> 11) as f64 / ((1_u64 << 53) as f64);
                (gap_dither * (2.0 * unit - 1.0)) as f32
            } else {
                0.0
            }
        })
        .collect()
}

fn clean_sine() -> Vec<f32> {
    let n = (DURATION * SAMPLE_RATE).round() as usize;
    (0..n)
        .map(|i| 0.5 * (2.0 * PI * 1000.0 * i as f64 / SAMPLE_RATE).sin() as f32)
        .collect()
}

fn silence() -> Vec<f32> {
    let n = (DURATION * SAMPLE_RATE).round() as usize;
    vec![0.0; n]
}

fn time_track(label: &str, signal: Vec<f32>) {
    let audio = Audio::new(vec![signal], SAMPLE_RATE).expect("valid audio");
    let view = audio.slice_samples(0..audio.frames());
    let params = PitchParams::default();
    // warm any lazy state
    let _ = crate::pitch_track(view.clone(), &params);
    let start = Instant::now();
    let track = crate::pitch_track(view, &params);
    let elapsed = start.elapsed();
    let voiced = track.frames().iter().filter(|f| f.f0.is_some()).count();
    println!(
        "[{label:>18}] pitch_track = {:8.2} ms  ({} frames, {voiced} voiced)",
        elapsed.as_secs_f64() * 1e3,
        track.frames().len()
    );
}

/// Per-frame breakdown: raw local-maximum count and time spent in the
/// candidate stage, so a candidate explosion shows up separately from raw ACF
/// cost.
fn breakdown(label: &str, signal: Vec<f32>) {
    let audio = Audio::new(vec![signal], SAMPLE_RATE).expect("valid audio");
    let view = audio.slice_samples(0..audio.frames());
    let params = PitchParams::default();
    let sample_rate = view.sample_rate();
    let step = params.resolved_step().unwrap();
    let (window_seconds, window) = analysis_window(&params, sample_rate);
    let grid = FrameGrid::new(view.duration(), window_seconds, step);
    let mono = view.mono_mix();
    let mut plan = RealFftPlan::new();

    let lp_start = Instant::now();
    let signal = soft_lowpass(mono.as_ref(), sample_rate, &mut plan);
    let lp_ms = lp_start.elapsed().as_secs_f64() * 1e3;

    let global_peak = signal.iter().map(|s| s.abs()).fold(0.0, f64::max);
    let ctx = CandidateContext {
        signal: &signal,
        sample_rate,
        params: &params,
        physical_window_seconds: window_seconds,
        window: &window,
        global_peak,
    };

    let mut acf_ms = 0.0;
    let mut cand_ms = 0.0;
    let mut total_local_maxima = 0usize;
    let mut max_local_maxima = 0usize;
    let mut frames_with_many = 0usize;

    for time in grid.centers() {
        // Raw local-maximum count on the corrected ACF (pre-truncation).
        let seg = crate::candidates::centered_segment(&signal, time, sample_rate, window.len());
        let acf_start = Instant::now();
        let rx = corrected_autocorrelation(&seg, &window, window_seconds, sample_rate, &mut plan);
        acf_ms += acf_start.elapsed().as_secs_f64() * 1e3;

        let min_lag = (sample_rate / params.ceiling_hz).ceil().max(1.0) as usize;
        let max_lag = (sample_rate / params.floor_hz).floor() as usize;
        let last = rx.len().saturating_sub(2);
        let (start, end) = (min_lag.max(1), max_lag.min(last));
        let mut local = 0usize;
        if start <= end {
            for lag in start..=end {
                if rx[lag - 1] < rx[lag] && rx[lag] > rx[lag + 1] {
                    local += 1;
                }
            }
        }
        total_local_maxima += local;
        max_local_maxima = max_local_maxima.max(local);
        if local > 30 {
            frames_with_many += 1;
        }

        let cand_start = Instant::now();
        let _ = ctx.candidates_for_frame(time, &mut plan);
        cand_ms += cand_start.elapsed().as_secs_f64() * 1e3;
    }

    let frame_count = grid.centers().count();
    println!(
        "[{label:>18}] soft_lowpass={lp_ms:7.2}ms  acf(sum)={acf_ms:8.2}ms  candidates(sum)={cand_ms:8.2}ms  | local-max: mean={:.1} max={max_local_maxima} frames>30={frames_with_many}/{frame_count}",
        total_local_maxima as f64 / frame_count as f64,
    );
}

#[test]
#[ignore = "timing harness; run manually in release"]
fn perf_three_cases() {
    println!();
    time_track("beep(zero-gap)", beep_bursts(0.0));
    time_track("beep(dither-gap)", beep_bursts(1e-4));
    time_track("clean-sine", clean_sine());
    time_track("silence", silence());
    println!("--- per-stage breakdown ---");
    breakdown("beep(zero-gap)", beep_bursts(0.0));
    breakdown("beep(dither-gap)", beep_bursts(1e-4));
    breakdown("clean-sine", clean_sine());
    breakdown("silence", silence());
}
