//! Squared signal convolved with a Kaiser-20 analysis window of effective
//! duration `3.2 / pitchFloor`, dB SPL re 2×10⁻⁵ Pa.
//!
//! Praat manual: "Sound: To Intensity..."
//! <https://www.fon.hum.uva.nl/praat/manual/Sound__To_Intensity___.html>,
//! "Intensity" <https://www.fon.hum.uva.nl/praat/manual/Intensity.html>,
//! "Intro 6.2. Configuring the intensity contour"
//! <https://www.fon.hum.uva.nl/praat/manual/Intro_6_2__Configuring_the_intensity_contour.html>.
//! The manual states the squared samples are convolved with a "Gaussian
//! analysis window (Kaiser-20; sidelobes below −190 dB)" of effective duration
//! `3.2 / pitchFloor`; the window is a Kaiser window whose shape parameter is
//! given by that "Kaiser-20" name (see [`INTENSITY_KAISER_BETA`]).
//! A `Sound` object's samples are documented as air pressure directly in
//! Pascal, so no separate calibration factor sits between sample amplitude
//! and the reference pressure below.
#![warn(missing_docs)]

use phx_audio::AudioView;
use phx_dsp::{FrameGrid, Window, window_samples};

/// `(2×10⁻⁵ Pa)²`, the reference pressure squared for dB SPL (Kinsler, Frey,
/// Coppens & Sanders, *Fundamentals of Acoustics*, 4th ed., 2000; equivalently
/// ANSI/ASA S1.1). `L = 10·log10(⟨p²⟩ / p_ref²)`.
const REFERENCE_PRESSURE_SQUARED: f64 = 4.0e-10;

/// Ratio of the truncated analysis window's physical length to its effective
/// duration.
///
/// Praat's Gaussian-family analysis windows (spectrogram, Burg formant,
/// intensity) run over a physical length of twice the effective duration; for
/// intensity the effective duration is `3.2 / pitchFloor` (manual, "Sound: To
/// Intensity...") and the physical support is therefore `6.4 / pitchFloor`.
/// That same physical length sets the frame-grid margins, so the leading and
/// trailing frames hold the whole window inside the signal exactly as Praat's
/// frame placement does.
const EFFECTIVE_LEN_FACTOR: f64 = 2.0;

/// Kaiser shape parameter for the intensity analysis window.
///
/// Praat's "Sound: To Intensity..." manual names the window "Kaiser-20;
/// sidelobes below −190 dB". A Kaiser window's stopband attenuation `A` (dB)
/// fixes its shape via `β = 0.1102·(A − 8.7)` for `A > 50` (Kaiser & Schafer,
/// "On the use of the I0-sinh window for spectrum analysis," *IEEE Trans.
/// ASSP* 28(1), 1980): `A = 190` gives `β = 0.1102·181.3 ≈ 19.98`, i.e. the
/// "Kaiser-20" label is the `β = 20` window. The value is documented, not
/// tuned to the oracle.
const INTENSITY_KAISER_BETA: f64 = 20.0;

/// Parameters for [`intensity_track`].
///
/// Defaults reproduce Praat's "Sound: To Intensity..." dialog defaults
/// (`algorithms-and-validation.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntensityParams {
    /// Pitch floor in hertz, sets the analysis window's effective duration
    /// (`3.2 / pitch_floor_hz`) and, when `time_step` is `None`, the frame
    /// hop. Praat-documented default: `100.0` Hz.
    pub pitch_floor_hz: f64,
    /// Frame hop in seconds. `None` resolves to Praat's automatic step,
    /// `0.8 / pitch_floor_hz`.
    pub time_step: Option<f64>,
    /// Local DC removal: subtract each frame's windowed-mean pressure before
    /// squaring, so a non-zero recording offset does not inflate the
    /// reported level. Praat-documented default: on. The manual states this
    /// is "computed locally around each point" without naming the local
    /// window; this implementation reuses the same Kaiser analysis window as
    /// the intensity smoothing itself, the only window the manual defines for
    /// this computation.
    pub subtract_mean: bool,
}

impl Default for IntensityParams {
    /// Praat-documented defaults: floor 100 Hz, automatic step, mean
    /// subtraction on (`algorithms-and-validation.md` §4.1).
    fn default() -> Self {
        Self {
            pitch_floor_hz: 100.0,
            time_step: None,
            subtract_mean: true,
        }
    }
}

