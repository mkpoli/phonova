//! Colormap selection and dB→color sampling.

use crate::data::{
    cividis::CIVIDIS_DATA, inferno::INFERNO_DATA, magma::MAGMA_DATA, plasma::PLASMA_DATA,
    viridis::VIRIDIS_DATA,
};
use crate::theme::Theme;

/// Perceptual colormap used to render a normalized dB tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Colormap {
    /// Perceptually uniform purple→teal→yellow ramp (matplotlib default
    /// since 2.0). Monotonically increasing in relative luminance.
    Viridis,
    /// Perceptually uniform black→purple→orange→pale-yellow ramp.
    /// Monotonically increasing in relative luminance.
    Magma,
    /// Perceptually uniform black→purple→orange→pale-yellow ramp, a warmer
    /// sibling of magma. Monotonically increasing in relative luminance.
    Inferno,
    /// Perceptually uniform dark-blue→purple→orange→yellow ramp.
    /// Monotonically increasing in relative luminance.
    Plasma,
    /// Perceptually uniform dark-blue→gray→yellow ramp optimized for
    /// color-vision deficiency. Monotonically increasing in relative
    /// luminance.
    Cividis,
    /// Achromatic ramp, tuned separately per [`Theme`] rather than
    /// inverted.
    Grayscale,
}

impl Colormap {
    /// Sample the colormap at normalized position `t` (clamped to
    /// `[0, 1]`, where `0` is the display floor and `1` is `max_db`),
    /// returning 8-bit sRGB channel values.
    pub(crate) fn sample(self, t: f32, theme: Theme) -> [u8; 3] {
        match self {
            Colormap::Viridis => sample_lut(&VIRIDIS_DATA, t),
            Colormap::Magma => sample_lut(&MAGMA_DATA, t),
            Colormap::Inferno => sample_lut(&INFERNO_DATA, t),
            Colormap::Plasma => sample_lut(&PLASMA_DATA, t),
            Colormap::Cividis => sample_lut(&CIVIDIS_DATA, t),
            Colormap::Grayscale => sample_grayscale(t, theme),
        }
    }
}

/// Linearly interpolate a 256-entry `[0, 1]` control-point table at `t`
/// and quantize to 8-bit sRGB channels.
fn sample_lut(lut: &[[f32; 3]; 256], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let pos = t * (lut.len() - 1) as f32;
    let i0 = pos.floor() as usize;
    let i1 = (i0 + 1).min(lut.len() - 1);
    let frac = pos - i0 as f32;
    let c0 = lut[i0];
    let c1 = lut[i1];
    [
        to_u8(c0[0] + (c1[0] - c0[0]) * frac),
        to_u8(c0[1] + (c1[1] - c0[1]) * frac),
        to_u8(c0[2] + (c1[2] - c0[2]) * frac),
    ]
}

/// Grayscale endpoints, `(floor_color, ceiling_color)`.
///
/// Light theme runs white (silence) to black (loudest), matching ink
/// density on a page. Dark theme is not that ramp inverted: pure black
/// at the floor would read as a hole punched through the dark panel
/// background, and pure white at the ceiling would blow out against it.
/// Instead the floor sits at a dark neutral gray close to the panel
/// background and the ceiling stops short of full white, keeping
/// low-energy regions visually merged with the surrounding UI and
/// reserving peak brightness for genuinely loud content.
const GRAYSCALE_LIGHT: ([u8; 3], [u8; 3]) = ([255, 255, 255], [0, 0, 0]);
const GRAYSCALE_DARK: ([u8; 3], [u8; 3]) = ([30, 30, 30], [235, 235, 235]);

/// Sample the theme-tuned grayscale ramp at normalized position `t`.
fn sample_grayscale(t: f32, theme: Theme) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let (floor, ceiling) = match theme {
        Theme::Light => GRAYSCALE_LIGHT,
        Theme::Dark => GRAYSCALE_DARK,
    };
    [
        lerp_u8(floor[0], ceiling[0], t),
        lerp_u8(floor[1], ceiling[1], t),
        lerp_u8(floor[2], ceiling[2], t),
    ]
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    to_u8(a as f32 / 255.0 + (b as f32 - a as f32) / 255.0 * t)
}

