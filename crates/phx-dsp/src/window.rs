//! Analysis windows.
//!
//! All windows here are symmetric: `w[i] == w[n - 1 - i]`. Sample positions run
//! across the physical window, endpoints included, so `n` samples span the
//! closed interval and the two ends carry the window's edge value.

use std::f64::consts::PI;

/// Shape of an analysis window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Window {
    /// Raised-cosine (Hann) window, `0.5 - 0.5·cos(2π·i/(n-1))`.
    ///
    /// Boersma (1993) uses this shape for autocorrelation pitch frames
    /// (eq. 6, `w(t) = 0.5 - 0.5·cos(2πt/T)`).
    Hanning,
    /// Gaussian window whose physical length is `effective_len_factor` times
    /// its effective (bandwidth-defining) length.
    ///
    /// The effective length `L` is the one that sets the −3 dB bandwidth
    /// `1.2982804 / L` (Praat manual, "Sound: To Spectrogram..."). With
    /// `effective_len_factor = 2` the window reaches the classic Praat edge
    /// value `exp(−12) ≈ 6.14e-6` at the physical ends before edge
    /// normalisation, matching the Gaussian window Praat applies for
    /// spectrogram and Burg-formant analysis.
    Gaussian {
        /// Ratio of physical window length to effective length. Praat's
        /// Gaussian analysis uses `2.0` (physical window twice the effective
        /// window). Must be positive.
        effective_len_factor: f64,
    },
    /// Kaiser window with shape parameter `beta`.
    ///
    /// `w[i] = I0(β·√(1 − r²)) / I0(β)` with `r = 2i/(n-1) − 1 ∈ [−1, 1]` and
    /// `I0` the zeroth-order modified Bessel function. Larger `beta` trades main
    /// -lobe width for sidelobe suppression (Kaiser & Schafer 1980).
    Kaiser {
        /// Shape parameter; `0` gives a rectangular window, larger values
        /// widen the main lobe and deepen the sidelobes.
        beta: f64,
    },
}

impl Default for Window {
    /// Praat's spectrogram/formant Gaussian: physical window twice the
    /// effective length (`effective_len_factor = 2.0`).
    fn default() -> Self {
        Window::Gaussian {
            effective_len_factor: 2.0,
        }
    }
}

/// Samples of window `w` over `n` points.
///
/// Returns an empty vector for `n == 0` and `vec![1.0]` for `n == 1` (a single
/// sample carries unit weight). For `n ≥ 2` the result is symmetric and its
/// peak sits at the centre.
#[must_use]
pub fn window_samples(w: Window, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![1.0];
    }
    let last = (n - 1) as f64;
    match w {
        Window::Hanning => (0..n)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / last).cos())
            .collect(),
        Window::Gaussian {
            effective_len_factor,
        } => {
            // u ∈ [−0.5, 0.5] across the physical window. σ_time = L/(2√6) for
            // effective length L = physical/effective_len_factor, so the
            // exponent is exp(−12·f²·u²). Subtract the edge value and rescale so
            // the ends fall exactly to zero (Praat's edge treatment).
            let f2 = effective_len_factor * effective_len_factor;
            let edge = (-3.0 * f2).exp();
            let denom = 1.0 - edge;
            (0..n)
                .map(|i| {
                    let u = i as f64 / last - 0.5;
                    ((-12.0 * f2 * u * u).exp() - edge) / denom
                })
                .collect()
        }
        Window::Kaiser { beta } => {
            let denom = bessel_i0(beta);
            (0..n)
                .map(|i| {
                    let r = 2.0 * i as f64 / last - 1.0;
                    bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / denom
                })
                .collect()
        }
    }
}

