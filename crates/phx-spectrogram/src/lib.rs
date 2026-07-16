//! Gaussian-window STFT power spectral density; viewport-independent tile
//! computation in dB.
#![warn(missing_docs)]

use std::f64::consts::PI;

use phx_audio::AudioView;
use phx_dsp::{FrameGrid, RealFftPlan, Window, next_pow2, window_samples};

const DEFAULT_WINDOW_LENGTH: f64 = 0.005;
const DEFAULT_MAX_FREQUENCY: f64 = 5000.0;
const DEFAULT_TIME_STEP: f64 = 0.002;
const DEFAULT_FREQUENCY_STEP: f64 = 20.0;
const SILENT_PSD_FLOOR: f64 = 1.0e-300;

/// Spectrogram analysis parameters.
///
/// The default values follow Praat's documented spectrogram defaults from
/// "Sound: To Spectrogram...": 5 ms window length, 5000 Hz maximum frequency,
/// 2 ms time step, 20 Hz frequency step, and a Gaussian window. The window
/// length is the Gaussian effective length `L`; with Praat's
/// `Window::Gaussian { effective_len_factor: 2.0 }`, the physical window is
/// twice as long and the −3 dB bandwidth is `1.2982804 / L`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectrogramParams {
    /// Gaussian effective window length in seconds.
    pub window_length: f64,
    /// Highest frequency requested for the object-level frequency grid.
    pub max_frequency: f64,
    /// Requested frame hop in seconds, clamped by [`effective_time_step`].
    pub time_step: f64,
    /// Requested frequency spacing in hertz, clamped by [`effective_frequency_step`].
    pub frequency_step: f64,
    /// Analysis window shape.
    pub window: Window,
}

impl Default for SpectrogramParams {
    fn default() -> Self {
        Self {
            window_length: DEFAULT_WINDOW_LENGTH,
            max_frequency: DEFAULT_MAX_FREQUENCY,
            time_step: DEFAULT_TIME_STEP,
            frequency_step: DEFAULT_FREQUENCY_STEP,
            window: Window::default(),
        }
    }
}

/// Request for a spectrogram tile.
///
/// The time axis is selected from the object-level [`FrameGrid`] derived from
/// the whole audio view duration, the physical window length (twice the
/// effective length for the Gaussian window), and the clamped time step. The
/// frequency axis is selected from a grid derived from the
/// clamped frequency step and FFT bins. `width_px` and `height_px` request the
/// number of selected snapped coordinates; when the natural snapped interval
/// contains a different count, nearest-index resampling is used over that
/// snapped interval.
#[derive(Debug, Clone, PartialEq)]
pub struct TileRequest {
    /// Left edge of the requested time range in seconds.
    pub t0: f64,
    /// Right edge of the requested time range in seconds.
    pub t1: f64,
    /// Lower edge of the requested frequency range in hertz.
    pub f0: f64,
    /// Upper edge of the requested frequency range in hertz.
    pub f1: f64,
    /// Requested number of tile columns.
    pub width_px: u32,
    /// Requested number of tile rows.
    pub height_px: u32,
    /// Spectrogram analysis parameters.
    pub params: SpectrogramParams,
}

/// A rendered spectrogram tile in raw PSD-derived decibels.
///
/// Values are row-major with row 0 at the lowest frequency in `f_axis`.
/// `t_axis` and `f_axis` store the snapped analysis coordinates selected for
/// each column and row. The raw `db` values do not include display
/// pre-emphasis.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    /// Raw power spectral density in `10·log10(Pa²/Hz)`, row-major.
    pub db: Vec<f32>,
    /// Snapped time coordinate for each tile column, in seconds.
    pub t_axis: Vec<f64>,
    /// Snapped frequency coordinate for each tile row, in hertz.
    pub f_axis: Vec<f64>,
}

/// A single snapped spectrogram frame in raw PSD-derived decibels.
///
/// `db[i]` is the raw `10·log10(Pa²/Hz)` value at `f_axis[i]`, without display
/// pre-emphasis.
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    /// Raw power spectral density in `10·log10(Pa²/Hz)`.
    pub db: Vec<f32>,
    /// Snapped frequency coordinate for each value, in hertz.
    pub f_axis: Vec<f64>,
}

