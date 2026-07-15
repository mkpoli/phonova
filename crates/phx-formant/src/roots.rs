use std::f64::consts::PI;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

use crate::FormantPoint;

const ROOT_TOLERANCE: f64 = 1.0e-12;
const MAX_ABERTH_ITERS: usize = 200;
const MAX_POLISH_ITERS: usize = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    fn abs(self) -> f64 {
        self.norm_sqr().sqrt()
    }

    fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    fn from_polar(radius: f64, angle: f64) -> Self {
        Self::new(radius * angle.cos(), radius * angle.sin())
    }
}

impl Add for Complex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl AddAssign for Complex {
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl Sub for Complex {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl SubAssign for Complex {
    fn sub_assign(&mut self, rhs: Self) {
        self.re -= rhs.re;
        self.im -= rhs.im;
    }
}

impl Mul for Complex {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl Mul<f64> for Complex {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.re * rhs, self.im * rhs)
    }
}

impl Div for Complex {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let denom = rhs.norm_sqr();
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / denom,
            (self.im * rhs.re - self.re * rhs.im) / denom,
        )
    }
}

impl Div<f64> for Complex {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self::new(self.re / rhs, self.im / rhs)
    }
}

impl Neg for Complex {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.re, -self.im)
    }
}

/// Converts LPC denominator coefficients into frequency-bandwidth candidates.
pub(crate) fn lpc_roots_to_formants(
    coeffs: &[f64],
    sample_rate: f64,
    ceiling_hz: f64,
) -> Vec<FormantPoint> {
    let roots = polynomial_roots(coeffs);
    let mut formants = roots
        .into_iter()
        .filter_map(|root| root_to_formant(root, sample_rate, ceiling_hz))
        .collect::<Vec<_>>();
    formants.sort_by(|left, right| left.frequency.total_cmp(&right.frequency));
    formants
}

fn root_to_formant(root: Complex, sample_rate: f64, ceiling_hz: f64) -> Option<FormantPoint> {
    let mut root = root;
    let norm = root.norm_sqr();
    if norm <= f64::EPSILON {
        return None;
    }
    if norm > 1.0 {
        root = root / norm;
    }
    if root.im <= 1.0e-8 {
        return None;
    }
    let radius = root.abs().clamp(f64::MIN_POSITIVE, 1.0);
    let frequency = root.arg() * sample_rate / (2.0 * PI);
    let bandwidth = -(sample_rate / PI) * radius.ln();
    (frequency >= 50.0 && frequency <= ceiling_hz - 50.0).then_some(FormantPoint {
        frequency,
        bandwidth,
    })
}

fn polynomial_roots(coeffs: &[f64]) -> Vec<Complex> {
    let degree = coeffs.len();
    if degree == 0 {
        return Vec::new();
    }

    let radius = 1.0
        + coeffs
            .iter()
            .fold(0.0_f64, |acc, coeff| acc.max(coeff.abs()));
    let angle_offset = PI / degree as f64;
    let mut roots = (0..degree)
        .map(|i| Complex::from_polar(radius, angle_offset + 2.0 * PI * i as f64 / degree as f64))
        .collect::<Vec<_>>();

    for _ in 0..MAX_ABERTH_ITERS {
        let mut max_delta = 0.0_f64;
        let previous = roots.clone();
        for i in 0..degree {
            let z = previous[i];
            let (value, derivative) = eval_poly_and_derivative(coeffs, z);
            if value.abs() <= ROOT_TOLERANCE {
                continue;
            }
            if derivative.abs() <= f64::EPSILON {
                continue;
            }

            let newton = value / derivative;
            let mut repulsion = Complex::default();
            for (j, other) in previous.iter().enumerate() {
                if i == j {
                    continue;
                }
                let diff = z - *other;
                if diff.abs() > f64::EPSILON {
                    repulsion += Complex::new(1.0, 0.0) / diff;
                }
            }
            let denominator = Complex::new(1.0, 0.0) - newton * repulsion;
            if denominator.abs() <= f64::EPSILON {
                continue;
            }
            let delta = newton / denominator;
            roots[i] -= delta;
            max_delta = max_delta.max(delta.abs());
        }
        if max_delta <= ROOT_TOLERANCE {
            break;
        }
    }

    for root in &mut roots {
        polish_root(coeffs, root);
    }

    roots
}

fn polish_root(coeffs: &[f64], root: &mut Complex) {
    for _ in 0..MAX_POLISH_ITERS {
        let (value, derivative) = eval_poly_and_derivative(coeffs, *root);
        if derivative.abs() <= f64::EPSILON {
            return;
        }
        let delta = value / derivative;
        *root -= delta;
        if delta.abs() <= ROOT_TOLERANCE {
            return;
        }
    }
}

fn eval_poly_and_derivative(coeffs: &[f64], z: Complex) -> (Complex, Complex) {
    let mut value = Complex::new(1.0, 0.0);
    let mut derivative = Complex::default();
    for &coeff in coeffs {
        derivative = derivative * z + value;
        value = value * z + Complex::new(coeff, 0.0);
    }
    (value, derivative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_quadratic_conjugate_pair() {
        let frequency = 700.0;
        let bandwidth = 80.0;
        let sample_rate = 11_000.0;
        let radius = (-PI * bandwidth / sample_rate).exp();
        let theta = 2.0 * PI * frequency / sample_rate;
        let coeffs = [-2.0 * radius * theta.cos(), radius * radius];
        let formants = lpc_roots_to_formants(&coeffs, sample_rate, 5500.0);
        assert_eq!(formants.len(), 1);
        assert!((formants[0].frequency - frequency).abs() < 1.0e-8);
        assert!((formants[0].bandwidth - bandwidth).abs() < 1.0e-8);
    }
}
