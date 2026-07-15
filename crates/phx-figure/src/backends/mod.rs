//! Rendering backends.
//!
//! The SVG backend is the scene-graph source of truth and is always available,
//! including on wasm. The PNG and PDF backends derive from the same SVG string
//! rather than re-running layout: [`to_png`] rasterizes it, [`to_pdf`] converts
//! it. Both pull a native font and raster stack, so they sit behind the
//! `raster` and `pdf` features that the wasm build leaves off.

pub mod svg;
pub use svg::to_svg;

#[cfg(feature = "raster")]
pub mod png;
#[cfg(feature = "raster")]
pub use png::{PngError, to_png};

#[cfg(feature = "pdf")]
pub mod pdf;
#[cfg(feature = "pdf")]
pub use pdf::{PdfError, to_pdf};

/// The bundled text face embedded into raster and PDF output, so exports carry
/// legible labels without depending on a system font being installed. A viewer
/// still substitutes a system face for on-screen SVG per the CSS font stack.
#[cfg(any(feature = "raster", feature = "pdf"))]
pub(crate) const BUNDLED_FONT: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");

/// The family name of [`BUNDLED_FONT`], set as the default for text that names
/// no family and matching the first entry of the SVG font stack.
#[cfg(any(feature = "raster", feature = "pdf"))]
pub(crate) const BUNDLED_FONT_FAMILY: &str = "DejaVu Sans";