/// Returns the effective time step after Praat's spectrogram speed clamp.
///
/// Praat's manual documents a minimum time step of `windowLength/(8√π)`.
/// Requests below that minimum are clamped upward.
#[must_use]
pub fn effective_time_step(params: &SpectrogramParams) -> f64 {
    validate_params(params);
    params
        .time_step
        .max(params.window_length / (8.0 * PI.sqrt()))
}

/// Returns the effective frequency step after Praat's spectrogram speed clamp.
///
/// Praat's manual documents a minimum frequency step of
/// `√π/(8·windowLength)`. Requests below that minimum are clamped upward.
#[must_use]
pub fn effective_frequency_step(params: &SpectrogramParams) -> f64 {
    validate_params(params);
    params
        .frequency_step
        .max(PI.sqrt() / (8.0 * params.window_length))
}

/// Applies display-only spectrogram pre-emphasis to a dB value.
///
/// The algorithms report records this as open item 1: the Praat manual does
/// not give a closed-form corner frequency for spectrogram display
/// pre-emphasis. This crate uses `+6·log2(f/1000)` dB above 1000 Hz and leaves
/// values at or below 1000 Hz unchanged. Raw tiles and slices never include
/// this adjustment.
#[must_use]
pub fn apply_display_preemphasis_db(db: f64, frequency_hz: f64) -> f64 {
    if frequency_hz > 1000.0 {
        db + 6.0 * (frequency_hz / 1000.0).log2()
    } else {
        db
    }
}

/// Computes a raw PSD spectrogram tile for an audio view.
///
/// The PSD estimate is the one-sided modified periodogram
/// `Pxx[k] = c[k]·|X[k]|² / (sample_rate·Σw[n]²)`, where `X[k]` is the
/// unnormalised DFT of the windowed frame, `c[k]` is `1` for DC and Nyquist
/// and `2` for interior one-sided bins, and `Σw[n]²` is the window energy.
/// This gives units of `Pa²/Hz`; integrating a bin-centred tone over frequency
/// recovers its mean-square pressure. Values are converted with
/// `10·log10(Pxx)`. Silent or underflowed bins are clamped to `1e-300 Pa²/Hz`
/// before conversion so public output contains finite `f32` values.
#[must_use]
pub fn compute_tile(audio: AudioView<'_>, req: &TileRequest) -> Tile {
    validate_request(req);
    let mono = audio.mono_mix();
    let analysis = Analysis::new(audio.sample_rate(), audio.duration(), &req.params);
    let centers: Vec<f64> = analysis.frame_grid.centers().collect();
    let time_indices = select_axis_indices(
        &centers,
        req.t0.min(req.t1),
        req.t0.max(req.t1),
        req.width_px as usize,
    );
    let freq_indices = select_axis_indices(
        &analysis.frequencies,
        req.f0.min(req.f1),
        req.f0.max(req.f1),
        req.height_px as usize,
    );

    let mut fft = RealFftPlan::new();
    let mut db = Vec::with_capacity(time_indices.len() * freq_indices.len());
    for &freq_index in &freq_indices {
        for &time_index in &time_indices {
            let spectrum = analysis.frame_db(mono.as_ref(), centers[time_index], &mut fft);
            db.push(spectrum[freq_index] as f32);
        }
    }

    Tile {
        db,
        t_axis: time_indices.iter().map(|&i| centers[i]).collect(),
        f_axis: freq_indices
            .iter()
            .map(|&i| analysis.frequencies[i])
            .collect(),
    }
}

/// Computes the raw spectrum nearest to `at` on the global frame grid.
///
/// The frame centre is selected from the same whole-audio [`FrameGrid`] used by
/// [`compute_tile`]; no independent frame is centred exactly at `at`.
#[must_use]
pub fn spectral_slice(audio: AudioView<'_>, at: f64, params: &SpectrogramParams) -> Slice {
    assert!(at.is_finite(), "slice time must be finite");
    let mono = audio.mono_mix();
    let analysis = Analysis::new(audio.sample_rate(), audio.duration(), params);
    let centers: Vec<f64> = analysis.frame_grid.centers().collect();
    if centers.is_empty() {
        return Slice {
            db: Vec::new(),
            f_axis: Vec::new(),
        };
    }
    let frame_index = nearest_axis_index(&centers, at);
    let mut fft = RealFftPlan::new();
    let db = analysis
        .frame_db(mono.as_ref(), centers[frame_index], &mut fft)
        .into_iter()
        .map(|v| v as f32)
        .collect();
    Slice {
        db,
        f_axis: analysis.frequencies,
    }
}

