/// Computes Burg LPC denominator coefficients for a real sequence.
///
/// Returns coefficients `a_i` for `1 + a_1 z^-1 + ... + a_p z^-p`, where
/// `p == order`. The reflection coefficient and coefficient recursion follow
/// the Numerical Recipes `memcof` Burg specification cited by Praat's public
/// manual pages.
pub(crate) fn burg_lpc(data: &[f64], order: usize) -> Option<Vec<f64>> {
    if order == 0 {
        return Some(Vec::new());
    }
    if data.len() <= order || data.iter().all(|&sample| sample == 0.0) {
        return None;
    }

    let mut error = data.iter().map(|sample| sample * sample).sum::<f64>() / data.len() as f64;
    if error <= f64::EPSILON {
        return None;
    }

    let mut forward = data[..data.len() - 1].to_vec();
    let mut backward = data[1..].to_vec();
    let mut coeffs = vec![0.0; order];

    for k in 0..order {
        let len = data.len() - k - 1;
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for j in 0..len {
            numerator += forward[j] * backward[j];
            denominator += forward[j] * forward[j] + backward[j] * backward[j];
        }
        if denominator <= f64::EPSILON {
            return None;
        }

        let reflection =
            (2.0 * numerator / denominator).clamp(-0.999_999_999_999, 0.999_999_999_999);
        let previous = coeffs.clone();
        coeffs[k] = -reflection;
        for i in 0..k {
            coeffs[i] = previous[i] - reflection * previous[k - i - 1];
        }

        error *= 1.0 - reflection * reflection;
        if error <= f64::EPSILON {
            break;
        }

        if k + 1 < order {
            let old_forward = forward;
            let old_backward = backward;
            let next_len = len - 1;
            forward = Vec::with_capacity(next_len);
            backward = Vec::with_capacity(next_len);
            for j in 0..next_len {
                forward.push(old_forward[j] - reflection * old_backward[j]);
                backward.push(old_backward[j + 1] - reflection * old_forward[j + 1]);
            }
        }
    }

    Some(coeffs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ar_one_coefficient_has_denominator_sign() {
        let phi = 0.82;
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next_noise = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed as f64 / u64::MAX as f64) - 0.5
        };
        let mut data = Vec::with_capacity(512);
        let mut value = 0.0;
        for _ in 0..512 {
            value = phi * value + next_noise();
            data.push(value);
        }
        let coeffs = burg_lpc(&data, 1).unwrap();
        assert!((coeffs[0] + phi).abs() < 0.04, "a1 {}", coeffs[0]);
    }
}
