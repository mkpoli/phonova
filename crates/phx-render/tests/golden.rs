//! Golden-image tests: one fixed 4-pixel tile, colorized with every
//! `(Colormap, Theme)` combination, compared byte-for-byte against a raw
//! RGBA fixture under `tests/golden/`.
//!
//! The tile carries dB values `[-100, -50, -25, 0]` under
//! `DisplayMapping { dynamic_range_db: 50.0, max_db: Some(0.0) }`, giving
//! normalized positions `[0.0, 0.0, 0.5, 1.0]` — the display floor
//! (clipped and exact), the ramp midpoint, and the ceiling. Fixture bytes
//! were derived independently from the published viridis/magma
//! control-point data and the documented grayscale endpoints, not
//! captured from this crate's own output.

use phx_render::{Colormap, DisplayMapping, Theme, colorize};

const TILE_DB: [f32; 4] = [-100.0, -50.0, -25.0, 0.0];
const MAPPING: DisplayMapping = DisplayMapping {
    dynamic_range_db: 50.0,
    max_db: Some(0.0),
};

fn check(colormap: Colormap, theme: Theme, fixture_bytes: &[u8]) {
    let out = colorize(&TILE_DB, 4, 1, &MAPPING, colormap, theme);
    assert_eq!(
        out, fixture_bytes,
        "{colormap:?}/{theme:?} tile does not match golden fixture"
    );
}

#[test]
fn viridis_light_matches_golden() {
    check(
        Colormap::Viridis,
        Theme::Light,
        include_bytes!("golden/viridis_light.rgba"),
    );
}

#[test]
fn viridis_dark_matches_golden() {
    check(
        Colormap::Viridis,
        Theme::Dark,
        include_bytes!("golden/viridis_dark.rgba"),
    );
}

#[test]
fn magma_light_matches_golden() {
    check(
        Colormap::Magma,
        Theme::Light,
        include_bytes!("golden/magma_light.rgba"),
    );
}

#[test]
fn magma_dark_matches_golden() {
    check(
        Colormap::Magma,
        Theme::Dark,
        include_bytes!("golden/magma_dark.rgba"),
    );
}

#[test]
fn grayscale_light_matches_golden() {
    check(
        Colormap::Grayscale,
        Theme::Light,
        include_bytes!("golden/grayscale_light.rgba"),
    );
}

#[test]
fn grayscale_dark_matches_golden() {
    check(
        Colormap::Grayscale,
        Theme::Dark,
        include_bytes!("golden/grayscale_dark.rgba"),
    );
}

/// Viridis and magma are theme-independent: the golden fixtures for light
/// and dark are byte-identical, and this asserts that invariant directly
/// rather than only through the duplicated fixture files.
#[test]
fn viridis_and_magma_ignore_theme() {
    let light = colorize(&TILE_DB, 4, 1, &MAPPING, Colormap::Viridis, Theme::Light);
    let dark = colorize(&TILE_DB, 4, 1, &MAPPING, Colormap::Viridis, Theme::Dark);
    assert_eq!(light, dark);

    let light = colorize(&TILE_DB, 4, 1, &MAPPING, Colormap::Magma, Theme::Light);
    let dark = colorize(&TILE_DB, 4, 1, &MAPPING, Colormap::Magma, Theme::Dark);
    assert_eq!(light, dark);
}

/// Grayscale is never a naive channel inversion between themes: the light
/// floor/ceiling pair must not simply be the dark ramp's ceiling/floor
/// swapped.
#[test]
fn grayscale_dark_is_not_the_light_ramp_inverted() {
    let light = colorize(&TILE_DB, 4, 1, &MAPPING, Colormap::Grayscale, Theme::Light);
    let dark = colorize(&TILE_DB, 4, 1, &MAPPING, Colormap::Grayscale, Theme::Dark);
    assert_ne!(light, dark);
    // A naive inversion would reuse the same 0/255 endpoints; the dark
    // floor color must differ from pure black and the ceiling from pure
    // white.
    assert_ne!(&dark[0..3], &[0, 0, 0]);
    assert_ne!(&dark[12..15], &[255, 255, 255]);
}