/// The global analysis axes for an audio view and parameters.
///
/// `times` are the frame-centre seconds of the object-level [`FrameGrid`];
/// `frequencies` are the snapped frequency-row centres in hertz. Both depend
/// only on the signal duration, sample rate, and `params`, so identical
/// coordinates land on identical indices no matter which tile requests them.
/// Building them runs no FFT.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisAxes {
    /// Frame-centre time of each column, in seconds.
    pub times: Vec<f64>,
    /// Snapped frequency-row centre of each row, in hertz.
    pub frequencies: Vec<f64>,
}

/// A contiguous block of global frame columns in raw PSD-derived decibels.
///
/// Storage is column-major: the value at `(local_col, row)` is at
/// `db[local_col * freq_len + row]`, where `local_col` counts up from
/// `first_col`. The dB values are bit-for-bit identical to the matching cells
/// of [`compute_tile`], which is what lets a cache keyed by column block serve
/// any viewport that overlaps it.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnBlock {
    /// Column-major raw PSD in `10·log10(Pa²/Hz)`.
    pub db: Vec<f32>,
    /// Global index of the first column present.
    pub first_col: usize,
    /// Number of columns present (may be shorter than requested at the grid end).
    pub col_count: usize,
    /// Number of frequency rows per column.
    pub freq_len: usize,
}

/// Returns the global time and frequency axes without running any FFT.
#[must_use]
pub fn analysis_axes(audio: AudioView<'_>, params: &SpectrogramParams) -> AnalysisAxes {
    let analysis = Analysis::new(audio.sample_rate(), audio.duration(), params);
    AnalysisAxes {
        times: analysis.frame_grid.centers().collect(),
        frequencies: analysis.frequencies,
    }
}

/// Computes raw PSD dB for a contiguous block of global frame columns.
///
/// Columns are `[first_col, first_col + col_count)` intersected with the
/// object-level frame grid; every frequency row is included. Frame centres come
/// from the same whole-audio grid [`compute_tile`] uses, so the block's values
/// match a tile that overlaps it bit for bit.
#[must_use]
pub fn compute_column_block(
    audio: AudioView<'_>,
    params: &SpectrogramParams,
    first_col: usize,
    col_count: usize,
) -> ColumnBlock {
    let mono = audio.mono_mix();
    let analysis = Analysis::new(audio.sample_rate(), audio.duration(), params);
    let centers: Vec<f64> = analysis.frame_grid.centers().collect();
    let freq_len = analysis.frequencies.len();
    let start = first_col.min(centers.len());
    let end = first_col.saturating_add(col_count).min(centers.len());
    let mut fft = RealFftPlan::new();
    let mut db = Vec::with_capacity(end.saturating_sub(start) * freq_len);
    for &center in &centers[start..end] {
        let spectrum = analysis.frame_db(mono.as_ref(), center, &mut fft);
        db.extend(spectrum.iter().map(|&v| v as f32));
    }
    ColumnBlock {
        db,
        first_col: start,
        col_count: end.saturating_sub(start),
        freq_len,
    }
}

struct Analysis {
    sample_rate: f64,
    frame_grid: FrameGrid,
    window: Vec<f64>,
    window_energy: f64,
    fft_len: usize,
    fft_bins: Vec<usize>,
    frequencies: Vec<f64>,
}

impl Analysis {
    fn new(sample_rate: f64, duration: f64, params: &SpectrogramParams) -> Self {
        validate_params(params);
        assert!(sample_rate.is_finite() && sample_rate > 0.0);
        assert!(duration.is_finite() && duration >= 0.0);

        let time_step = effective_time_step(params);
        let frequency_step = effective_frequency_step(params);
        let physical_window = params.window_length * physical_window_factor(params.window);
        let window_len = ((physical_window * sample_rate).round() as usize).saturating_add(1);
        let window_len = window_len.max(1);
        let min_fft_len = (sample_rate / frequency_step).ceil() as usize + 1;
        let fft_len = next_pow2(window_len.max(min_fft_len));
        let window = window_samples(params.window, window_len);
        let window_energy = window.iter().map(|v| v * v).sum();
        let (fft_bins, frequencies) =
            frequency_grid(sample_rate, fft_len, params.max_frequency, frequency_step);

        Self {
            sample_rate,
            frame_grid: FrameGrid::new(duration, physical_window, time_step),
            window,
            window_energy,
            fft_len,
            fft_bins,
            frequencies,
        }
    }

