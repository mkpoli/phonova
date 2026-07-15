//! Pulse extraction from pitch + waveform, jitter/shimmer families, HNR,
//! CPP/CPPS, spectral moments, aggregate voice report.
#![warn(missing_docs)]

use std::borrow::Cow;

use phx_audio::AudioView;
use phx_dsp::{FrameGrid, RealFftPlan, Window, next_pow2, sinc_interpolate_max, window_samples};
use phx_pitch::{PitchParams, PitchTrack, TimeSpan, pitch_track};

const EPSILON: f64 = 1e-12;

/// A sequence of glottal-pulse times in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct PointProcess {
    times: Vec<f64>,
}

impl PointProcess {
    /// Creates a sorted point process from finite pulse times in seconds.
    ///
    /// Duplicate times closer than `1e-12` seconds are collapsed.
    #[must_use]
    pub fn new(mut times: Vec<f64>) -> Self {
        times.retain(|time| time.is_finite());
        times.sort_by(f64::total_cmp);
        times.dedup_by(|a, b| (*a - *b).abs() <= EPSILON);
        Self { times }
    }

    /// Returns pulse times in seconds.
    #[must_use]
    pub fn times(&self) -> &[f64] {
        &self.times
    }

    /// Returns pulse times within a closed time span.
    #[must_use]
    pub fn times_in(&self, span: TimeSpan) -> Vec<f64> {
        self.times
            .iter()
            .copied()
            .filter(|&time| span.contains(time))
            .collect()
    }
}

/// Pulse extraction parameters.
///
/// Defaults are implementation parameters for the period-by-period peak
/// alignment described in the Praat voice manual: positive waveform peaks,
/// a half-period peak-search radius, and accepted pulse spacings between
/// `0.6` and `1.6` times the local pitch period.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseParams {
    /// Search radius as a fraction of the local pitch period.
    pub peak_search_radius_periods: f64,
    /// Smallest accepted pulse spacing as a fraction of the local pitch period.
    pub min_period_factor: f64,
    /// Largest accepted pulse spacing as a fraction of the local pitch period.
    pub max_period_factor: f64,
    /// Minimum selected pitch-frame strength used as a pulse-chain seed.
    pub min_seed_strength: f64,
}

impl Default for PulseParams {
    fn default() -> Self {
        Self {
            peak_search_radius_periods: 0.5,
            min_period_factor: 0.6,
            max_period_factor: 1.6,
            min_seed_strength: 0.0,
        }
    }
}

/// Jitter measure selector from Praat voice report section 5.2.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum JitterKind {
    /// Average absolute difference between consecutive periods divided by mean period.
    Local,
    /// Average absolute difference between consecutive periods in seconds.
    LocalAbsolute,
    /// Relative average perturbation over three adjacent periods.
    Rap,
    /// Five-point period perturbation quotient.
    Ppq5,
    /// Average absolute difference between consecutive period differences.
    Ddp,
}

/// Shimmer measure selector from Praat voice report section 5.3.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ShimmerKind {
    /// Average absolute difference between consecutive amplitudes divided by mean amplitude.
    Local,
    /// Average absolute decibel ratio between consecutive amplitudes.
    LocalDb,
    /// Three-point amplitude perturbation quotient.
    Apq3,
    /// Five-point amplitude perturbation quotient.
    Apq5,
    /// Eleven-point amplitude perturbation quotient.
    Apq11,
    /// Three times APQ3.
    Dda,
}

/// Harmonicity analysis parameters.
///
/// Defaults match Praat "Sound: To Harmonicity (ac)...": time step `0.01` s,
/// pitch floor `75` Hz, silence threshold `0.1`, and `4.5` periods per window.
/// The ceiling `600` Hz is the Praat pitch ceiling default used to bound the
/// autocorrelation peak search.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicityParams {
    /// Frame step in seconds.
    pub time_step: f64,
    /// Lowest searched fundamental frequency in hertz.
    pub floor_hz: f64,
    /// Highest searched fundamental frequency in hertz.
    pub ceiling_hz: f64,
    /// Silence threshold relative to the global peak absolute amplitude.
    pub silence_threshold: f64,
    /// Analysis-window length in periods of `floor_hz`.
    pub periods_per_window: f64,
}

impl Default for HarmonicityParams {
    fn default() -> Self {
        Self {
            time_step: 0.01,
            floor_hz: 75.0,
            ceiling_hz: 600.0,
            silence_threshold: 0.1,
            periods_per_window: 4.5,
        }
    }
}

/// One harmonicity frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HnrFrame {
    /// Frame centre time in seconds.
    pub time: f64,
    /// Harmonics-to-noise ratio in decibels, absent for silent or invalid frames.
    pub hnr_db: Option<f64>,
    /// Corrected normalized autocorrelation peak used as the periodic fraction.
    pub periodic_fraction: Option<f64>,
}

/// Harmonicity track over a frame grid.
#[derive(Debug, Clone, PartialEq)]
pub struct HnrTrack {
    /// Frames in ascending time order.
    pub frames: Vec<HnrFrame>,
    /// Parameters used to compute the track.
    pub params: HarmonicityParams,
}

impl HnrTrack {
    /// Mean HNR in decibels over a closed time span.
    #[must_use]
    pub fn mean_db(&self, span: TimeSpan) -> Option<f64> {
        mean_option(
            self.frames
                .iter()
                .filter(|frame| span.contains(frame.time))
                .filter_map(|frame| frame.hnr_db),
        )
    }
}

