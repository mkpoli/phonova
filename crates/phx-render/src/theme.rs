//! UI theme the tile is rendered against.

/// Light or dark UI theme.
///
/// Only [`crate::Colormap::Grayscale`] changes shape between themes —
/// viridis and magma were designed as fixed perceptual ramps and render
/// identically in both, per `docs/plan/ux.md` ("palettes are defined
/// against both backgrounds, never inverted").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Theme {
    /// Light application background.
    Light,
    /// Dark application background.
    Dark,
}