    fn frame_db(&self, samples: &[f32], center: f64, fft: &mut RealFftPlan) -> Vec<f64> {
        if self.window.is_empty() || self.fft_bins.is_empty() {
            return Vec::new();
        }

        let mut frame = vec![0.0; self.fft_len];
        let midpoint = (self.window.len().saturating_sub(1)) as f64 / 2.0;
        let center_sample = center * self.sample_rate;
        for (i, (&w, dst)) in self.window.iter().zip(frame.iter_mut()).enumerate() {
            let sample_index = (center_sample + i as f64 - midpoint).round();
            if sample_index >= 0.0 {
                let sample_index = sample_index as usize;
                if let Some(&sample) = samples.get(sample_index) {
                    *dst = f64::from(sample) * w;
                }
            }
        }

        let spectrum = fft.rfft(&mut frame);
        self.fft_bins
            .iter()
            .map(|&bin| {
                let norm = spectrum[bin].norm_sqr();
                let one_sided =
                    if bin == 0 || (self.fft_len.is_multiple_of(2) && bin == self.fft_len / 2) {
                        1.0
                    } else {
                        2.0
                    };
                let psd = one_sided * norm / (self.sample_rate * self.window_energy);
                10.0 * psd.max(SILENT_PSD_FLOOR).log10()
            })
            .collect()
    }
}

fn physical_window_factor(window: Window) -> f64 {
    match window {
        Window::Gaussian {
            effective_len_factor,
        } => effective_len_factor,
        Window::Hanning | Window::Kaiser { .. } => 1.0,
    }
}

fn frequency_grid(
    sample_rate: f64,
    fft_len: usize,
    max_frequency: f64,
    frequency_step: f64,
) -> (Vec<usize>, Vec<f64>) {
    let nyquist = sample_rate / 2.0;
    let max_frequency = max_frequency.min(nyquist);
    let fft_bin_hz = sample_rate / fft_len as f64;
    let max_bin = fft_len / 2;
    let mut bins = Vec::new();
    let mut frequencies = Vec::new();
    let mut previous = None;
    let mut target = 0.0;
    while target <= max_frequency + frequency_step * 1.0e-9 {
        let bin = ((target / fft_bin_hz).round() as usize).min(max_bin);
        if Some(bin) != previous {
            let frequency = bin as f64 * fft_bin_hz;
            if frequency <= max_frequency + fft_bin_hz * 0.5 {
                bins.push(bin);
                frequencies.push(frequency);
                previous = Some(bin);
            }
        }
        target += frequency_step;
    }
    (bins, frequencies)
}

/// Selects the axis indices a tile column or row range maps to.
///
/// Returns the snapped indices of `axis` that fall inside `[start, end]`, nearest
/// index-resampled to exactly `pixels` entries when the natural count differs.
/// The result is empty when no axis point lies in the range. Both a tile request
/// and a cached column block resolve their coordinates through this function, so
/// identical `(axis, start, end, pixels)` inputs always pick identical indices.
#[must_use]
pub fn select_axis_indices(axis: &[f64], start: f64, end: f64, pixels: usize) -> Vec<usize> {
    if axis.is_empty() || pixels == 0 {
        return Vec::new();
    }
    let first = axis.partition_point(|&v| v < start);
    let last_exclusive = axis.partition_point(|&v| v <= end);
    if first >= last_exclusive {
        return Vec::new();
    }
    let count = last_exclusive - first;
    if count == pixels {
        return (first..last_exclusive).collect();
    }
    if pixels == 1 {
        return vec![first + count / 2];
    }
    (0..pixels)
        .map(|i| {
            let u = i as f64 / (pixels - 1) as f64;
            first + (u * (count - 1) as f64).round() as usize
        })
        .collect()
}