/// CPP and CPPS parameters.
///
/// Defaults are implementation values in the Hillenbrand and Maryn-Weenink
/// range: a `0.04` s Hann frame, `0.01` s CPPS step, F0 search from `60` to
/// `300` Hz, and a regression interval from `1` to `20` ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CppParams {
    /// CPP analysis-frame length in seconds.
    pub frame_length_seconds: f64,
    /// CPPS frame step in seconds.
    pub time_step: f64,
    /// Lowest searched F0 in hertz.
    pub min_f0_hz: f64,
    /// Highest searched F0 in hertz.
    pub max_f0_hz: f64,
    /// Regression-line start quefrency in seconds.
    pub regression_start_seconds: f64,
    /// Regression-line end quefrency in seconds.
    pub regression_end_seconds: f64,
    /// Moving-average smoothing width in quefrency bins for CPPS.
    pub quefrency_smoothing_bins: usize,
}

impl Default for CppParams {
    fn default() -> Self {
        Self {
            frame_length_seconds: 0.04,
            time_step: 0.01,
            min_f0_hz: 60.0,
            max_f0_hz: 300.0,
            regression_start_seconds: 0.001,
            regression_end_seconds: 0.020,
            quefrency_smoothing_bins: 5,
        }
    }
}

/// A local spectrum slice for spectral moments.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectrumSlice {
    /// Frequency-bin centres in hertz.
    pub frequencies_hz: Vec<f64>,
    /// Linear non-negative spectral magnitudes or powers.
    pub values: Vec<f64>,
}

/// Power-weighted spectral moments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moments {
    /// Power-weighted centre of gravity in hertz.
    pub centre_of_gravity_hz: Option<f64>,
    /// Power-weighted standard deviation in hertz.
    pub standard_deviation_hz: Option<f64>,
    /// Power-weighted skewness.
    pub skewness: Option<f64>,
    /// Power-weighted kurtosis.
    pub kurtosis: Option<f64>,
}

/// Pitch summary embedded in a voice report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchSummary {
    /// Mean voiced F0 in hertz.
    pub mean_hz: Option<f64>,
    /// Median voiced F0 in hertz.
    pub median_hz: Option<f64>,
    /// Minimum voiced F0 in hertz.
    pub min_hz: Option<f64>,
    /// Maximum voiced F0 in hertz.
    pub max_hz: Option<f64>,
}

/// Jitter family values embedded in a voice report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JitterMeasures {
    /// Local jitter.
    pub local: Option<f64>,
    /// Local absolute jitter in seconds.
    pub local_absolute: Option<f64>,
    /// Relative average perturbation.
    pub rap: Option<f64>,
    /// Five-point period perturbation quotient.
    pub ppq5: Option<f64>,
    /// Difference of differences of periods.
    pub ddp: Option<f64>,
}

/// Shimmer family values embedded in a voice report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShimmerMeasures {
    /// Local shimmer.
    pub local: Option<f64>,
    /// Local shimmer in decibels.
    pub local_db: Option<f64>,
    /// Three-point amplitude perturbation quotient.
    pub apq3: Option<f64>,
    /// Five-point amplitude perturbation quotient.
    pub apq5: Option<f64>,
    /// Eleven-point amplitude perturbation quotient.
    pub apq11: Option<f64>,
    /// Three times APQ3.
    pub dda: Option<f64>,
}

/// Voice-break summary embedded in a voice report.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceBreaks {
    /// Gap threshold in seconds.
    pub threshold_seconds: f64,
    /// Gaps between consecutive pulses whose duration exceeds the threshold.
    pub gaps: Vec<TimeSpan>,
    /// Total duration of reported gaps in seconds.
    pub total_seconds: f64,
}

/// Aggregate voice-quality report.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceReport {
    /// Requested report span.
    pub span: TimeSpan,
    /// Pitch parameters used for internal pitch tracking.
    pub pitch_params: PitchParams,
    /// Pulse extraction parameters used for the report.
    pub pulse_params: PulseParams,
    /// Harmonicity parameters used for the report.
    pub harmonicity_params: HarmonicityParams,
    /// CPP parameters used for the report.
    pub cpp_params: CppParams,
    /// Pitch summary over the requested span.
    pub pitch: PitchSummary,
    /// Extracted glottal-pulse process.
    pub pulses: PointProcess,
    /// Jitter family over the requested span.
    pub jitter: JitterMeasures,
    /// Shimmer family over the requested span.
    pub shimmer: ShimmerMeasures,
    /// Mean HNR over the requested span.
    pub mean_hnr_db: Option<f64>,
    /// CPP at the span midpoint.
    pub cpp_db: f64,
    /// Smoothed CPP over the requested span.
    pub cpps_db: Option<f64>,
    /// Voice breaks from pulse gaps greater than `1.25 / pitch_floor`.
    pub voice_breaks: VoiceBreaks,
}

