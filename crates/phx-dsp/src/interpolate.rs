//! Windowed-sinc interpolation and sub-sample peak location.

use std::f64::consts::PI;

/// Value of the sampled sequence `y` at the continuous index `x`, by
/// Hanning-windowed `sin(x)/x` interpolation.
///
/// This is Boersma (1993) eq. 22: the ideal sinc reconstruction truncated to
/// `depth` samples on each side of `x` and tapered by a raised cosine so the
/// interpolation falls to zero at its edges. With `nl = ⌊x⌋`, `φ_l = x − nl`,
/// and `φ_r = 1 − φ_l`,
///
/// ```text
/// y(x) ≈ Σ_{k=1}^{depth} y[nl+1−k] · sinc(φ_l+k−1) · ½(1 + cos(π(φ_l+k−1)/(φ_l+depth)))
///      + Σ_{k=1}^{depth} y[nl+k]   · sinc(φ_r+k−1) · ½(1 + cos(π(φ_r+k−1)/(φ_r+depth)))
/// ```
///
/// where `sinc(d) = sin(πd)/(πd)` (and `sinc(0) = 1`). Samples whose index falls
/// outside `y` are treated as zero. At an integer `x` every off-sample term has
/// a zero sinc factor, so the result is exactly `y[x]`.
fn sinc_interpolate(y: &[f64], x: f64, depth: usize) -> f64 {
    let nl = x.floor();
    let nl_i = nl as isize;
    let phi_l = x - nl; // ∈ [0, 1)
    let phi_r = 1.0 - phi_l;
    let d = depth as f64;

    let mut acc = 0.0;
    for k in 1..=depth {
        let kf = k as f64;
        // Left side: sample index nl + 1 − k, distance φ_l + k − 1.
        let li = nl_i + 1 - k as isize;
        if li >= 0 && (li as usize) < y.len() {
            let dist = phi_l + kf - 1.0;
            acc += y[li as usize] * sinc(dist) * raised_cosine(dist, phi_l, d);
        }
        // Right side: sample index nl + k, distance φ_r + k − 1.
        let ri = nl_i + k as isize;
        if ri >= 0 && (ri as usize) < y.len() {
            let dist = phi_r + kf - 1.0;
            acc += y[ri as usize] * sinc(dist) * raised_cosine(dist, phi_r, d);
        }
    }
    acc
}

/// `sin(πd)/(πd)`, with the removable singularity `sinc(0) = 1`.
fn sinc(d: f64) -> f64 {
    if d == 0.0 {
        1.0
    } else {
        let a = PI * d;
        a.sin() / a
    }
}

/// Raised-cosine edge taper `½(1 + cos(π·dist/(phi + depth)))` of eq. 22, which
/// reaches zero one step past the outermost sample.
fn raised_cosine(dist: f64, phi: f64, depth: f64) -> f64 {
    0.5 + 0.5 * (PI * dist / (phi + depth)).cos()
}

/// Locates the local maximum of `y` near index `around` to sub-sample precision.
///
/// The sampled sequence is reconstructed with [`sinc_interpolate`] (Boersma
/// 1993 eq. 22) and the continuous maximum is sought in `[around−1, around+1]`
/// (Boersma refines this bracket with Brent's method; a coarse scan followed by
/// golden-section search reaches the same optimum here). Returns
/// `(position, value)` with `position` in fractional sample units. Sinc
/// interpolation — rather than parabolic — is what recovers the peak height
/// accurately, which downstream HNR estimation needs.
///
/// `depth` is the one-sided interpolation width in samples; larger is more
/// accurate and slower (Boersma caps it at 500). A `depth` of `0` is treated as
/// `1`.
#[must_use]
pub fn sinc_interpolate_max(y: &[f64], around: usize, depth: usize) -> (f64, f64) {
    let depth = depth.max(1);
    if y.is_empty() {
        return (0.0, 0.0);
    }
    let last = (y.len() - 1) as f64;
    let lo = (around as f64 - 1.0).max(0.0);
    let hi = (around as f64 + 1.0).min(last);
    if lo >= hi {
        let x = (around as f64).clamp(0.0, last);
        return (x, sinc_interpolate(y, x, depth));
    }

    // Coarse scan to bracket the global maximum inside [lo, hi], then a
    // golden-section refinement around the best node. The scan guards against
    // the rare non-unimodal case a bare golden-section search could miss.
    const GRID: usize = 64;
    let mut best_x = lo;
    let mut best_v = sinc_interpolate(y, lo, depth);
    for i in 1..=GRID {
        let x = lo + (hi - lo) * i as f64 / GRID as f64;
        let v = sinc_interpolate(y, x, depth);
        if v > best_v {
            best_v = v;
            best_x = x;
        }
    }
    let node = (hi - lo) / GRID as f64;
    let a = (best_x - node).max(lo);
    let b = (best_x + node).min(hi);
    golden_section_max(|x| sinc_interpolate(y, x, depth), a, b)
}

