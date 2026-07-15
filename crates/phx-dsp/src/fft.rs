//! Cached real-input FFT plans.

use std::collections::HashMap;
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

/// A cache of real-FFT plans keyed by transform length.
///
/// Building a plan is comparatively costly; per-frame analysis reuses one plan
/// per size across thousands of frames. Forward transforms take `n` real
/// samples to `n/2 + 1` complex bins; inverse transforms take those bins back to
/// `n` real samples. Both directions follow the unnormalised DFT convention:
/// a forward-then-inverse round trip scales the signal by `n`, so divide by `n`
/// after an inverse when reconstructing.
pub struct RealFftPlan {
    planner: RealFftPlanner<f64>,
    forward: HashMap<usize, Arc<dyn RealToComplex<f64>>>,
    inverse: HashMap<usize, Arc<dyn ComplexToReal<f64>>>,
}

impl RealFftPlan {
    /// Creates an empty plan cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            planner: RealFftPlanner::new(),
            forward: HashMap::new(),
            inverse: HashMap::new(),
        }
    }

    /// Returns the forward (real → complex) plan for length `n`, creating and
    /// caching it on first use.
    pub fn forward(&mut self, n: usize) -> Arc<dyn RealToComplex<f64>> {
        let planner = &mut self.planner;
        Arc::clone(
            self.forward
                .entry(n)
                .or_insert_with(|| planner.plan_fft_forward(n)),
        )
    }

    /// Returns the inverse (complex → real) plan for length `n`, creating and
    /// caching it on first use.
    pub fn inverse(&mut self, n: usize) -> Arc<dyn ComplexToReal<f64>> {
        let planner = &mut self.planner;
        Arc::clone(
            self.inverse
                .entry(n)
                .or_insert_with(|| planner.plan_fft_inverse(n)),
        )
    }

    /// Forward transform of `input`.
    ///
    /// Returns the `input.len()/2 + 1` non-redundant complex bins of the
    /// unnormalised DFT. `input` is used as scratch and left in an unspecified
    /// state.
    pub fn rfft(&mut self, input: &mut [f64]) -> Vec<Complex<f64>> {
        let plan = self.forward(input.len());
        let mut out = plan.make_output_vec();
        plan.process(input, &mut out)
            .expect("realfft forward: buffer lengths match the plan");
        out
    }

    /// Inverse transform of `spectrum` back to `n` real samples (unnormalised;
    /// divide by `n` for a round-trip).
    ///
    /// `spectrum` must hold `n/2 + 1` bins and is used as scratch. Its DC bin
    /// (and Nyquist bin, when `n` is even) must be real, as they are for any
    /// spectrum produced by [`RealFftPlan::rfft`].
    pub fn irfft(&mut self, spectrum: &mut [Complex<f64>], n: usize) -> Vec<f64> {
        let plan = self.inverse(n);
        let mut out = plan.make_output_vec();
        plan.process(spectrum, &mut out)
            .expect("realfft inverse: buffer lengths and DC/Nyquist bins are valid");
        out
    }
}

impl Default for RealFftPlan {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn dc_signal_spectrum() {
        let n = 64;
        let mut x = vec![1.0_f64; n];
        let mut plan = RealFftPlan::new();
        let spec = plan.rfft(&mut x);
        assert_eq!(spec.len(), n / 2 + 1);
        // Constant 1 → all energy in the DC bin, value n.
        assert!((spec[0].re - n as f64).abs() < 1e-9);
        assert!(spec[0].im.abs() < 1e-9);
        for bin in &spec[1..] {
            assert!(bin.norm() < 1e-9, "nonzero AC bin {bin}");
        }
    }

    #[test]
    fn cosine_spectrum_matches_analytic() {
        // x[j] = cos(2π k0 j / n) → a single real spectral line n/2 at bin k0.
        let n = 128;
        let k0 = 7;
        let mut x: Vec<f64> = (0..n)
            .map(|j| (2.0 * PI * k0 as f64 * j as f64 / n as f64).cos())
            .collect();
        let mut plan = RealFftPlan::new();
        let spec = plan.rfft(&mut x);
        for (k, bin) in spec.iter().enumerate() {
            let expected = if k == k0 { n as f64 / 2.0 } else { 0.0 };
            assert!((bin.re - expected).abs() < 1e-9, "bin {k} re {}", bin.re);
            assert!(bin.im.abs() < 1e-9, "bin {k} im {}", bin.im);
        }
    }

    #[test]
    fn sine_spectrum_matches_analytic() {
        // x[j] = sin(2π k0 j / n) → imaginary line −n/2 at bin k0.
        let n = 256;
        let k0 = 11;
        let mut x: Vec<f64> = (0..n)
            .map(|j| (2.0 * PI * k0 as f64 * j as f64 / n as f64).sin())
            .collect();
        let mut plan = RealFftPlan::new();
        let spec = plan.rfft(&mut x);
        for (k, bin) in spec.iter().enumerate() {
            let expected_im = if k == k0 { -(n as f64) / 2.0 } else { 0.0 };
            assert!((bin.im - expected_im).abs() < 1e-9, "bin {k} im {}", bin.im);
            assert!(bin.re.abs() < 1e-9, "bin {k} re {}", bin.re);
        }
    }

    #[test]
    fn round_trip_reconstructs() {
        let n = 100;
        let original: Vec<f64> = (0..n)
            .map(|j| (0.3 * j as f64).sin() + 0.2 * j as f64)
            .collect();
        let mut buf = original.clone();
        let mut plan = RealFftPlan::new();
        let mut spec = plan.rfft(&mut buf);
        let back = plan.irfft(&mut spec, n);
        for (a, b) in original.iter().zip(back.iter()) {
            assert!((a - b / n as f64).abs() < 1e-9);
        }
    }

    #[test]
    fn plans_are_cached() {
        let mut plan = RealFftPlan::new();
        let _ = plan.forward(256);
        let _ = plan.forward(256);
        let _ = plan.inverse(256);
        assert_eq!(plan.forward.len(), 1);
        assert_eq!(plan.inverse.len(), 1);
    }
}
