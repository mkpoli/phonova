//! Visual styling for figure layers.
//!
//! Styles carry the colors, line weights, and text sizes a backend needs to
//! draw a layer. Line weights and text sizes are in typographic points so
//! they resolve the same way as [`crate::model::SizeSpec`]. Colors are stored
//! as explicit sRGBA; the spectrogram is the only layer re-colorized per
//! theme at export time, so every other layer commits its color here.

use serde::{Deserialize, Serialize};

/// 8-bit sRGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbaColor {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel; `255` is fully opaque.
    pub a: u8,
}

impl RgbaColor {
    /// Opaque color from red, green, and blue channels.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Color from red, green, blue, and alpha channels.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Dash pattern for a stroked path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DashStyle {
    /// Unbroken stroke.
    #[default]
    Solid,
    /// Long dashes.
    Dashed,
    /// Short dots.
    Dotted,
}

/// Stroke style for line and boundary layers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LineStyle {
    /// Stroke width in typographic points.
    pub width_pt: f64,
    /// Stroke color.
    pub color: RgbaColor,
    /// Dash pattern.
    pub dash: DashStyle,
}

impl LineStyle {
    /// Solid stroke of `width_pt` points in `color`.
    #[must_use]
    pub const fn solid(width_pt: f64, color: RgbaColor) -> Self {
        Self {
            width_pt,
            color,
            dash: DashStyle::Solid,
        }
    }
}

impl Default for LineStyle {
    /// One-point solid black stroke.
    fn default() -> Self {
        Self::solid(1.0, RgbaColor::rgb(0, 0, 0))
    }
}

/// Marker style for a formant speckle layer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpeckleStyle {
    /// Marker radius in typographic points.
    pub radius_pt: f64,
    /// Marker fill color.
    pub color: RgbaColor,
}

impl Default for SpeckleStyle {
    /// One-point red dots, the conventional formant speckle.
    fn default() -> Self {
        Self {
            radius_pt: 1.0,
            color: RgbaColor::rgb(200, 0, 0),
        }
    }
}

/// Style for a tier layer: boundary strokes plus label text.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TierStyle {
    /// Stroke for interval boundaries and point markers.
    pub boundary: LineStyle,
    /// Label text size in typographic points.
    pub text_pt: f64,
    /// Label text color.
    pub text_color: RgbaColor,
}

impl Default for TierStyle {
    /// Thin gray boundaries with 10-point black labels.
    fn default() -> Self {
        Self {
            boundary: LineStyle::solid(0.5, RgbaColor::rgb(120, 120, 120)),
            text_pt: 10.0,
            text_color: RgbaColor::rgb(0, 0, 0),
        }
    }
}