/// Golden-section search for the maximum of a unimodal `f` on `[a, b]`.
fn golden_section_max<F: Fn(f64) -> f64>(f: F, mut a: f64, mut b: f64) -> (f64, f64) {
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0; // 1/φ ≈ 0.618
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    let mut fc = f(c);
    let mut fd = f(d);
    // ~100 iterations shrink the bracket by 0.618^100 ≈ 1e-21, far below the
    // required sub-sample precision.
    for _ in 0..100 {
        if fc > fd {
            b = d;
            d = c;
            fd = fc;
            c = b - gr * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + gr * (b - a);
            fd = f(d);
        }
    }
    let x = 0.5 * (a + b);
    (x, f(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_value_at_integer() {
        let y = [0.0, 1.0, 4.0, 9.0, 16.0, 25.0];
        for (i, &yi) in y.iter().enumerate() {
            let v = sinc_interpolate(&y, i as f64, 8);
            assert!((v - yi).abs() < 1e-9, "index {i}: {v} vs {yi}");
        }
    }

    #[test]
    fn recovers_sinusoid_peak_to_subsample() {
        // A pure cosine is band-limited below Nyquist, so windowed-sinc
        // interpolation reconstructs it and locates its peak precisely.
        let period = 20.0;
        let true_peak = 64.37; // non-integer maximum location
        let n = 128;
        let y: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * (i as f64 - true_peak) / period).cos())
            .collect();
        let around = true_peak.round() as usize;
        let (pos, val) = sinc_interpolate_max(&y, around, 50);
        assert!(
            (pos - true_peak).abs() < 0.01,
            "peak at {pos}, want {true_peak}"
        );
        assert!((val - 1.0).abs() < 1e-3, "peak height {val}");
    }

    #[test]
    fn recovers_peak_for_several_offsets() {
        let period = 24.0;
        let n = 160;
        for &frac in &[0.05, 0.25, 0.5, 0.73, 0.9] {
            let true_peak = 80.0 + frac;
            let y: Vec<f64> = (0..n)
                .map(|i| (2.0 * PI * (i as f64 - true_peak) / period).cos())
                .collect();
            let (pos, _) = sinc_interpolate_max(&y, true_peak.round() as usize, 60);
            assert!((pos - true_peak).abs() < 0.01, "frac {frac}: got {pos}");
        }
    }

    #[test]
    fn handles_edges_and_degenerate_inputs() {
        assert_eq!(sinc_interpolate_max(&[], 0, 10), (0.0, 0.0));
        let (p, v) = sinc_interpolate_max(&[5.0], 0, 10);
        assert_eq!(p, 0.0);
        assert!((v - 5.0).abs() < 1e-12);
        // A peak at the first sample: bracket clamps to [0, 1].
        let y = [3.0, 1.0, 0.5, 0.2];
        let (p, _) = sinc_interpolate_max(&y, 0, 8);
        assert!((0.0..=1.0).contains(&p));
    }
}
