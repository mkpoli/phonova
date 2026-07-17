//! Shared DSP primitives: analysis windows (Hanning, Gaussian, Kaiser), an
//! absolute-time frame grid, cached real-FFT plans, in-place pre-emphasis, and
//! windowed-sinc peak interpolation.
//!
//! Every analysis crate frames the signal through [`FrameGrid`], so a value
//! queried at a given time is identical regardless of zoom, scroll, or which
//! caller asked for it. Samples are stored `f32` elsewhere in the workspace and
//! promoted to `f64` here before windowing and transforms.
#![warn(missing_docs)]

mod fft;
mod filter;
mod frame_grid;
mod interpolate;
mod preemphasis;
mod window;

pub use fft::RealFftPlan;
pub use filter::{EDGE_TAPER_S, PASS_BAND_SKIRT_HZ, band_pass_filter, band_pass_gain};
pub use frame_grid::FrameGrid;
pub use interpolate::sinc_interpolate_max;
pub use preemphasis::preemphasis_in_place;
pub use window::{Window, window_samples};

/// Smallest power of two greater than or equal to `n`.
///
/// Returns `1` for `n == 0` and `n == 1`. Used to size FFT buffers after
/// zero-padding, where transform lengths must be powers of two.
#[must_use]
pub fn next_pow2(n: usize) -> usize {
    n.max(1).next_power_of_two()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_pow2_boundaries() {
        assert_eq!(next_pow2(0), 1);
        assert_eq!(next_pow2(1), 1);
        assert_eq!(next_pow2(2), 2);
        assert_eq!(next_pow2(3), 4);
        assert_eq!(next_pow2(1024), 1024);
        assert_eq!(next_pow2(1025), 2048);
    }
}