/// Extracts glottal pulses by chaining positive waveform peaks through voiced pitch spans.
///
/// For each contiguous voiced span, the strongest pitch frame seeds a pulse
/// chain. Pulses advance and retreat by the local pitch period, each predicted
/// time is aligned to the largest positive waveform sample within
/// `peak_search_radius_periods` periods, and spacings outside
/// `[min_period_factor, max_period_factor]` times the local period are rejected.
#[must_use]
pub fn pulses(audio: AudioView<'_>, pitch: &PitchTrack, params: &PulseParams) -> PointProcess {
    if audio.frames() == 0 || pitch.frames().is_empty() {
        return PointProcess::new(Vec::new());
    }
    let signal = mono_as_f64(&audio);
    let sample_rate = audio.sample_rate();
    let segments = voiced_segments(pitch, audio.duration());
    let mut times = Vec::new();

    for segment in segments {
        let Some(seed) = strongest_frame_in_span(pitch, segment, params.min_seed_strength) else {
            continue;
        };
        let Some(seed_period) = local_period(pitch, seed.time) else {
            continue;
        };
        let Some(seed_time) = align_positive_peak(
            &signal,
            sample_rate,
            seed.time,
            seed_period,
            params.peak_search_radius_periods,
        ) else {
            continue;
        };
        times.push(seed_time);
        let context = PulseChainContext {
            signal: &signal,
            sample_rate,
            pitch,
            params,
            segment,
        };
        chain_pulses(ChainDirection::Forward, context, seed_time, &mut times);
        chain_pulses(ChainDirection::Backward, context, seed_time, &mut times);
    }

    PointProcess::new(times)
}

/// Computes a jitter measure over pulse periods in `span`.
#[must_use]
pub fn jitter(pp: &PointProcess, span: TimeSpan, kind: JitterKind) -> Option<f64> {
    let periods = periods_in_span(pp, span);
    let mean_period = positive_mean(&periods)?;
    match kind {
        JitterKind::Local => adjacent_average_abs_diff(&periods).map(|value| value / mean_period),
        JitterKind::LocalAbsolute => adjacent_average_abs_diff(&periods),
        JitterKind::Rap => perturbation_quotient(&periods, 3).map(|value| value / mean_period),
        JitterKind::Ppq5 => perturbation_quotient(&periods, 5).map(|value| value / mean_period),
        JitterKind::Ddp => second_difference_average(&periods).map(|value| value / mean_period),
    }
}

/// Computes a shimmer measure from per-period peak amplitudes in `span`.
#[must_use]
pub fn shimmer(
    audio: AudioView<'_>,
    pp: &PointProcess,
    span: TimeSpan,
    kind: ShimmerKind,
) -> Option<f64> {
    let amplitudes = amplitudes_in_span(audio, pp, span);
    let mean_amplitude = positive_mean(&amplitudes)?;
    match kind {
        ShimmerKind::Local => {
            adjacent_average_abs_diff(&amplitudes).map(|value| value / mean_amplitude)
        }
        ShimmerKind::LocalDb => local_db_shimmer(&amplitudes),
        ShimmerKind::Apq3 => {
            perturbation_quotient(&amplitudes, 3).map(|value| value / mean_amplitude)
        }
        ShimmerKind::Apq5 => {
            perturbation_quotient(&amplitudes, 5).map(|value| value / mean_amplitude)
        }
        ShimmerKind::Apq11 => {
            perturbation_quotient(&amplitudes, 11).map(|value| value / mean_amplitude)
        }
        ShimmerKind::Dda => {
            perturbation_quotient(&amplitudes, 3).map(|value| 3.0 * value / mean_amplitude)
        }
    }
}

/// Computes a harmonicity track with Boersma-style Hanning-window corrected ACF peaks.
#[must_use]
pub fn hnr_track(audio: AudioView<'_>, params: &HarmonicityParams) -> HnrTrack {
    if !valid_hnr_params(params) || audio.frames() == 0 {
        return HnrTrack {
            frames: Vec::new(),
            params: *params,
        };
    }

    let signal = mono_as_f64(&audio);
    let global_peak = signal.iter().copied().map(f64::abs).fold(0.0, f64::max);
    let window_seconds = params.periods_per_window / params.floor_hz;
    let grid = FrameGrid::new(audio.duration(), window_seconds, params.time_step);
    let mut plan = RealFftPlan::new();
    let frames = grid
        .centers()
        .map(|time| {
            hnr_frame(
                time,
                &signal,
                audio.sample_rate(),
                global_peak,
                params,
                &mut plan,
            )
        })
        .collect();

    HnrTrack {
        frames,
        params: *params,
    }
}

/// Computes CPP at one time in seconds.
///
/// The return value is finite. It is `0.0` when the frame cannot support the
/// requested quefrency and regression ranges.
#[must_use]
pub fn cpp(audio: AudioView<'_>, at: f64, params: &CppParams) -> f64 {
    let signal = mono_as_f64(&audio);
    let mut plan = RealFftPlan::new();
    cpp_from_frame(&signal, audio.sample_rate(), at, params, &mut plan).unwrap_or(0.0)
}