impl IntensityParams {
    /// Resolves the analysis frame hop, in seconds.
    ///
    /// Returns `time_step` when set, otherwise Praat's automatic step
    /// `0.8 / pitch_floor_hz`.
    #[must_use]
    pub fn resolved_time_step(&self) -> f64 {
        self.time_step.unwrap_or(0.8 / self.pitch_floor_hz)
    }

    /// Effective (Gaussian-sigma-defining) duration of the analysis window,
    /// in seconds: `3.2 / pitch_floor_hz`. Guarantees pitch-synchronous
    /// ripple stays negligible for any true F0 at or above the floor
    /// (`algorithms-and-validation.md` §4.1).
    #[must_use]
    pub fn window_duration(&self) -> f64 {
        3.2 / self.pitch_floor_hz
    }
}

/// An intensity contour: one dB SPL value per frame of a
/// [`phx_dsp::FrameGrid`] anchored to the source signal's own time domain, so
/// a value queried at a given time is identical regardless of zoom or query
/// span.
#[derive(Debug, Clone, PartialEq)]
pub struct IntensityTrack {
    grid: FrameGrid,
    db: Vec<f64>,
}

impl IntensityTrack {
    /// The frame grid the contour was computed on.
    #[must_use]
    pub fn frame_grid(&self) -> &FrameGrid {
        &self.grid
    }

    /// Number of frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.db.len()
    }

    /// Whether the contour holds no frames (signal shorter than the analysis
    /// window).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    /// Absolute time of frame `i`, in seconds, or `None` if `i` is out of
    /// range.
    #[must_use]
    pub fn time(&self, i: usize) -> Option<f64> {
        self.grid.center(i)
    }

    /// Level of frame `i`, in dB SPL re 2×10⁻⁵ Pa, or `None` if `i` is out of
    /// range.
    #[must_use]
    pub fn db(&self, i: usize) -> Option<f64> {
        self.db.get(i).copied()
    }

    /// All frame levels, in dB SPL, ascending by time.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.db
    }

    /// Iterates `(time, dB)` pairs, ascending by time.
    pub fn iter(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.grid.centers().zip(self.db.iter().copied())
    }
}

/// Computes the intensity contour of `audio`.
///
/// Samples are squared, then convolved with a Kaiser window of shape
/// [`INTENSITY_KAISER_BETA`] and effective duration `3.2 / pitch_floor_hz` —
/// Praat's documented intensity window (manual, "Sound: To Intensity...";
/// `algorithms-and-validation.md` §4.1). When `params.subtract_mean` is set,
/// each frame's windowed-mean pressure is subtracted before squaring. Frames
/// sit on a [`FrameGrid`] built from the signal's own duration, so results are
/// independent of any viewport.
#[must_use]
pub fn intensity_track(audio: AudioView<'_>, params: &IntensityParams) -> IntensityTrack {
    let sample_rate = audio.sample_rate();
    let samples = audio.mono_mix();
    let window_duration = params.window_duration();
    // The frame count subtracts the physical window (the full
    // `EFFECTIVE_LEN_FACTOR × effective` span of the Kaiser window) from the
    // discrete signal duration, `sample count × sampling period`, so the
    // leading and trailing margins hold the whole analysis window inside the
    // signal exactly as Praat's frame placement does.
    let grid_duration = samples.len() as f64 * (1.0 / sample_rate);
    let grid = FrameGrid::new(
        grid_duration,
        EFFECTIVE_LEN_FACTOR * window_duration,
        params.resolved_time_step(),
    );

    let kernel = kaiser_kernel(window_duration, sample_rate);
    let half = (kernel.len() - 1) / 2;

    let db = grid
        .centers()
        .map(|t| {
            let center_index = (t * sample_rate).round() as i64;
            frame_db(&samples, center_index, half, &kernel, params.subtract_mean)
        })
        .collect();

    IntensityTrack { grid, db }
}

