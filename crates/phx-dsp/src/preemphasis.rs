//! In-place first-difference pre-emphasis.

use std::f64::consts::PI;

/// Applies pre-emphasis `y[t] = x[t] − a·x[t−1]` in place, with
/// `a = exp(−2π·from_hz / sample_rate)`.
///
/// This first-difference high-pass compensates the roughly −6 dB/octave
/// source-plus-radiation tilt of voiced speech before LPC analysis
/// (Markel & Gray 1976; Praat manual, "Sound: Pre-emphasize (in-place)...").
/// `from_hz` is the +3 dB corner of the resulting +6 dB/octave filter; Praat's
/// default is 50 Hz. The first sample is left unchanged (no predecessor).
/// Samples are processed back-to-front so each update reads the original
/// `x[t−1]`.
///
/// A non-positive `from_hz` yields `a ≥ 1` and is a valid (if unusual) request;
/// `from_hz = 0` gives `a = 1`, the plain first difference.
pub fn preemphasis_in_place(x: &mut [f64], from_hz: f64, sample_rate: f64) {
    assert!(sample_rate > 0.0, "sample_rate must be positive");
    let a = (-2.0 * PI * from_hz / sample_rate).exp();
    for t in (1..x.len()).rev() {
        x[t] -= a * x[t - 1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_matches_worked_example() {
        // Praat manual worked example: 48.47 Hz at 10 kHz → a ≈ 0.97.
        let a = (-2.0 * PI * 48.47 / 10_000.0).exp();
        assert!((a - 0.97).abs() < 5e-4, "a = {a}");
    }

    #[test]
    fn impulse_response_is_first_difference() {
        // Impulse in → FIR taps [1, −a].
        let a = (-2.0 * PI * 50.0 / 10_000.0).exp();
        let mut x = vec![1.0, 0.0, 0.0, 0.0];
        preemphasis_in_place(&mut x, 50.0, 10_000.0);
        assert!((x[0] - 1.0).abs() < 1e-15);
        assert!((x[1] - (-a)).abs() < 1e-15);
        assert!(x[2].abs() < 1e-15);
        assert!(x[3].abs() < 1e-15);
    }

    #[test]
    fn constant_signal_is_attenuated() {
        let a = (-2.0 * PI * 50.0 / 16_000.0).exp();
        let mut x = vec![3.0; 8];
        preemphasis_in_place(&mut x, 50.0, 16_000.0);
        assert!((x[0] - 3.0).abs() < 1e-15); // first sample untouched
        for v in &x[1..] {
            assert!((v - 3.0 * (1.0 - a)).abs() < 1e-12);
        }
    }

    #[test]
    fn dc_gain_is_zero_at_nonzero_corner() {
        // The filter (1, −a) has DC gain 1 − a < 1 and Nyquist gain 1 + a.
        let sr = 16_000.0;
        let a = (-2.0 * PI * 50.0 / sr).exp();
        let n = 64;
        // Nyquist input (−1)^t, amplitude preserved by the alternating sum.
        let mut x: Vec<f64> = (0..n)
            .map(|t| if t % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        preemphasis_in_place(&mut x, 50.0, sr);
        // y[t] = x[t] − a·x[t−1] = (−1)^t (1 + a) for t ≥ 1.
        for t in 1..n {
            let s = if t % 2 == 0 { 1.0 } else { -1.0 };
            assert!((x[t] - s * (1.0 + a)).abs() < 1e-12);
        }
    }

    #[test]
    fn short_inputs_are_noops() {
        let mut empty: [f64; 0] = [];
        preemphasis_in_place(&mut empty, 50.0, 16_000.0);
        let mut single = [2.5];
        preemphasis_in_place(&mut single, 50.0, 16_000.0);
        assert_eq!(single, [2.5]);
    }
}