/// Computes smoothed CPP across a span by averaging frame cepstra, smoothing
/// across quefrency, and measuring the cepstral peak over the configured F0 range.
#[must_use]
pub fn cpps(audio: AudioView<'_>, span: TimeSpan, params: &CppParams) -> Option<f64> {
    if !valid_cpp_params(params) || span.end <= span.start {
        return None;
    }
    let signal = mono_as_f64(&audio);
    let grid = FrameGrid::new(
        audio.duration(),
        params.frame_length_seconds,
        params.time_step,
    );
    let mut plan = RealFftPlan::new();
    let mut cepstra = Vec::new();
    for time in grid.centers().filter(|&time| span.contains(time)) {
        if let Some(cepstrum) = cepstrum_at(&signal, audio.sample_rate(), time, params, &mut plan) {
            cepstra.push(cepstrum);
        }
    }
    if cepstra.len() < 2 {
        return None;
    }
    let len = cepstra.iter().map(Vec::len).min()?;
    if len == 0 {
        return None;
    }
    let mut averaged = vec![0.0; len];
    for cepstrum in &cepstra {
        for (dst, value) in averaged.iter_mut().zip(cepstrum) {
            *dst += *value;
        }
    }
    let scale = 1.0 / cepstra.len() as f64;
    for value in &mut averaged {
        *value *= scale;
    }
    smooth_in_place(&mut averaged, params.quefrency_smoothing_bins);
    cpp_from_cepstrum(&averaged, audio.sample_rate(), params)
}

/// Computes power-weighted spectral moments.
#[must_use]
pub fn spectral_moments(slice: &SpectrumSlice, power: f64) -> Moments {
    if !power.is_finite()
        || power <= 0.0
        || slice.frequencies_hz.len() != slice.values.len()
        || slice.frequencies_hz.is_empty()
    {
        return empty_moments();
    }

    let mut weight_sum = 0.0;
    let mut first = 0.0;
    for (&frequency, &value) in slice.frequencies_hz.iter().zip(&slice.values) {
        if !frequency.is_finite() || !value.is_finite() || value < 0.0 {
            continue;
        }
        let weight = value.powf(power);
        weight_sum += weight;
        first += weight * frequency;
    }
    if weight_sum <= 0.0 {
        return empty_moments();
    }
    let centre = first / weight_sum;
    let mut m2 = 0.0;
    let mut m3 = 0.0;
    let mut m4 = 0.0;
    for (&frequency, &value) in slice.frequencies_hz.iter().zip(&slice.values) {
        if !frequency.is_finite() || !value.is_finite() || value < 0.0 {
            continue;
        }
        let weight = value.powf(power);
        let delta = frequency - centre;
        let delta2 = delta * delta;
        m2 += weight * delta2;
        m3 += weight * delta2 * delta;
        m4 += weight * delta2 * delta2;
    }
    let variance = m2 / weight_sum;
    let standard_deviation = variance.sqrt();
    let (skewness, kurtosis) = if standard_deviation > 0.0 {
        (
            Some((m3 / weight_sum) / standard_deviation.powi(3)),
            Some((m4 / weight_sum) / standard_deviation.powi(4)),
        )
    } else {
        (None, None)
    };
    Moments {
        centre_of_gravity_hz: Some(centre),
        standard_deviation_hz: Some(standard_deviation),
        skewness,
        kurtosis,
    }
}

/// Computes an aggregate voice-quality report over `span`.
#[must_use]
pub fn voice_report(
    audio: AudioView<'_>,
    span: TimeSpan,
    pitch_params: &PitchParams,
) -> VoiceReport {
    let pitch = pitch_track(audio.clone(), pitch_params);
    let pulse_params = PulseParams::default();
    let harmonicity_params = HarmonicityParams {
        floor_hz: pitch_params.floor_hz,
        time_step: pitch_params
            .time_step
            .unwrap_or(HarmonicityParams::default().time_step),
        ..HarmonicityParams::default()
    };
    let cpp_params = CppParams::default();
    let pp = pulses(audio.clone(), &pitch, &pulse_params);
    let hnr = hnr_track(audio.clone(), &harmonicity_params);
    let midpoint = 0.5 * (span.start + span.end);

    VoiceReport {
        span,
        pitch_params: pitch_params.clone(),
        pulse_params,
        harmonicity_params,
        cpp_params,
        pitch: PitchSummary {
            mean_hz: pitch.mean_hz(span),
            median_hz: pitch.median_hz(span),
            min_hz: pitch.min_hz(span),
            max_hz: pitch.max_hz(span),
        },
        jitter: JitterMeasures {
            local: jitter(&pp, span, JitterKind::Local),
            local_absolute: jitter(&pp, span, JitterKind::LocalAbsolute),
            rap: jitter(&pp, span, JitterKind::Rap),
            ppq5: jitter(&pp, span, JitterKind::Ppq5),
            ddp: jitter(&pp, span, JitterKind::Ddp),
        },
        shimmer: ShimmerMeasures {
            local: shimmer(audio.clone(), &pp, span, ShimmerKind::Local),
            local_db: shimmer(audio.clone(), &pp, span, ShimmerKind::LocalDb),
            apq3: shimmer(audio.clone(), &pp, span, ShimmerKind::Apq3),
            apq5: shimmer(audio.clone(), &pp, span, ShimmerKind::Apq5),
            apq11: shimmer(audio.clone(), &pp, span, ShimmerKind::Apq11),
            dda: shimmer(audio.clone(), &pp, span, ShimmerKind::Dda),
        },
        mean_hnr_db: hnr.mean_db(span),
        cpp_db: cpp(audio.clone(), midpoint, &cpp_params),
        cpps_db: cpps(audio, span, &cpp_params),
        voice_breaks: voice_breaks(&pp, span, 1.25 / pitch_params.floor_hz),
        pulses: pp,
    }
}

#[derive(Debug, Clone, Copy)]
struct VoicedSegment {
    start: f64,
    end: f64,
}

#[derive(Debug, Clone, Copy)]
enum ChainDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy)]
struct PulseChainContext<'a> {
    signal: &'a [f64],
    sample_rate: f64,
    pitch: &'a PitchTrack,
    params: &'a PulseParams,
    segment: VoicedSegment,
}