/// Builds a unit-sum discrete Kaiser convolution kernel of effective duration
/// `effective_duration` seconds, sampled at `sample_rate`.
///
/// The kernel spans `EFFECTIVE_LEN_FACTOR × effective_duration` seconds of
/// physical support and is centred on an odd number of samples so a single
/// index is the exact peak. Its shape parameter is [`INTENSITY_KAISER_BETA`],
/// Praat's documented "Kaiser-20" intensity window.
fn kaiser_kernel(effective_duration: f64, sample_rate: f64) -> Vec<f64> {
    let physical_duration = EFFECTIVE_LEN_FACTOR * effective_duration;
    let half_samples = ((physical_duration / 2.0) * sample_rate).round().max(1.0) as usize;
    let n = 2 * half_samples + 1;
    let mut weights = window_samples(
        Window::Kaiser {
            beta: INTENSITY_KAISER_BETA,
        },
        n,
    );
    let sum: f64 = weights.iter().sum();
    if sum > 0.0 {
        for w in &mut weights {
            *w /= sum;
        }
    }
    weights
}

/// Computes one frame's dB SPL level from `samples` around `center_index`,
/// weighted by `kernel` (already unit-sum, `2·half + 1` taps). Out-of-range
/// taps clamp to the nearest in-range sample; [`FrameGrid`] keeps every
/// frame's window inside the signal, so clamping only ever guards against
/// sub-sample rounding at the very edge.
fn frame_db(
    samples: &[f32],
    center_index: i64,
    half: usize,
    kernel: &[f64],
    subtract_mean: bool,
) -> f64 {
    let last = samples.len() as i64 - 1;
    let tap_at = |offset: usize| -> f64 {
        let idx = (center_index + offset as i64 - half as i64).clamp(0, last);
        f64::from(samples[idx as usize])
    };

    let mean = if subtract_mean {
        kernel
            .iter()
            .enumerate()
            .map(|(offset, &w)| w * tap_at(offset))
            .sum()
    } else {
        0.0
    };

    let mean_sq: f64 = kernel
        .iter()
        .enumerate()
        .map(|(offset, &w)| {
            let centered = tap_at(offset) - mean;
            w * centered * centered
        })
        .sum();

    10.0 * (mean_sq / REFERENCE_PRESSURE_SQUARED).log10()
}

#[cfg(test)]
mod tests {
    use super::*;
    use phx_audio::Audio;
    use std::f64::consts::PI;

    fn sine_audio(amplitude: f32, freq_hz: f64, sample_rate: f64, duration_s: f64) -> Audio {
        let frames = (duration_s * sample_rate).round() as usize;
        let samples = (0..frames)
            .map(|n| {
                let t = n as f64 / sample_rate;
                (f64::from(amplitude) * (2.0 * PI * freq_hz * t).sin()) as f32
            })
            .collect();
        Audio::new(vec![samples], sample_rate).unwrap()
    }

