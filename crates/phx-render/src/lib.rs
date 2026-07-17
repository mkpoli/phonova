//! Perceptual colormaps (viridis, magma, grayscale), theme-aware dB→RGBA
//! tile rendering.
//!
//! [`colorize`] maps a row-major tile of dB power values onto 8-bit RGBA
//! pixels: a linear-in-dB clip against `[floor, ceiling]`
//! ([`DisplayMapping`]), then a perceptual [`Colormap`] lookup tuned per
//! [`Theme`]. The crate takes plain arrays in and out; it has no
//! dependency on the rest of the workspace.
#![warn(missing_docs)]

mod colormap;
mod data;
mod mapping;
mod theme;

pub use colormap::Colormap;
pub use mapping::DisplayMapping;
pub use theme::Theme;

/// Colorize a row-major tile of dB power values into 8-bit RGBA pixels.
///
/// `tile_db` must contain exactly `w * h` values in row-major order (row 0
/// first). Values are clipped to `[max_db - dynamic_range_db, max_db]`
/// (autoscaling `max_db` to the tile's maximum finite value when
/// `map.max_db` is `None`) and mapped linearly onto the colormap. A
/// non-finite input value (`NaN`, `-inf`, e.g. from a silent frame) is
/// treated as the display floor.
///
/// Returns `4 * w * h` bytes, four per pixel in `R, G, B, A` order; tiles
/// are always fully opaque (`A = 255`) since color alone carries the
/// energy signal.
///
/// # Panics
/// Panics if `tile_db.len() != w as usize * h as usize`.
pub fn colorize(
    tile_db: &[f32],
    w: u32,
    h: u32,
    map: &DisplayMapping,
    cm: Colormap,
    theme: Theme,
) -> Vec<u8> {
    colorize_with(tile_db, w, h, map, |t| cm.sample(t, theme))
}

/// Colorize a row-major tile of dB power values with a caller-supplied
/// 256-entry 8-bit sRGB lookup table — a custom ramp built in the gradient
/// editor rather than one of the built-in [`Colormap`]s.
///
/// The display mapping and pixel layout match [`colorize`] exactly; only the
/// dB→color step differs, sampling `lut` (linearly interpolated between its
/// entries) in place of a [`Colormap`]. `lut[0]` is the display floor and
/// `lut[255]` the ceiling. A custom ramp is not luminance-monotonicity
/// checked here — the editor surfaces that property to the user instead.
///
/// # Panics
/// Panics if `tile_db.len() != w as usize * h as usize`.
pub fn colorize_with_lut(
    tile_db: &[f32],
    w: u32,
    h: u32,
    map: &DisplayMapping,
    lut: &[[u8; 3]; 256],
) -> Vec<u8> {
    colorize_with(tile_db, w, h, map, |t| sample_u8_lut(lut, t))
}

/// Linearly interpolate a caller-supplied 256-entry 8-bit sRGB lookup table at
/// `t` (clamped to `[0, 1]`). The interpolation mirrors the built-in ramps'
/// sampling, so a custom ramp and a built-in ramp with the same 256 entries
/// render identically.
fn sample_u8_lut(lut: &[[u8; 3]; 256], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let pos = t * (lut.len() - 1) as f32;
    let i0 = pos.floor() as usize;
    let i1 = (i0 + 1).min(lut.len() - 1);
    let frac = pos - i0 as f32;
    let c0 = lut[i0];
    let c1 = lut[i1];
    let lerp = |a: u8, b: u8| {
        let v = a as f32 + (b as f32 - a as f32) * frac;
        v.round().clamp(0.0, 255.0) as u8
    };
    [
        lerp(c0[0], c1[0]),
        lerp(c0[1], c1[1]),
        lerp(c0[2], c1[2]),
    ]
}

