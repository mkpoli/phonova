//! Band-pass filtering of a finite span by spectral multiplication.
//!
//! A forward real FFT of the zero-padded span, a per-bin gain that is unity
//! inside `[f_low, f_high]` and zero outside, and an inverse FFT back to the
//! time domain. The two band edges are not brick walls: each is smoothed by a
//! half-cosine (Hann) skirt over a fixed frequency interval, the smoothing form
//! Praat's manual documents for "Spectrum: Filter (pass Hz)…"
//! (<https://www.fon.hum.uva.nl/praat/manual/Spectrum__Filter__pass_Hz____.html>).
//! A brick-wall gain multiplies the spectrum by a rectangle whose inverse
//! transform is a sinc, so the filtered signal rings around transients; the
//! skirt tapers that rectangle so the impulse response decays and the ringing
//! is suppressed.
//!
//! The span is padded to a power-of-two transform length so the circular
//! convolution the FFT performs approximates a linear one over the span
//! interior, and the reconstructed span's first and last few milliseconds are
//! tapered to absorb the residual wrap-around at the edges.

use std::f64::consts::PI;

use realfft::num_complex::Complex;

use crate::fft::RealFftPlan;
use crate::next_pow2;

/// Half-cosine skirt width applied at each passband edge, in hertz.
///
/// Each edge rolls from stop to pass over this interval as a raised cosine
/// rather than a hard step, the smoothing Praat's "Spectrum: Filter (pass Hz)…"
/// applies to keep the inverse transform from ringing. The width is a fixed
/// constant here; 100 Hz is narrow enough to keep the passband sharp at speech
/// resolution and wide enough to tame the sinc skirts of a hard cut.
pub const PASS_BAND_SKIRT_HZ: f64 = 100.0;

/// Raised-cosine taper applied to the first and last samples of the filtered
/// span, in seconds.
///
/// The FFT treats the span as one period of a circular signal, so energy that
/// wraps past either end lands as a transient at the other. A 5 ms fade in and
/// out absorbs that wrap-around without touching the span interior.
pub const EDGE_TAPER_S: f64 = 0.005;

/// Passband gain at `freq_hz` for a band-pass over `[f_low, f_high]` with a
/// [`PASS_BAND_SKIRT_HZ`] half-cosine skirt at each edge.
///
/// Unity inside the band, zero beyond a skirt width outside it, and a monotonic
/// raised-cosine transition across each skirt. Frequencies are taken by
/// magnitude, so the gain is even in `freq_hz`.
#[must_use]
pub fn band_pass_gain(freq_hz: f64, f_low: f64, f_high: f64) -> f64 {
    band_pass_gain_with_skirt(freq_hz, f_low, f_high, PASS_BAND_SKIRT_HZ)
}

fn band_pass_gain_with_skirt(freq_hz: f64, f_low: f64, f_high: f64, skirt: f64) -> f64 {
    let f = freq_hz.abs();
    if skirt <= 0.0 {
        return f64::from(f >= f_low && f <= f_high);
    }
    if f < f_low - skirt || f > f_high + skirt {
        0.0
    } else if f < f_low {
        // Rising half-cosine: 0 at `f_low - skirt`, 1 at `f_low`.
        let u = (f - (f_low - skirt)) / skirt;
        0.5 - 0.5 * (PI * u).cos()
    } else if f <= f_high {
        1.0
    } else {
        // Falling half-cosine: 1 at `f_high`, 0 at `f_high + skirt`.
        let u = (f - f_high) / skirt;
        0.5 + 0.5 * (PI * u).cos()
    }
}

/// Band-pass filters `samples` to `[f_low, f_high]` and returns a same-length
/// mono buffer.
///
/// `sample_rate` sets the bin spacing of the transform. `plan` caches the FFT
/// plans so repeated calls at the same padded length — the box-selection replay
/// a frontend fires as a user drags — reuse one plan. The result is
/// deterministic: the same inputs always yield bit-identical output.
///
/// A span of fewer than two samples, or a non-finite non-positive sample rate,
/// is returned unchanged.
#[must_use]
pub fn band_pass_filter(
    plan: &mut RealFftPlan,
    samples: &[f32],
    sample_rate: f64,
    f_low: f64,
    f_high: f64,
) -> Vec<f32> {
    let n = samples.len();
    if n < 2 || !sample_rate.is_finite() || sample_rate <= 0.0 {
        return samples.to_vec();
    }
    let (lo, hi) = (f_low.min(f_high), f_low.max(f_high));

    let fft_len = next_pow2(n);
    let mut buffer = vec![0.0_f64; fft_len];
    for (dst, &sample) in buffer.iter_mut().zip(samples) {
        *dst = f64::from(sample);
    }

    let mut spectrum = plan.rfft(&mut buffer);
    let bin_hz = sample_rate / fft_len as f64;
    for (k, bin) in spectrum.iter_mut().enumerate() {
        let gain = band_pass_gain(k as f64 * bin_hz, lo, hi);
        *bin = scaled(*bin, gain);
    }

    let time = plan.irfft(&mut spectrum, fft_len);
    let scale = 1.0 / fft_len as f64;
    let mut out: Vec<f32> = time[..n].iter().map(|&v| (v * scale) as f32).collect();
    apply_edge_taper(&mut out, sample_rate);
    out
}

fn scaled(bin: Complex<f64>, gain: f64) -> Complex<f64> {
    Complex::new(bin.re * gain, bin.im * gain)
}