fn mono_as_f64(audio: &AudioView<'_>) -> Vec<f64> {
    match audio.mono_mix() {
        Cow::Borrowed(samples) => samples.iter().map(|&sample| f64::from(sample)).collect(),
        Cow::Owned(samples) => samples.into_iter().map(f64::from).collect(),
    }
}

fn voiced_segments(pitch: &PitchTrack, duration: f64) -> Vec<VoicedSegment> {
    let frames = pitch.frames();
    if frames.is_empty() {
        return Vec::new();
    }
    let mut segments = Vec::new();
    let mut start = None;
    for (i, frame) in frames.iter().enumerate() {
        if frame.f0.is_some() && start.is_none() {
            let left = if i == 0 {
                0.0
            } else {
                0.5 * (frames[i - 1].time + frame.time)
            };
            start = Some(left.max(0.0));
        }
        if frame.f0.is_none() && start.is_some() {
            let right = if i == 0 {
                frame.time
            } else {
                0.5 * (frames[i - 1].time + frame.time)
            };
            segments.push(VoicedSegment {
                start: start.take().unwrap_or(0.0),
                end: right.min(duration),
            });
        }
    }
    if let Some(start) = start {
        segments.push(VoicedSegment {
            start,
            end: duration,
        });
    }
    segments.retain(|segment| segment.end > segment.start);
    segments
}

fn strongest_frame_in_span(
    pitch: &PitchTrack,
    segment: VoicedSegment,
    min_strength: f64,
) -> Option<&phx_pitch::PitchFrame> {
    pitch
        .frames()
        .iter()
        .filter(|frame| segment.start <= frame.time && frame.time <= segment.end)
        .filter(|frame| frame.f0.is_some() && frame.strength >= min_strength)
        .max_by(|a, b| a.strength.total_cmp(&b.strength))
}

fn local_period(pitch: &PitchTrack, time: f64) -> Option<f64> {
    pitch
        .frames()
        .iter()
        .filter_map(|frame| frame.f0.map(|f0| (frame.time, f0)))
        .filter(|(_, f0)| f0.is_finite() && *f0 > 0.0)
        .min_by(|(time_a, _), (time_b, _)| (time_a - time).abs().total_cmp(&(time_b - time).abs()))
        .map(|(_, f0)| 1.0 / f0)
}

fn align_positive_peak(
    signal: &[f64],
    sample_rate: f64,
    predicted_time: f64,
    period: f64,
    radius_periods: f64,
) -> Option<f64> {
    if signal.is_empty() || !predicted_time.is_finite() || period <= 0.0 || radius_periods < 0.0 {
        return None;
    }
    let centre = (predicted_time * sample_rate).round() as isize;
    let radius = (radius_periods * period * sample_rate).round().max(1.0) as isize;
    let start = (centre - radius).max(0) as usize;
    let end = (centre + radius + 1).min(signal.len() as isize) as usize;
    if start >= end {
        return None;
    }
    let (offset, _) = signal[start..end]
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))?;
    Some((start + offset) as f64 / sample_rate)
}

fn chain_pulses(
    direction: ChainDirection,
    context: PulseChainContext<'_>,
    seed_time: f64,
    times: &mut Vec<f64>,
) {
    let mut current = seed_time;
    while let Some(period) = local_period(context.pitch, current) {
        let predicted = match direction {
            ChainDirection::Forward => current + period,
            ChainDirection::Backward => current - period,
        };
        if predicted < context.segment.start || predicted > context.segment.end {
            break;
        }
        let Some(aligned) = align_positive_peak(
            context.signal,
            context.sample_rate,
            predicted,
            period,
            context.params.peak_search_radius_periods,
        ) else {
            break;
        };
        let spacing = (aligned - current).abs();
        if spacing < context.params.min_period_factor * period
            || spacing > context.params.max_period_factor * period
        {
            break;
        }
        times.push(aligned);
        current = aligned;
    }
}

fn periods_in_span(pp: &PointProcess, span: TimeSpan) -> Vec<f64> {
    pp.times
        .windows(2)
        .filter_map(|pair| {
            let start = pair[0];
            let end = pair[1];
            (span.contains(start) && span.contains(end) && end > start).then_some(end - start)
        })
        .collect()
}

