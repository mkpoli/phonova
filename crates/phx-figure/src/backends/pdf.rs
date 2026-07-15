//! PDF backend: convert the SVG scene graph with svg2pdf.
//!
//! [`to_pdf`] renders the figure to SVG once and hands it to svg2pdf, which
//! parses it with usvg and writes a single-page vector PDF. Tracks, axes, and
//! text stay vector in the PDF; the embedded spectrogram PNG is carried through
//! as an image. Layout is never recomputed — the PDF is the SVG converted.

use svg2pdf::usvg;

use crate::backends::svg::to_svg;
use crate::backends::{BUNDLED_FONT, BUNDLED_FONT_FAMILY};
use crate::model::Figure;

/// A PDF conversion failure.
#[derive(Debug)]
pub enum PdfError {
    /// The intermediate SVG could not be parsed.
    Parse(usvg::Error),
    /// svg2pdf could not convert the parsed tree.
    Convert(svg2pdf::ConversionError),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfError::Parse(e) => write!(f, "svg parse failed: {e}"),
            PdfError::Convert(e) => write!(f, "pdf conversion failed: {e}"),
        }
    }
}

impl std::error::Error for PdfError {}

/// Converts `fig` to a single-page PDF.
///
/// The page size is the figure's physical size: svg2pdf reads the SVG's
/// dimensions and emits a page in typographic points. Text is shaped with the
/// bundled font so labels render without a system font present.
///
/// # Errors
/// Returns [`PdfError`] if the intermediate SVG fails to parse or svg2pdf
/// cannot convert it.
pub fn to_pdf(fig: &Figure) -> Result<Vec<u8>, PdfError> {
    let svg = to_svg(fig);

    let mut options = usvg::Options {
        font_family: BUNDLED_FONT_FAMILY.to_owned(),
        ..usvg::Options::default()
    };
    let fontdb = options.fontdb_mut();
    fontdb.load_font_data(BUNDLED_FONT.to_vec());
    fontdb.load_system_fonts();
    fontdb.set_sans_serif_family(BUNDLED_FONT_FAMILY);

    let tree = usvg::Tree::from_str(&svg, &options).map_err(PdfError::Parse)?;
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(PdfError::Convert)
}
