use std::f64::consts::PI;

use phx_dsp::{RealFftPlan, next_pow2};

pub(crate) const WINDOW_ACF_EPSILON: f64 = 1e-9;

pub(crate) fn soft_lowpass(signal: &[f32], sample_rate: f64, plan: &mut RealFftPlan) -> Vec<f64> {
    let mut filtered: Vec<f64> = signal.iter().map(|&sample| f64::from(sample)).collect();
    if filtered.len() < 2 {
        return filtered;
    }

    let len = filtered.len();
    let n = next_pow2(len);
    filtered.resize(n, 0.0);
    let mut spectrum = plan.rfft(&mut filtered);
    let nyquist = 0.5 * sample_rate;
    let taper_start = 0.95 * nyquist;
    let width = nyquist - taper_start;

    // §1.2 step 1 specifies a qualitative near-Nyquist soft lowpass; use a
    // raised-cosine taper over the top five percent of the spectrum.
    for (bin, value) in spectrum.iter_mut().enumerate() {
        let frequency = bin as f64 * sample_rate / n as f64;
        if frequency > taper_start {
            let x = ((frequency - taper_start) / width).clamp(0.0, 1.0);
            let scale = 0.5 * (1.0 + (PI * x).cos());
            *value *= scale;
        }
    }

    let mut out = plan.irfft(&mut spectrum, n);
    out.truncate(len);
    for sample in &mut out {
        *sample /= n as f64;
    }
    out
}

pub(crate) fn corrected_autocorrelation(
    segment: &[f64],
    window: &[f64],
    physical_window_seconds: f64,
    sample_rate: f64,
    plan: &mut RealFftPlan,
) -> Vec<f64> {
    debug_assert_eq!(segment.len(), window.len());
    if segment.is_empty() {
        return Vec::new();
    }

    let mean = segment.iter().sum::<f64>() / segment.len() as f64;
    let mut buffer: Vec<f64> = segment
        .iter()
        .zip(window)
        .map(|(&sample, &w)| (sample - mean) * w)
        .collect();
    let padded = next_pow2(buffer.len().saturating_mul(3).div_ceil(2));
    buffer.resize(padded, 0.0);

    let mut spectrum = plan.rfft(&mut buffer);
    for bin in &mut spectrum {
        bin.re = bin.norm_sqr();
        bin.im = 0.0;
    }
    let raw = plan.irfft(&mut spectrum, padded);

    let usable = raw.len().min(segment.len());
    let mut corrected = Vec::with_capacity(usable);
    for (lag, &value) in raw.iter().take(usable).enumerate() {
        let tau = lag as f64 / sample_rate;
        let rw = window_autocorrelation(tau, physical_window_seconds);
        if rw.is_finite() && rw.abs() >= WINDOW_ACF_EPSILON {
            corrected.push(value / padded as f64 / rw);
        } else {
            corrected.push(f64::NAN);
        }
    }

    let Some(&zero) = corrected.first() else {
        return corrected;
    };
    if zero.is_finite() && zero.abs() >= WINDOW_ACF_EPSILON {
        for value in &mut corrected {
            *value /= zero;
        }
    }
    corrected
}

pub(crate) fn window_autocorrelation(tau: f64, physical_window_seconds: f64) -> f64 {
    if !tau.is_finite() || !physical_window_seconds.is_finite() || physical_window_seconds <= 0.0 {
        return f64::NAN;
    }
    let abs_tau = tau.abs();
    let ratio = abs_tau / physical_window_seconds;
    let angle = 2.0 * PI * ratio;
    (1.0 - ratio) * (2.0 / 3.0 + (1.0 / 3.0) * angle.cos()) + (1.0 / (2.0 * PI)) * angle.sin()
}