/// Quantize a `[0, 1]` linear channel value to an 8-bit integer, clamping
/// out-of-range input defensively.
fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG 2.x relative luminance of a linear-`[0, 1]` control-point triple
    /// (no 8-bit quantization). This is the property the colormap data was
    /// designed to satisfy; testing it pre-quantization avoids spurious
    /// failures from the same 1-LSB rounding jitter that shows up as a
    /// handful of near-zero luminance dips once the ramp is rounded to
    /// 8-bit sRGB (an imperceptible quantization artifact, not a property
    /// of the ramp itself).
    fn relative_luminance_linear([r, g, b]: [f32; 3]) -> f64 {
        fn channel(c: f32) -> f64 {
            let c = c as f64;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }

    #[test]
    fn viridis_endpoints_match_published_hex() {
        assert_eq!(sample_lut(&VIRIDIS_DATA, 0.0), [68, 1, 84]);
        assert_eq!(sample_lut(&VIRIDIS_DATA, 1.0), [253, 231, 37]);
    }

    #[test]
    fn magma_endpoints_match_published_hex() {
        assert_eq!(sample_lut(&MAGMA_DATA, 0.0), [0, 0, 4]);
        assert_eq!(sample_lut(&MAGMA_DATA, 1.0), [252, 253, 191]);
    }

    #[test]
    fn inferno_endpoints_match_published_hex() {
        assert_eq!(sample_lut(&INFERNO_DATA, 0.0), [0, 0, 4]);
        assert_eq!(sample_lut(&INFERNO_DATA, 1.0), [252, 255, 164]);
    }

    #[test]
    fn plasma_endpoints_match_published_hex() {
        assert_eq!(sample_lut(&PLASMA_DATA, 0.0), [13, 8, 135]);
        assert_eq!(sample_lut(&PLASMA_DATA, 1.0), [240, 249, 33]);
    }

    #[test]
    fn cividis_endpoints_match_published_hex() {
        assert_eq!(sample_lut(&CIVIDIS_DATA, 0.0), [0, 34, 78]);
        assert_eq!(sample_lut(&CIVIDIS_DATA, 1.0), [254, 232, 56]);
    }

    fn assert_monotonic_luminance(data: &[[f32; 3]; 256], name: &str) {
        let mut prev = relative_luminance_linear(data[0]);
        for (i, &stop) in data.iter().enumerate().skip(1) {
            let lum = relative_luminance_linear(stop);
            assert!(
                lum + 1e-12 >= prev,
                "{name} luminance decreased at stop {i}: {prev} -> {lum}"
            );
            prev = lum;
        }
    }

    #[test]
    fn inferno_luminance_is_monotonic() {
        assert_monotonic_luminance(&INFERNO_DATA, "inferno");
    }

    #[test]
    fn plasma_luminance_is_monotonic() {
        assert_monotonic_luminance(&PLASMA_DATA, "plasma");
    }

    #[test]
    fn cividis_luminance_is_monotonic() {
        assert_monotonic_luminance(&CIVIDIS_DATA, "cividis");
    }

    #[test]
    fn viridis_luminance_is_monotonic() {
        let mut prev = relative_luminance_linear(VIRIDIS_DATA[0]);
        for (i, &stop) in VIRIDIS_DATA.iter().enumerate().skip(1) {
            let lum = relative_luminance_linear(stop);
            assert!(
                lum + 1e-12 >= prev,
                "luminance decreased at stop {i}: {prev} -> {lum}"
            );
            prev = lum;
        }
    }

    #[test]
    fn magma_luminance_is_monotonic() {
        let mut prev = relative_luminance_linear(MAGMA_DATA[0]);
        for (i, &stop) in MAGMA_DATA.iter().enumerate().skip(1) {
            let lum = relative_luminance_linear(stop);
            assert!(
                lum + 1e-12 >= prev,
                "luminance decreased at stop {i}: {prev} -> {lum}"
            );
            prev = lum;
        }
    }

    #[test]
    fn grayscale_dark_floor_is_not_pure_black() {
        // Never a naive inversion of the light ramp.
        let (floor, ceiling) = GRAYSCALE_DARK;
        assert_ne!(floor, [0, 0, 0]);
        assert_ne!(ceiling, [255, 255, 255]);
        assert_ne!((floor, ceiling), (GRAYSCALE_LIGHT.1, GRAYSCALE_LIGHT.0));
    }
}
