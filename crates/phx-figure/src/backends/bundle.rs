//! Shared carriers and helpers for the text-emission backends.
//!
//! A text or code export is a small bundle: one main document plus named
//! sidecar files (a spectrogram image, or the CSV and matrix data a generated
//! script reads). The UI writes a bundle to disk verbatim, zipping it when it
//! holds more than the main file.
//!
//! The helpers here are the pieces every text backend shares: colorizing a
//! raw-decibel tile to a deterministic PNG, formatting numbers compactly, and
//! escaping label text for the target syntaxes.

use phx_render::{Colormap, DisplayMapping, Theme, colorize};

/// A named file emitted alongside an export's main document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarFile {
    /// File name relative to the bundle root, e.g. `spectrogram.png`.
    pub name: String,
    /// Raw file contents.
    pub bytes: Vec<u8>,
}

impl SidecarFile {
    /// A sidecar holding UTF-8 text.
    #[must_use]
    pub fn text(name: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bytes: contents.into().into_bytes(),
        }
    }

    /// A sidecar holding raw bytes.
    #[must_use]
    pub fn binary(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            bytes,
        }
    }
}

/// A text export: one main source document plus its named sidecars.
///
/// The main document references each sidecar by name, so writing every file to
/// one directory reproduces a self-contained, compilable source tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextExport {
    /// Suggested file name for the main document, e.g. `figure.tex`.
    pub main_name: String,
    /// The main document source.
    pub main: String,
    /// Files the main document references, e.g. a spectrogram image.
    pub sidecars: Vec<SidecarFile>,
}

/// A generated-code export: one script plus the data files it reads.
///
/// The script carries the figure's structure and styling inline and reads only
/// bulk arrays — one CSV per track or tier layer, one matrix per spectrogram —
/// from `data_files`, so the script text stays small regardless of clip length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeExport {
    /// Suggested file name for the script, e.g. `figure.py`.
    pub script_name: String,
    /// The generated script source.
    pub script: String,
    /// Data files the script reads, named relative to the script.
    pub data_files: Vec<SidecarFile>,
}

/// Target language for [`crate::to_code`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLang {
    /// A matplotlib script (no seaborn).
    Python,
    /// A base-ggplot2 script.
    R,
    /// A Makie script (CairoMakie).
    Julia,
}

/// Colorize a raw-decibel tile against `theme` and encode it as a PNG whose
/// row 0 is the highest frequency (image top).
///
/// The tile stores row 0 at the lowest frequency; an image places row 0 at the
/// top, so the rows are flipped before encoding. Encoding uses fixed,
/// non-adaptive settings, so the same tile yields byte-identical PNG bytes.
#[must_use]
pub(crate) fn spectrogram_png(
    db: &[f32],
    width: u32,
    height: u32,
    display: &DisplayMapping,
    colormap: Colormap,
    theme: Theme,
) -> Vec<u8> {
    let flat = colorize(db, width, height, display, colormap, theme);
    let (w, h) = (width as usize, height as usize);
    let stride = w * 4;
    let mut flipped = vec![0u8; flat.len()];
    for row in 0..h {
        let src = (h - 1 - row) * stride;
        let dst = row * stride;
        flipped[dst..dst + stride].copy_from_slice(&flat[src..src + stride]);
    }

    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(&flipped).expect("png data");
    }
    out
}

/// Format a floating-point value compactly and deterministically: up to six
/// decimals, trailing zeros trimmed, negative zero collapsed to `0`.
#[must_use]
pub(crate) fn fnum(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let v = if v == 0.0 { 0.0 } else { v };
    let mut out = format!("{v:.6}");
    if out.contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
    out
}

/// The axis title `"Label (unit)"`, falling back to whichever part is present.
#[must_use]
pub(crate) fn axis_title(axis: &crate::model::Axis) -> Option<String> {
    match (&axis.label, &axis.unit) {
        (Some(l), Some(u)) => Some(format!("{l} ({u})")),
        (Some(l), None) => Some(l.clone()),
        (None, Some(u)) => Some(u.clone()),
        (None, None) => None,
    }
}

/// The matplotlib/Makie colormap name for a [`Colormap`].
#[must_use]
pub(crate) fn colormap_name(cm: Colormap) -> &'static str {
    match cm {
        // Phonia is the on-screen brand ramp; figures export the perceptual and
        // grayscale built-ins, so a code backend never names it in practice.
        // Viridis is the closest widely available reproduction for a re-colorizing
        // script that somehow receives it.
        Colormap::Phonia => "viridis",
        Colormap::Viridis => "viridis",
        Colormap::Magma => "magma",
        Colormap::Inferno => "inferno",
        Colormap::Plasma => "plasma",
        Colormap::Cividis => "cividis",
        Colormap::Grayscale => "gray",
    }
}

/// Resolve the `(vmin, vmax)` decibel clip window a backend colormap uses, so a
/// re-colorizing backend (matplotlib, ggplot2, Makie) matches the display
/// mapping the model carries.
#[must_use]
pub(crate) fn db_window(db: &[f32], display: &DisplayMapping) -> (f64, f64) {
    let vmax = display.max_db.unwrap_or_else(|| {
        db.iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(f32::NEG_INFINITY, f32::max) as f64
    });
    let vmax = if vmax.is_finite() { vmax } else { 0.0 };
    (vmax - display.dynamic_range_db, vmax)
}

/// One `#RRGGBB` hex string for an opaque sRGB triple.
#[must_use]
pub(crate) fn hex(color: crate::style::RgbaColor) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// Deterministic "nice" tick positions across `[min, max]`, roughly `target` of
/// them, snapped to 1/2/5 times a power of ten. Matches the SVG backend so
/// exports agree on tick placement.
#[must_use]
pub(crate) fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    if !min.is_finite() || !max.is_finite() || max <= min {
        return vec![min];
    }
    let range = max - min;
    let raw = range / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    } * mag;
    let start = (min / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut x = start;
    let mut guard = 0;
    while x <= max + step * 1e-6 && guard < 1000 {
        ticks.push(x);
        x += step;
        guard += 1;
    }
    if ticks.is_empty() {
        ticks.push(min);
    }
    ticks
}

/// The spacing between adjacent ticks, or `1.0` for a lone tick.
#[must_use]
pub(crate) fn tick_step(ticks: &[f64]) -> f64 {
    if ticks.len() >= 2 {
        (ticks[1] - ticks[0]).abs()
    } else {
        1.0
    }
}

/// Format a tick label at a precision matched to the tick step.
#[must_use]
pub(crate) fn fmt_tick(v: f64, step: f64) -> String {
    let v = if v.abs() < step * 1e-9 { 0.0 } else { v };
    let dec = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    };
    format!("{v:.dec$}")
}