/// Shared dB→RGBA driver: resolve the clip window once, then map each value to
/// a normalized `t` and hand it to `sample`. Keeps the display mapping, the
/// non-finite-is-floor rule, and the always-opaque alpha identical between the
/// built-in and custom-LUT paths.
fn colorize_with(
    tile_db: &[f32],
    w: u32,
    h: u32,
    map: &DisplayMapping,
    sample: impl Fn(f32) -> [u8; 3],
) -> Vec<u8> {
    let expected_len = w as usize * h as usize;
    assert_eq!(
        tile_db.len(),
        expected_len,
        "tile_db has {} values, expected w*h = {expected_len}",
        tile_db.len()
    );

    let (floor_db, ceiling_db) = map.resolve(tile_db);
    let span = ceiling_db - floor_db;

    let mut out = Vec::with_capacity(expected_len * 4);
    for &db in tile_db {
        let t = if !db.is_finite() {
            0.0
        } else if span > 0.0 {
            (((db as f64) - floor_db) / span).clamp(0.0, 1.0) as f32
        } else if (db as f64) >= ceiling_db {
            1.0
        } else {
            0.0
        };
        let [r, g, b] = sample(t);
        out.extend_from_slice(&[r, g, b, 255]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_length_matches_tile_dimensions() {
        let tile = vec![-10.0f32; 6];
        let out = colorize(
            &tile,
            3,
            2,
            &DisplayMapping::default(),
            Colormap::Viridis,
            Theme::Light,
        );
        assert_eq!(out.len(), 6 * 4);
    }

    #[test]
    fn alpha_channel_is_always_opaque() {
        let tile = [f32::NEG_INFINITY, 0.0, -1000.0, 5.0];
        let out = colorize(
            &tile,
            4,
            1,
            &DisplayMapping::default(),
            Colormap::Magma,
            Theme::Dark,
        );
        for chunk in out.chunks(4) {
            assert_eq!(chunk[3], 255);
        }
    }

    #[test]
    fn silence_and_clipped_low_values_map_to_the_floor_color() {
        let map = DisplayMapping {
            dynamic_range_db: 50.0,
            max_db: Some(0.0),
        };
        let tile = [f32::NEG_INFINITY, -1000.0, -50.0];
        let out = colorize(&tile, 3, 1, &map, Colormap::Viridis, Theme::Light);
        let floor = out[0..3].to_vec();
        assert_eq!(out[4..7], floor[..]);
        assert_eq!(out[8..11], floor[..]);
    }

    #[test]
    fn autoscale_uses_tile_maximum_finite_value() {
        let map = DisplayMapping::default();
        let tile = [-80.0f32, -10.0, f32::NEG_INFINITY];
        let out = colorize(&tile, 3, 1, &map, Colormap::Grayscale, Theme::Light);
        // -10 dB is the autoscaled ceiling -> t=1 -> black on the light
        // grayscale ramp.
        assert_eq!(&out[4..8], &[0, 0, 0, 255]);
    }

    #[test]
    fn colorize_with_lut_maps_floor_and_ceiling_to_the_lut_endpoints() {
        let mut lut = [[0u8; 3]; 256];
        for (i, entry) in lut.iter_mut().enumerate() {
            *entry = [i as u8, i as u8, i as u8];
        }
        let map = DisplayMapping {
            dynamic_range_db: 50.0,
            max_db: Some(0.0),
        };
        // t = [0.0, 0.5, 1.0] over the 50 dB window.
        let tile = [-100.0f32, -25.0, 0.0];
        let out = colorize_with_lut(&tile, 3, 1, &map, &lut);
        assert_eq!(&out[0..4], &[0, 0, 0, 255]);
        assert_eq!(&out[8..12], &[255, 255, 255, 255]);
        // Midpoint interpolates between entries 127 and 128.
        assert_eq!(&out[4..8], &[128, 128, 128, 255]);
    }

    #[test]
    #[should_panic]
    fn panics_on_length_mismatch() {
        let tile = vec![0.0f32; 3];
        colorize(
            &tile,
            2,
            2,
            &DisplayMapping::default(),
            Colormap::Viridis,
            Theme::Light,
        );
    }
}