    /// A constant-amplitude sine yields a flat contour: ripple across
    /// interior frames (away from the leading/trailing analysis-window
    /// margin) stays below 1e-4 dB.
    #[test]
    fn constant_amplitude_sine_is_flat_away_from_edges() {
        let sample_rate = 44_100.0;
        let audio = sine_audio(0.6, 1000.0, sample_rate, 2.0);
        let params = IntensityParams::default();
        let track = intensity_track(audio.slice_samples(0..audio.frames()), &params);
        assert!(track.len() > 20, "need enough frames to trim margins");

        // Trim a few frames off each end: the Kaiser kernel's tails are
        // clamped to the boundary sample there, which is a legitimate edge
        // effect, not analysis ripple.
        let trim = 5;
        let interior = &track.values()[trim..track.len() - trim];
        let min = interior.iter().copied().fold(f64::INFINITY, f64::min);
        let max = interior.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max - min < 1e-4,
            "ripple {} dB exceeds 1e-4 dB (min {min}, max {max})",
            max - min
        );
    }

    /// A signal of known RMS pressure maps to the exact analytic dB SPL
    /// value, `L = 20·log10(p_rms / p_ref)`.
    #[test]
    fn known_rms_maps_to_exact_db() {
        let sample_rate = 44_100.0;
        let amplitude = 0.6_f64;
        let audio = sine_audio(amplitude as f32, 1000.0, sample_rate, 2.0);
        let params = IntensityParams::default();
        let track = intensity_track(audio.slice_samples(0..audio.frames()), &params);

        let p_rms = amplitude / 2.0_f64.sqrt();
        let expected_db = 20.0 * (p_rms / 2.0e-5).log10();

        let mid = track.len() / 2;
        let actual_db = track.db(mid).unwrap();
        assert!(
            (actual_db - expected_db).abs() < 1e-3,
            "frame {mid}: expected {expected_db} dB, got {actual_db} dB"
        );
    }

    /// With mean subtraction off, a pure-DC signal's level matches the
    /// closed-form `L = 20·log10(A / p_ref)` exactly (the kernel is
    /// unit-sum, so a constant maps to itself with no smoothing error).
    #[test]
    fn constant_dc_without_mean_subtraction_matches_closed_form() {
        let sample_rate = 16_000.0;
        let amplitude = 0.05_f32;
        let frames = (0.5 * sample_rate) as usize;
        let audio = Audio::new(vec![vec![amplitude; frames]], sample_rate).unwrap();
        let params = IntensityParams {
            subtract_mean: false,
            ..IntensityParams::default()
        };
        let track = intensity_track(audio.slice_samples(0..audio.frames()), &params);
        assert!(!track.is_empty());

        let expected_db = 20.0 * (f64::from(amplitude) / 2.0e-5).log10();
        for i in 0..track.len() {
            let actual = track.db(i).unwrap();
            assert!(
                (actual - expected_db).abs() < 1e-9,
                "frame {i}: expected {expected_db} dB, got {actual} dB"
            );
        }
    }

    /// Local DC removal drives a pure-DC signal's level to (numerically)
    /// negative infinity, since the local mean equals the signal exactly.
    #[test]
    fn constant_dc_with_mean_subtraction_is_silent() {
        let sample_rate = 16_000.0;
        let frames = (0.5 * sample_rate) as usize;
        let audio = Audio::new(vec![vec![0.05_f32; frames]], sample_rate).unwrap();
        let params = IntensityParams::default();
        let track = intensity_track(audio.slice_samples(0..audio.frames()), &params);
        assert!(!track.is_empty());
        // Without subtraction this amplitude reads ~68 dB (see the sibling
        // test); -150 dB is deep in f64 rounding noise, far below anything a
        // genuine signal could produce, so it safely demonstrates removal.
        for i in 0..track.len() {
            assert!(track.db(i).unwrap() < -150.0, "frame {i} not near-silent");
        }
    }

    /// Frames sit on a `FrameGrid` derived from the signal's own duration:
    /// recomputing the grid independently from the same `(duration, window,
    /// step)` reproduces the exact same frame count and centre times,
    /// independent of any viewport.
    #[test]
    fn frames_follow_the_frame_grid() {
        let sample_rate = 8_000.0;
        let audio = sine_audio(0.3, 300.0, sample_rate, 1.0);
        let params = IntensityParams::default();
        let view = audio.slice_samples(0..audio.frames());
        let track = intensity_track(view.clone(), &params);

        let expected_grid = FrameGrid::new(
            view.frames() as f64 * (1.0 / view.sample_rate()),
            EFFECTIVE_LEN_FACTOR * params.window_duration(),
            params.resolved_time_step(),
        );
        assert_eq!(track.frame_grid(), &expected_grid);
        assert_eq!(track.len(), expected_grid.len());
        for i in 0..track.len() {
            assert_eq!(track.time(i), expected_grid.center(i));
        }
    }

    #[test]
    fn default_params_match_documented_praat_defaults() {
        let params = IntensityParams::default();
        assert_eq!(params.pitch_floor_hz, 100.0);
        assert_eq!(params.time_step, None);
        assert!(params.subtract_mean);
        assert!((params.resolved_time_step() - 0.008).abs() < 1e-12);
        assert!((params.window_duration() - 0.032).abs() < 1e-12);
    }

    /// Too-short a signal for even one analysis window yields an empty
    /// contour rather than panicking.
    #[test]
    fn signal_shorter_than_window_is_empty() {
        let sample_rate = 16_000.0;
        let audio = sine_audio(0.2, 500.0, sample_rate, 0.001);
        let params = IntensityParams::default();
        let track = intensity_track(audio.slice_samples(0..audio.frames()), &params);
        assert!(track.is_empty());
    }
}