fn nearest_axis_index(axis: &[f64], target: f64) -> usize {
    let split = axis.partition_point(|&v| v < target);
    if split == 0 {
        0
    } else if split >= axis.len() {
        axis.len() - 1
    } else if (axis[split] - target).abs() < (target - axis[split - 1]).abs() {
        split
    } else {
        split - 1
    }
}

fn validate_request(req: &TileRequest) {
    assert!(req.t0.is_finite() && req.t1.is_finite());
    assert!(req.f0.is_finite() && req.f1.is_finite());
    validate_params(&req.params);
}

fn validate_params(params: &SpectrogramParams) {
    assert!(
        params.window_length.is_finite() && params.window_length > 0.0,
        "window_length must be finite and positive"
    );
    assert!(
        params.max_frequency.is_finite() && params.max_frequency >= 0.0,
        "max_frequency must be finite and non-negative"
    );
    assert!(
        params.time_step.is_finite() && params.time_step > 0.0,
        "time_step must be finite and positive"
    );
    assert!(
        params.frequency_step.is_finite() && params.frequency_step > 0.0,
        "frequency_step must be finite and positive"
    );
    if let Window::Gaussian {
        effective_len_factor,
    } = params.window
    {
        assert!(
            effective_len_factor.is_finite() && effective_len_factor > 0.0,
            "Gaussian effective_len_factor must be finite and positive"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phx_audio::Audio;

    fn sine_audio(sample_rate: f64, duration: f64, frequency: f64) -> Audio {
        let frames = (sample_rate * duration).round() as usize;
        let samples = (0..frames)
            .map(|i| (2.0 * PI * frequency * i as f64 / sample_rate).sin() as f32)
            .collect();
        Audio::new(vec![samples], sample_rate).unwrap()
    }

    fn oracle_audio(sample_rate: f64, duration: f64) -> Audio {
        let frames = (sample_rate * duration).round() as usize;
        let samples = (0..frames)
            .map(|i| {
                let t = i as f64 / sample_rate;
                let envelope = 0.7 + 0.2 * (2.0 * PI * 7.0 * t).cos();
                let signal = envelope * (2.0 * PI * 440.0 * t).sin()
                    + 0.3 * (2.0 * PI * 1230.0 * t + 0.2).cos()
                    + 0.05 * (2.0 * PI * 2750.0 * t).sin();
                signal as f32
            })
            .collect();
        Audio::new(vec![samples], sample_rate).unwrap()
    }

    #[test]
    fn step_clamps_take_effect() {
        let params = SpectrogramParams {
            window_length: 0.005,
            time_step: 1.0e-6,
            frequency_step: 1.0e-6,
            ..SpectrogramParams::default()
        };
        assert!((effective_time_step(&params) - 0.005 / (8.0 * PI.sqrt())).abs() < 1.0e-15);
        assert!((effective_frequency_step(&params) - PI.sqrt() / (8.0 * 0.005)).abs() < 1.0e-12);

        let audio = sine_audio(16_000.0, 0.1, 1000.0);
        let tile = compute_tile(
            audio.slice_samples(0..audio.frames()),
            &TileRequest {
                t0: 0.0,
                t1: 0.1,
                f0: 0.0,
                f1: 500.0,
                width_px: 3,
                height_px: 3,
                params,
            },
        );
        assert!((tile.t_axis[1] - tile.t_axis[0]) >= effective_time_step(&params) - 1.0e-15);
        assert!((tile.f_axis[1] - tile.f_axis[0]) >= effective_frequency_step(&params) * 0.5);
    }

    #[test]
    fn pure_sine_has_no_high_sidelobes_far_from_line() {
        let audio = sine_audio(16_000.0, 0.2, 1000.0);
        let params = SpectrogramParams {
            window_length: 0.03,
            max_frequency: 3000.0,
            frequency_step: 5.0,
            ..SpectrogramParams::default()
        };
        let slice = spectral_slice(audio.slice_samples(0..audio.frames()), 0.1, &params);
        let peak = slice.db.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let peak_index = slice
            .db
            .iter()
            .position(|&v| v == peak)
            .expect("non-empty slice");
        assert!((slice.f_axis[peak_index] - 1000.0).abs() < 20.0);

        let bandwidth = 1.298_280_4 / params.window_length;
        for (&db, &frequency) in slice.db.iter().zip(&slice.f_axis) {
            if (frequency - 1000.0).abs() > 4.0 * bandwidth {
                assert!(
                    f64::from(peak - db) >= 60.0,
                    "bin {frequency} Hz is {} dB below peak",
                    peak - db
                );
            }
        }
    }

    #[test]
    fn overlapping_tiles_share_values_bit_for_bit() {
        let audio = sine_audio(16_000.0, 0.16, 1000.0);
        let params = SpectrogramParams {
            window_length: 0.01,
            max_frequency: 2400.0,
            time_step: 0.004,
            frequency_step: 80.0,
            ..SpectrogramParams::default()
        };
        let a = compute_tile(
            audio.slice_samples(0..audio.frames()),
            &TileRequest {
                t0: 0.02,
                t1: 0.12,
                f0: 400.0,
                f1: 1800.0,
                width_px: 26,
                height_px: 18,
                params,
            },
        );
        let b = compute_tile(
            audio.slice_samples(0..audio.frames()),
            &TileRequest {
                t0: 0.052,
                t1: 0.096,
                f0: 720.0,
                f1: 1520.0,
                width_px: 12,
                height_px: 11,
                params,
            },
        );

        for (bt, &time) in b.t_axis.iter().enumerate() {
            if let Some(at) = a.t_axis.iter().position(|&candidate| candidate == time) {
                for (bf, &frequency) in b.f_axis.iter().enumerate() {
                    if let Some(af) = a
                        .f_axis
                        .iter()
                        .position(|&candidate| candidate == frequency)
                    {
                        let av = a.db[af * a.t_axis.len() + at];
                        let bv = b.db[bf * b.t_axis.len() + bt];
                        assert_eq!(av.to_bits(), bv.to_bits());
                    }
                }
            }
        }
    }

    #[test]
    fn column_block_matches_compute_tile_cells_bit_for_bit() {
        let audio = sine_audio(16_000.0, 0.16, 1000.0);
        let params = SpectrogramParams {
            window_length: 0.01,
            max_frequency: 2400.0,
            time_step: 0.004,
            frequency_step: 80.0,
            ..SpectrogramParams::default()
        };
        let view = audio.slice_samples(0..audio.frames());
        let axes = analysis_axes(view.clone(), &params);
        let n_time = axes.times.len();
        let n_freq = axes.frequencies.len();
        assert!(n_time > 4 && n_freq > 2);

        // A tile spanning the whole grid: every cell must match the column block.
        let tile = compute_tile(
            view.clone(),
            &TileRequest {
                t0: axes.times[0],
                t1: *axes.times.last().unwrap(),
                f0: axes.frequencies[0],
                f1: *axes.frequencies.last().unwrap(),
                width_px: n_time as u32,
                height_px: n_freq as u32,
                params,
            },
        );
        let block = compute_column_block(view, &params, 0, n_time);
        assert_eq!(block.col_count, n_time);
        assert_eq!(block.freq_len, n_freq);
        for (t, _) in axes.times.iter().enumerate() {
            for (f, _) in axes.frequencies.iter().enumerate() {
                let tile_v = tile.db[f * n_time + t];
                let block_v = block.db[t * n_freq + f];
                assert_eq!(tile_v.to_bits(), block_v.to_bits());
            }
        }
    }

    #[test]
    fn column_block_clamps_at_the_grid_end() {
        let audio = sine_audio(16_000.0, 0.05, 800.0);
        let params = SpectrogramParams::default();
        let view = audio.slice_samples(0..audio.frames());
        let axes = analysis_axes(view.clone(), &params);
        let n_time = axes.times.len();
        // Requesting far past the end yields only the frames that exist.
        let block = compute_column_block(view, &params, n_time.saturating_sub(2), 512);
        assert_eq!(block.first_col, n_time.saturating_sub(2));
        assert_eq!(block.col_count, n_time - n_time.saturating_sub(2));
        assert_eq!(block.db.len(), block.col_count * block.freq_len);
    }

    #[test]
    fn display_preemphasis_is_separate_from_raw_values() {
        let audio = sine_audio(16_000.0, 0.1, 1200.0);
        let params = SpectrogramParams::default();
        let req = TileRequest {
            t0: 0.02,
            t1: 0.08,
            f0: 500.0,
            f1: 4000.0,
            width_px: 4,
            height_px: 8,
            params,
        };
        let raw = compute_tile(audio.slice_samples(0..audio.frames()), &req);
        let adjusted: Vec<f32> = raw
            .db
            .iter()
            .zip(raw.f_axis.iter().cycle())
            .map(|(&db, &frequency)| apply_display_preemphasis_db(f64::from(db), frequency) as f32)
            .collect();
        let raw_again = compute_tile(audio.slice_samples(0..audio.frames()), &req);
        assert_eq!(raw.db, raw_again.db);
        assert_ne!(raw.db, adjusted);
    }

    #[test]
    fn gaussian_bandwidth_matches_effective_window_length() {
        let audio = sine_audio(48_000.0, 0.3, 1000.0);
        let params = SpectrogramParams {
            window_length: 0.03,
            max_frequency: 1300.0,
            frequency_step: 1.0,
            ..SpectrogramParams::default()
        };
        let slice = spectral_slice(audio.slice_samples(0..audio.frames()), 0.15, &params);
        let peak_index = slice
            .db
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap();
        let target = f64::from(slice.db[peak_index]) - 10.0 * 2.0_f64.log10();
        let lower = crossing_frequency(&slice, peak_index, target, -1);
        let upper = crossing_frequency(&slice, peak_index, target, 1);
        let measured = upper - lower;
        let expected = 1.298_280_4 / params.window_length;
        assert!(
            (measured - expected).abs() < 1.0,
            "measured {measured}, expected {expected}"
        );
    }

    #[test]
    fn scipy_oracle_fixture_matches_relative_tolerance() {
        let fixture = include_str!("../../../tools/oracle/spectrogram_reference.csv");
        let rows = parse_oracle_fixture(fixture);
        let audio = oracle_audio(16_000.0, 0.08);
        let params = SpectrogramParams {
            window_length: 0.01,
            max_frequency: 3200.0,
            time_step: 0.004,
            frequency_step: 125.0,
            ..SpectrogramParams::default()
        };
        let tile = compute_tile(
            audio.slice_samples(0..audio.frames()),
            &TileRequest {
                t0: 0.0,
                t1: 0.08,
                f0: 0.0,
                f1: 3200.0,
                width_px: 18,
                height_px: 26,
                params,
            },
        );

        for row in rows {
            let time_index = tile
                .t_axis
                .iter()
                .position(|&v| (v - row.time).abs() < 1.0e-15)
                .expect("fixture time on tile axis");
            let freq_index = tile
                .f_axis
                .iter()
                .position(|&v| (v - row.frequency).abs() < 1.0e-12)
                .expect("fixture frequency on tile axis");
            let actual = f64::from(tile.db[freq_index * tile.t_axis.len() + time_index]);
            let reference = row.db;
            let rel = (actual - reference).abs() / reference.abs().max(1.0e-12);
            assert!(
                rel <= 1.0e-6,
                "time {} freq {} actual {actual} reference {reference} rel {rel}",
                row.time,
                row.frequency
            );
        }
    }

    fn crossing_frequency(slice: &Slice, peak_index: usize, target: f64, direction: isize) -> f64 {
        let mut i = peak_index as isize;
        loop {
            let next = i + direction;
            assert!(next >= 0 && (next as usize) < slice.db.len());
            let a = f64::from(slice.db[i as usize]);
            let b = f64::from(slice.db[next as usize]);
            if (a >= target && b <= target) || (a <= target && b >= target) {
                let fa = slice.f_axis[i as usize];
                let fb = slice.f_axis[next as usize];
                let u = (target - a) / (b - a);
                return fa + u * (fb - fa);
            }
            i = next;
        }
    }

    #[derive(Debug)]
    struct OracleRow {
        time: f64,
        frequency: f64,
        db: f64,
    }

    fn parse_oracle_fixture(text: &str) -> Vec<OracleRow> {
        text.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("time"))
            .map(|line| {
                let mut fields = line.split(',');
                OracleRow {
                    time: fields.next().unwrap().parse().unwrap(),
                    frequency: fields.next().unwrap().parse().unwrap(),
                    db: fields.next().unwrap().parse().unwrap(),
                }
            })
            .collect()
    }
}