/// Zeroth-order modified Bessel function of the first kind, `I0(x)`, by its
/// power series `Σ ((x/2)^k / k!)²`.
fn bessel_i0(x: f64) -> f64 {
    let half_sq = (x * 0.5) * (x * 0.5);
    let mut term = 1.0;
    let mut sum = 1.0;
    let mut k = 1.0;
    loop {
        term *= half_sq / (k * k);
        sum += term;
        if term <= 1e-16 * sum {
            break;
        }
        k += 1.0;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_symmetric(w: &[f64]) -> bool {
        let n = w.len();
        (0..n / 2).all(|i| (w[i] - w[n - 1 - i]).abs() < 1e-15)
    }

    #[test]
    fn degenerate_lengths() {
        assert!(window_samples(Window::Hanning, 0).is_empty());
        assert_eq!(window_samples(Window::Hanning, 1), vec![1.0]);
        assert_eq!(
            window_samples(
                Window::Gaussian {
                    effective_len_factor: 2.0
                },
                1
            ),
            vec![1.0]
        );
        assert_eq!(window_samples(Window::Kaiser { beta: 8.0 }, 1), vec![1.0]);
    }

    #[test]
    fn all_windows_symmetric() {
        for n in [2usize, 3, 16, 17, 256, 257] {
            assert!(
                is_symmetric(&window_samples(Window::Hanning, n)),
                "hann {n}"
            );
            assert!(
                is_symmetric(&window_samples(
                    Window::Gaussian {
                        effective_len_factor: 2.0
                    },
                    n
                )),
                "gauss {n}"
            );
            assert!(
                is_symmetric(&window_samples(Window::Kaiser { beta: 8.0 }, n)),
                "kaiser {n}"
            );
        }
    }

    #[test]
    fn hanning_endpoints_and_centre() {
        let w = window_samples(Window::Hanning, 101);
        assert!(w[0].abs() < 1e-15);
        assert!(w[100].abs() < 1e-15);
        assert!((w[50] - 1.0).abs() < 1e-15); // centre of an odd-length Hann
    }

    #[test]
    fn hanning_energy_and_gain() {
        // Analytic large-n limits: mean of w → 0.5, mean of w² → 0.375.
        let n = 4096;
        let w = window_samples(Window::Hanning, n);
        let mean: f64 = w.iter().sum::<f64>() / n as f64;
        let mean_sq: f64 = w.iter().map(|v| v * v).sum::<f64>() / n as f64;
        assert!((mean - 0.5).abs() < 1e-3, "coherent gain {mean}");
        assert!((mean_sq - 0.375).abs() < 1e-3, "energy {mean_sq}");
    }

    #[test]
    fn gaussian_edge_value_matches_praat() {
        // effective_len_factor = 2 → physical ends fall to exp(−12) before the
        // edge is subtracted, so after normalisation the ends are exactly zero
        // and the centre is exactly one.
        let w = window_samples(
            Window::Gaussian {
                effective_len_factor: 2.0,
            },
            1001,
        );
        assert!(w[0].abs() < 1e-12);
        assert!(w[1000].abs() < 1e-12);
        assert!((w[500] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn gaussian_minus3db_bandwidth() {
        // Numerically confirm the −3 dB bandwidth of the effective Gaussian is
        // 1.2982804 / L. Build the effective-length Gaussian (factor 1) with a
        // fine time grid, FFT-free closed form: magnitude response of
        // exp(−t²/2σ²) is exp(−σ²ω²/2), σ = L/(2√6). Half-power at
        // f = √(ln2)/(2πσ); full −3 dB width = 2f = √(ln2)·2√6/(π·L).
        let l = 1.0_f64; // effective length in seconds (arbitrary)
        let sigma = l / (2.0 * 6.0_f64.sqrt());
        let f_half = (2.0_f64.ln()).sqrt() / (2.0 * PI * sigma);
        let bandwidth = 2.0 * f_half;
        assert!((bandwidth - 1.2982804 / l).abs() < 1e-6, "bw {bandwidth}");
    }

    #[test]
    fn bessel_i0_reference_values() {
        assert!((bessel_i0(0.0) - 1.0).abs() < 1e-15);
        // I0(1) ≈ 1.2660658778, I0(2) ≈ 2.2795853024 (standard tables).
        assert!((bessel_i0(1.0) - 1.266_065_877_8).abs() < 1e-9);
        assert!((bessel_i0(2.0) - 2.279_585_302_4).abs() < 1e-9);
    }
}