fn apply_edge_taper(out: &mut [f32], sample_rate: f64) {
    let n = out.len();
    let taper = ((EDGE_TAPER_S * sample_rate).round() as usize).min(n / 2);
    if taper == 0 {
        return;
    }
    for i in 0..taper {
        // Rising half-cosine over the fade, sampled at bin centres so neither
        // endpoint is exactly zero.
        let u = (i as f64 + 0.5) / taper as f64;
        let weight = 0.5 - 0.5 * (PI * u).cos();
        out[i] = (f64::from(out[i]) * weight) as f32;
        out[n - 1 - i] = (f64::from(out[n - 1 - i]) * weight) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn gain_is_unity_inside_and_zero_outside_the_skirts() {
        let (lo, hi) = (800.0, 1200.0);
        assert!((band_pass_gain(1000.0, lo, hi) - 1.0).abs() < 1e-12);
        assert!((band_pass_gain(lo, lo, hi) - 1.0).abs() < 1e-12);
        assert!((band_pass_gain(hi, lo, hi) - 1.0).abs() < 1e-12);
        assert!(band_pass_gain(lo - PASS_BAND_SKIRT_HZ, lo, hi).abs() < 1e-12);
        assert!(band_pass_gain(hi + PASS_BAND_SKIRT_HZ, lo, hi).abs() < 1e-12);
        assert!(band_pass_gain(200.0, lo, hi).abs() < 1e-12);
        assert!(band_pass_gain(5000.0, lo, hi).abs() < 1e-12);
    }

    #[test]
    fn skirts_are_monotonic_across_each_edge() {
        let (lo, hi) = (800.0, 1200.0);
        let steps = 200;

        // Lower skirt rises monotonically from 0 to 1.
        let mut prev = -1.0;
        for i in 0..=steps {
            let f = (lo - PASS_BAND_SKIRT_HZ) + PASS_BAND_SKIRT_HZ * i as f64 / steps as f64;
            let g = band_pass_gain(f, lo, hi);
            assert!(g >= prev - 1e-12, "lower skirt dipped at {f} Hz");
            prev = g;
        }

        // Upper skirt falls monotonically from 1 to 0.
        let mut prev = 2.0;
        for i in 0..=steps {
            let f = hi + PASS_BAND_SKIRT_HZ * i as f64 / steps as f64;
            let g = band_pass_gain(f, lo, hi);
            assert!(g <= prev + 1e-12, "upper skirt rose at {f} Hz");
            prev = g;
        }
    }

    /// Central RMS in decibels over the taper-free interior of `signal`.
    fn interior_db(signal: &[f32], sample_rate: f64) -> f64 {
        let taper = (EDGE_TAPER_S * sample_rate).round() as usize;
        let inner = &signal[taper..signal.len() - taper];
        let power: f64 = inner
            .iter()
            .map(|&s| f64::from(s) * f64::from(s))
            .sum::<f64>()
            / inner.len() as f64;
        10.0 * power.max(1e-300).log10()
    }

    fn bin_aligned_cosine(bin: usize, len: usize, amplitude: f64) -> Vec<f32> {
        (0..len)
            .map(|j| (amplitude * (TAU * bin as f64 * j as f64 / len as f64).cos()) as f32)
            .collect()
    }

    #[test]
    fn in_band_tone_passes_within_a_tenth_of_a_decibel() {
        let (sr, len) = (16_000.0, 1024);
        // Bin 64 → 1000 Hz at 16 kHz over 1024 samples; band [800, 1200].
        let tone = bin_aligned_cosine(64, len, 0.5);
        let mut plan = RealFftPlan::new();
        let filtered = band_pass_filter(&mut plan, &tone, sr, 800.0, 1200.0);
        let diff = (interior_db(&filtered, sr) - interior_db(&tone, sr)).abs();
        assert!(diff < 0.1, "in-band tone changed by {diff} dB");
    }

    #[test]
    fn out_of_band_tone_is_suppressed_past_sixty_decibels() {
        let (sr, len) = (16_000.0, 1024);
        // Bin 256 → 4000 Hz, well outside band [800, 1200] and its skirts.
        let tone = bin_aligned_cosine(256, len, 0.5);
        let mut plan = RealFftPlan::new();
        let filtered = band_pass_filter(&mut plan, &tone, sr, 800.0, 1200.0);
        let drop = interior_db(&tone, sr) - interior_db(&filtered, sr);
        assert!(drop >= 60.0, "out-of-band tone only dropped {drop} dB");
    }

    #[test]
    fn filtering_is_deterministic() {
        let tone = bin_aligned_cosine(64, 1024, 0.5);
        let mut plan_a = RealFftPlan::new();
        let mut plan_b = RealFftPlan::new();
        let a = band_pass_filter(&mut plan_a, &tone, 16_000.0, 800.0, 1200.0);
        let b = band_pass_filter(&mut plan_b, &tone, 16_000.0, 800.0, 1200.0);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.to_bits(), y.to_bits());
        }
    }

    #[test]
    fn short_or_invalid_spans_pass_through_unchanged() {
        let mut plan = RealFftPlan::new();
        let one = vec![0.7_f32];
        assert_eq!(
            band_pass_filter(&mut plan, &one, 16_000.0, 100.0, 200.0),
            one
        );
        let some = vec![0.1_f32, -0.2, 0.3];
        assert_eq!(band_pass_filter(&mut plan, &some, 0.0, 100.0, 200.0), some);
    }
}