fn positive_mean(values: &[f64]) -> Option<f64> {
    if values.is_empty()
        || values
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

fn adjacent_average_abs_diff(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    Some(
        values
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .sum::<f64>()
            / (values.len() - 1) as f64,
    )
}

fn perturbation_quotient(values: &[f64], width: usize) -> Option<f64> {
    if width.is_multiple_of(2) || values.len() < width {
        return None;
    }
    let half = width / 2;
    let count = values.len() - 2 * half;
    let mut sum = 0.0;
    for i in half..values.len() - half {
        let local_mean = values[i - half..=i + half].iter().sum::<f64>() / width as f64;
        sum += (values[i] - local_mean).abs();
    }
    Some(sum / count as f64)
}

fn second_difference_average(values: &[f64]) -> Option<f64> {
    if values.len() < 3 {
        return None;
    }
    Some(
        values
            .windows(3)
            .map(|triple| (triple[2] - 2.0 * triple[1] + triple[0]).abs())
            .sum::<f64>()
            / (values.len() - 2) as f64,
    )
}

fn amplitudes_in_span(audio: AudioView<'_>, pp: &PointProcess, span: TimeSpan) -> Vec<f64> {
    let signal = mono_as_f64(&audio);
    let sample_rate = audio.sample_rate();
    pp.times
        .windows(2)
        .filter_map(|pair| {
            let start = pair[0];
            let end = pair[1];
            if !(span.contains(start) && span.contains(end) && end > start) {
                return None;
            }
            let start_sample = (start * sample_rate).floor().max(0.0) as usize;
            let end_sample = (end * sample_rate).ceil().min(signal.len() as f64) as usize;
            (start_sample < end_sample).then(|| {
                signal[start_sample..end_sample]
                    .iter()
                    .copied()
                    .map(f64::abs)
                    .fold(0.0, f64::max)
            })
        })
        .filter(|amplitude| *amplitude > 0.0)
        .collect()
}

fn local_db_shimmer(amplitudes: &[f64]) -> Option<f64> {
    if amplitudes.len() < 2 || amplitudes.iter().any(|value| *value <= 0.0) {
        return None;
    }
    Some(
        amplitudes
            .windows(2)
            .map(|pair| (20.0 * (pair[1] / pair[0]).log10()).abs())
            .sum::<f64>()
            / (amplitudes.len() - 1) as f64,
    )
}

fn valid_hnr_params(params: &HarmonicityParams) -> bool {
    params.time_step.is_finite()
        && params.time_step > 0.0
        && params.floor_hz.is_finite()
        && params.floor_hz > 0.0
        && params.ceiling_hz.is_finite()
        && params.ceiling_hz > params.floor_hz
        && params.silence_threshold.is_finite()
        && params.silence_threshold >= 0.0
        && params.periods_per_window.is_finite()
        && params.periods_per_window > 0.0
}

fn hnr_frame(
    time: f64,
    signal: &[f64],
    sample_rate: f64,
    global_peak: f64,
    params: &HarmonicityParams,
    plan: &mut RealFftPlan,
) -> HnrFrame {
    let window_len = (params.periods_per_window * sample_rate / params.floor_hz)
        .round()
        .max(3.0) as usize;
    let frame = centered_frame(signal, sample_rate, time, window_len);
    let frame_peak = frame.iter().copied().map(f64::abs).fold(0.0, f64::max);
    if global_peak <= 0.0 || frame_peak < params.silence_threshold * global_peak {
        return HnrFrame {
            time,
            hnr_db: None,
            periodic_fraction: None,
        };
    }
    let window = window_samples(Window::Hanning, window_len);
    let windowed = frame
        .iter()
        .zip(&window)
        .map(|(sample, weight)| sample * weight)
        .collect::<Vec<_>>();
    let raw = autocorrelation_fft(&windowed, plan);
    let window_acf = autocorrelation_direct(&window);
    let min_lag = (sample_rate / params.ceiling_hz).floor().max(1.0) as usize;
    let max_lag = (sample_rate / params.floor_hz)
        .ceil()
        .min((window_len - 1) as f64) as usize;
    let periodic_fraction =
        corrected_acf_peak(&raw, &window_acf, min_lag, max_lag).filter(|r| *r > 0.0 && *r < 1.0);
    HnrFrame {
        time,
        hnr_db: periodic_fraction.map(|r| 10.0 * (r / (1.0 - r)).log10()),
        periodic_fraction,
    }
}

fn centered_frame(signal: &[f64], sample_rate: f64, time: f64, len: usize) -> Vec<f64> {
    let centre = (time * sample_rate).round() as isize;
    let half = (len / 2) as isize;
    (0..len)
        .map(|i| {
            let index = centre - half + i as isize;
            if index < 0 || index >= signal.len() as isize {
                0.0
            } else {
                signal[index as usize]
            }
        })
        .collect()
}

fn autocorrelation_fft(input: &[f64], plan: &mut RealFftPlan) -> Vec<f64> {
    let fft_len = next_pow2(input.len() * 2);
    let mut padded = vec![0.0; fft_len];
    padded[..input.len()].copy_from_slice(input);
    let mut spectrum = plan.rfft(&mut padded);
    for bin in &mut spectrum {
        let power = bin.re * bin.re + bin.im * bin.im;
        bin.re = power;
        bin.im = 0.0;
    }
    let acf = plan.irfft(&mut spectrum, fft_len);
    acf.into_iter()
        .take(input.len())
        .map(|value| value / fft_len as f64)
        .collect()
}

fn autocorrelation_direct(input: &[f64]) -> Vec<f64> {
    (0..input.len())
        .map(|lag| {
            input
                .iter()
                .zip(&input[lag..])
                .map(|(a, b)| a * b)
                .sum::<f64>()
        })
        .collect()
}

fn corrected_acf_peak(
    raw: &[f64],
    window_acf: &[f64],
    min_lag: usize,
    max_lag: usize,
) -> Option<f64> {
    if raw.is_empty()
        || window_acf.is_empty()
        || raw[0] <= 0.0
        || window_acf[0] <= 0.0
        || min_lag > max_lag
        || max_lag >= raw.len()
        || max_lag >= window_acf.len()
    {
        return None;
    }
    let corrected = (0..=max_lag)
        .map(|lag| {
            let denom = window_acf[lag] / window_acf[0];
            if denom > 0.0 {
                (raw[lag] / raw[0]) / denom
            } else {
                f64::NEG_INFINITY
            }
        })
        .collect::<Vec<_>>();
    let peak_lag = (min_lag..=max_lag).max_by(|&a, &b| corrected[a].total_cmp(&corrected[b]))?;
    let peak = if peak_lag > min_lag && peak_lag < max_lag {
        let (_, value) = sinc_interpolate_max(&corrected, peak_lag, 3);
        value
    } else {
        corrected[peak_lag]
    };
    Some(peak.clamp(0.0, 1.0 - 1e-9))
}

fn valid_cpp_params(params: &CppParams) -> bool {
    params.frame_length_seconds.is_finite()
        && params.frame_length_seconds > 0.0
        && params.time_step.is_finite()
        && params.time_step > 0.0
        && params.min_f0_hz.is_finite()
        && params.min_f0_hz > 0.0
        && params.max_f0_hz.is_finite()
        && params.max_f0_hz > params.min_f0_hz
        && params.regression_start_seconds.is_finite()
        && params.regression_start_seconds >= 0.0
        && params.regression_end_seconds.is_finite()
        && params.regression_end_seconds > params.regression_start_seconds
}

fn cpp_from_frame(
    signal: &[f64],
    sample_rate: f64,
    at: f64,
    params: &CppParams,
    plan: &mut RealFftPlan,
) -> Option<f64> {
    let cepstrum = cepstrum_at(signal, sample_rate, at, params, plan)?;
    cpp_from_cepstrum(&cepstrum, sample_rate, params)
}

fn cepstrum_at(
    signal: &[f64],
    sample_rate: f64,
    at: f64,
    params: &CppParams,
    plan: &mut RealFftPlan,
) -> Option<Vec<f64>> {
    if !valid_cpp_params(params) || signal.is_empty() || !at.is_finite() {
        return None;
    }
    let frame_len = (params.frame_length_seconds * sample_rate).round().max(3.0) as usize;
    let frame = centered_frame(signal, sample_rate, at, frame_len);
    let window = window_samples(Window::Hanning, frame_len);
    let fft_len = next_pow2(frame_len * 2);
    let mut padded = vec![0.0; fft_len];
    let mean = frame.iter().sum::<f64>() / frame.len() as f64;
    for (dst, (sample, weight)) in padded.iter_mut().zip(frame.iter().zip(&window)) {
        *dst = (sample - mean) * weight;
    }
    let mut spectrum = plan.rfft(&mut padded);
    for bin in &mut spectrum {
        let magnitude = bin.norm().max(EPSILON);
        bin.re = 20.0 * magnitude.log10();
        bin.im = 0.0;
    }
    let cepstrum = plan.irfft(&mut spectrum, fft_len);
    Some(
        cepstrum
            .into_iter()
            .map(|value| value / fft_len as f64)
            .collect(),
    )
}

fn cpp_from_cepstrum(cepstrum: &[f64], sample_rate: f64, params: &CppParams) -> Option<f64> {
    let min_quefrency = 1.0 / params.max_f0_hz;
    let max_quefrency = 1.0 / params.min_f0_hz;
    let min_bin = (min_quefrency * sample_rate).ceil() as usize;
    let max_bin = ((max_quefrency * sample_rate).floor() as usize).min(cepstrum.len() - 1);
    if min_bin > max_bin {
        return None;
    }
    let peak_bin = (min_bin..=max_bin).max_by(|&a, &b| cepstrum[a].total_cmp(&cepstrum[b]))?;
    let (slope, intercept) = cepstral_regression(cepstrum, sample_rate, params)?;
    let quefrency = peak_bin as f64 / sample_rate;
    Some((cepstrum[peak_bin] - (slope * quefrency + intercept)).max(0.0))
}

fn cepstral_regression(
    cepstrum: &[f64],
    sample_rate: f64,
    params: &CppParams,
) -> Option<(f64, f64)> {
    let start = (params.regression_start_seconds * sample_rate).ceil() as usize;
    let end = ((params.regression_end_seconds * sample_rate).floor() as usize)
        .min(cepstrum.len().saturating_sub(1));
    if start >= end {
        return None;
    }
    let mut n = 0.0;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_xy = 0.0;
    for (bin, &y) in cepstrum.iter().enumerate().take(end + 1).skip(start) {
        let x = bin as f64 / sample_rate;
        n += 1.0;
        sum_x += x;
        sum_y += y;
        sum_xx += x * x;
        sum_xy += x * y;
    }
    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() <= EPSILON {
        return None;
    }
    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;
    Some((slope, intercept))
}

fn smooth_in_place(values: &mut [f64], width: usize) {
    if width <= 1 || values.is_empty() {
        return;
    }
    let half = width / 2;
    let original = values.to_vec();
    for (i, value) in values.iter_mut().enumerate() {
        let start = i.saturating_sub(half);
        let end = (i + half + 1).min(original.len());
        *value = original[start..end].iter().sum::<f64>() / (end - start) as f64;
    }
}

fn empty_moments() -> Moments {
    Moments {
        centre_of_gravity_hz: None,
        standard_deviation_hz: None,
        skewness: None,
        kurtosis: None,
    }
}

fn mean_option(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut count = 0usize;
    let mut sum = 0.0;
    for value in values {
        count += 1;
        sum += value;
    }
    (count > 0).then_some(sum / count as f64)
}

fn voice_breaks(pp: &PointProcess, span: TimeSpan, threshold_seconds: f64) -> VoiceBreaks {
    let gaps = pp
        .times
        .windows(2)
        .filter_map(|pair| {
            let start = pair[0];
            let end = pair[1];
            (span.contains(start) && span.contains(end) && end - start > threshold_seconds)
                .then_some(TimeSpan::new(start, end))
        })
        .collect::<Vec<_>>();
    let total_seconds = gaps.iter().map(|gap| gap.end - gap.start).sum();
    VoiceBreaks {
        threshold_seconds,
        gaps,
        total_seconds,
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use phx_audio::Audio;

    use super::*;

    fn audio_from_signal(signal: Vec<f32>, sample_rate: f64) -> Audio {
        Audio::new(vec![signal], sample_rate).expect("valid synthetic audio")
    }

    fn assert_relative_close(actual: f64, expected: f64, tolerance: f64) {
        let relative = (actual - expected).abs() / expected.abs().max(EPSILON);
        assert!(
            relative <= tolerance,
            "got {actual}, want {expected}, relative error {relative}"
        );
    }

    #[test]
    fn jitter_recovers_injected_local_period_perturbation() {
        let base_period = 0.01;
        let perturbation = 0.04;
        let periods = (0usize..80)
            .map(|i| {
                if i.is_multiple_of(2) {
                    base_period * (1.0 - perturbation * 0.5)
                } else {
                    base_period * (1.0 + perturbation * 0.5)
                }
            })
            .collect::<Vec<_>>();
        let mut times = vec![0.0];
        for period in &periods {
            times.push(times.last().copied().unwrap_or(0.0) + period);
        }
        let pp = PointProcess::new(times);
        let actual = jitter(&pp, TimeSpan::new(0.0, 1.0), JitterKind::Local).unwrap();
        assert_relative_close(actual, perturbation, 0.05);
    }

    #[test]
    fn shimmer_recovers_injected_local_amplitude_perturbation() {
        let sample_rate = 48_000.0;
        let period = 0.01;
        let perturbation = 0.10;
        let periods = 60usize;
        let samples_per_period = (period * sample_rate) as usize;
        let mut signal = vec![0.0_f32; periods * samples_per_period];
        let mut times = Vec::with_capacity(periods + 1);
        for i in 0..=periods {
            times.push(i as f64 * period);
        }
        for i in 0..periods {
            let amplitude = if i.is_multiple_of(2) {
                1.0 - perturbation * 0.5
            } else {
                1.0 + perturbation * 0.5
            };
            let start = i * samples_per_period;
            for j in 0..samples_per_period {
                let phase = 2.0 * PI * j as f64 / samples_per_period as f64;
                signal[start + j] = (amplitude * phase.sin()) as f32;
            }
        }
        let audio = audio_from_signal(signal, sample_rate);
        let pp = PointProcess::new(times);
        let actual = shimmer(
            audio.slice_samples(0..audio.frames()),
            &pp,
            TimeSpan::new(0.0, audio.duration()),
            ShimmerKind::Local,
        )
        .unwrap();
        assert_relative_close(actual, perturbation, 0.05);
    }

    #[test]
    fn hnr_recovers_constructed_harmonic_noise_ratio() {
        let sample_rate = 48_000.0;
        let duration = 1.0;
        let f0 = 160.0;
        let target_hnr_db = 15.0;
        let periodic_rms = 1.0 / 2.0_f64.sqrt();
        let noise_rms = periodic_rms / 10.0_f64.powf(target_hnr_db / 20.0);
        let n = (duration * sample_rate) as usize;
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut noise = Vec::with_capacity(n);
        for _ in 0..n {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let unit = (seed >> 11) as f64 / ((1_u64 << 53) as f64);
            noise.push(2.0 * unit - 1.0);
        }
        let raw_rms = (noise.iter().map(|value| value * value).sum::<f64>() / n as f64).sqrt();
        let noise_scale = noise_rms / raw_rms;
        let signal = (0..n)
            .map(|i| {
                let t = i as f64 / sample_rate;
                let harmonic = (2.0 * PI * f0 * t).sin();
                (harmonic + noise_scale * noise[i]) as f32
            })
            .collect::<Vec<_>>();
        let audio = audio_from_signal(signal, sample_rate);
        let params = HarmonicityParams {
            floor_hz: 75.0,
            ceiling_hz: 300.0,
            ..HarmonicityParams::default()
        };
        let track = hnr_track(audio.slice_samples(0..audio.frames()), &params);
        let actual = track.mean_db(TimeSpan::new(0.2, 0.8)).unwrap();
        assert!(
            (actual - target_hnr_db).abs() <= 0.5,
            "got {actual}, want {target_hnr_db}"
        );
    }

    #[test]
    fn spectral_moments_are_power_weighted() {
        let slice = SpectrumSlice {
            frequencies_hz: vec![100.0, 200.0, 300.0],
            values: vec![1.0, 2.0, 1.0],
        };
        let moments = spectral_moments(&slice, 2.0);
        assert_eq!(moments.centre_of_gravity_hz, Some(200.0));
        assert!(moments.standard_deviation_hz.unwrap() > 0.0);
        assert!(moments.skewness.unwrap().abs() < 1e-12);
    }

    #[test]
    fn absent_period_measures_return_none() {
        let pp = PointProcess::new(vec![0.0, 0.01]);
        assert_eq!(
            jitter(&pp, TimeSpan::new(0.0, 0.02), JitterKind::Local),
            None
        );
    }
}
