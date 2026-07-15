//! PNG backend: rasterize the SVG scene graph.
//!
//! [`to_png`] renders the figure to SVG once, parses it with usvg, and paints
//! it with resvg + tiny-skia at a chosen DPI. The PNG is a rasterization of the
//! exact same scene the SVG and PDF backends use; nothing about the layout is
//! recomputed here.

use resvg::tiny_skia;
use resvg::usvg;

use crate::backends::svg::to_svg;
use crate::backends::{BUNDLED_FONT, BUNDLED_FONT_FAMILY};
use crate::model::Figure;

/// A PNG rendering failure.
#[derive(Debug)]
pub enum PngError {
    /// The intermediate SVG could not be parsed.
    Parse(usvg::Error),
    /// The target pixel size was zero or too large to allocate.
    Size {
        /// Requested width in pixels.
        width: u32,
        /// Requested height in pixels.
        height: u32,
    },
    /// The pixmap could not be PNG-encoded.
    Encode(png::EncodingError),
}

impl std::fmt::Display for PngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PngError::Parse(e) => write!(f, "svg parse failed: {e}"),
            PngError::Size { width, height } => {
                write!(f, "invalid raster size {width}x{height}")
            }
            PngError::Encode(e) => write!(f, "png encode failed: {e}"),
        }
    }
}

impl std::error::Error for PngError {}

/// Rasterizes `fig` to a PNG at `dpi` dots per inch.
///
/// The pixel dimensions are the figure's physical size resolved at `dpi`
/// (`fig.size.px_at(dpi)`), so a caller sets output resolution purely through
/// `dpi`. The embedded spectrogram is already colorized in the SVG; usvg
/// decodes it and resvg composites it with the vector layers.
///
/// # Errors
/// Returns [`PngError`] if the intermediate SVG fails to parse, the target
/// size is degenerate, or PNG encoding fails.
pub fn to_png(fig: &Figure, dpi: f64) -> Result<Vec<u8>, PngError> {
    let svg = to_svg(fig);
    let tree = parse_tree(&svg)?;

    let (w, h) = fig.size.px_at(dpi);
    if w == 0 || h == 0 {
        return Err(PngError::Size {
            width: w,
            height: h,
        });
    }
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or(PngError::Size {
        width: w,
        height: h,
    })?;

    // The SVG is authored in 96-DPI user units; scale so the physical figure
    // fills the requested pixel canvas.
    let scale = (dpi / 96.0) as f32;
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap.encode_png().map_err(PngError::Encode)
}

/// Parse an SVG string into a usvg tree with the bundled font registered as
/// the default family, so text renders even without system fonts.
fn parse_tree(svg: &str) -> Result<usvg::Tree, PngError> {
    let mut options = usvg::Options {
        font_family: BUNDLED_FONT_FAMILY.to_owned(),
        ..usvg::Options::default()
    };
    let fontdb = options.fontdb_mut();
    fontdb.load_font_data(BUNDLED_FONT.to_vec());
    fontdb.load_system_fonts();
    fontdb.set_sans_serif_family(BUNDLED_FONT_FAMILY);
    usvg::Tree::from_str(svg, &options).map_err(PngError::Parse)
}
