//! The backend-agnostic figure description.
//!
//! A [`Figure`] is a self-contained snapshot: every layer carries its own
//! embedded data, so an exporter never reaches back to an engine, an audio
//! store, or a live analysis. Waveforms carry min/max envelopes, spectrograms
//! carry raw decibel matrices (re-colorized per theme at export, never baked
//! to RGBA), and tracks and tiers carry plain point and interval data. The
//! model derives serde on every type; it doubles as the dialog↔worker wire
//! format and serializes to JSON deterministically.
//!
//! Physical size and stroke units resolve here. [`SizeSpec::px_at`] is the one
//! place that turns centimeters, inches, or points into pixels; no backend
//! does unit arithmetic.

use std::collections::BTreeMap;

use phx_pitch::TimeSpan;
use phx_render::{Colormap, DisplayMapping, Theme};
use serde::{Deserialize, Serialize};

use crate::style::{LineStyle, SpeckleStyle, TierStyle};

/// A physical length unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    /// Centimeters.
    Cm,
    /// Inches.
    In,
    /// Typographic points (1/72 inch).
    Pt,
}

impl LengthUnit {
    /// Length of one unit expressed in inches.
    #[must_use]
    fn inches_per_unit(self) -> f64 {
        match self {
            LengthUnit::Cm => 1.0 / 2.54,
            LengthUnit::In => 1.0,
            LengthUnit::Pt => 1.0 / 72.0,
        }
    }
}

/// Physical figure size in a single [`LengthUnit`].
///
/// The figure is described in real-world dimensions; pixel dimensions are a
/// property of an export at a chosen resolution, produced by [`Self::px_at`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SizeSpec {
    /// Width in `unit`.
    pub width: f64,
    /// Height in `unit`.
    pub height: f64,
    /// Unit both dimensions are expressed in.
    pub unit: LengthUnit,
}

impl SizeSpec {
    /// Size of `width` × `height` in `unit`.
    #[must_use]
    pub const fn new(width: f64, height: f64, unit: LengthUnit) -> Self {
        Self {
            width,
            height,
            unit,
        }
    }

    /// Width in inches.
    #[must_use]
    pub fn width_in(&self) -> f64 {
        self.width * self.unit.inches_per_unit()
    }

    /// Height in inches.
    #[must_use]
    pub fn height_in(&self) -> f64 {
        self.height * self.unit.inches_per_unit()
    }

    /// Pixel dimensions at `dpi` dots per inch, rounded to the nearest pixel.
    #[must_use]
    pub fn px_at(&self, dpi: f64) -> (u32, u32) {
        let w = (self.width_in() * dpi).round().max(0.0);
        let h = (self.height_in() * dpi).round().max(0.0);
        (w as u32, h as u32)
    }
}

/// Whether an axis is linear or logarithmic in its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AxisScale {
    /// Linear scale.
    #[default]
    Linear,
    /// Base-10 logarithmic scale.
    Log,
}

/// A panel axis: value range, optional label and unit, and scale.
///
/// Tick placement is left to the backend; the model fixes only the numeric
/// range so exports agree on extent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Axis {
    /// Lower bound in the axis unit.
    pub min: f64,
    /// Upper bound in the axis unit.
    pub max: f64,
    /// Axis label, e.g. `"Time"` or `"Frequency"`.
    pub label: Option<String>,
    /// Axis unit, e.g. `"s"` or `"Hz"`.
    pub unit: Option<String>,
    /// Linear or logarithmic scale.
    pub scale: AxisScale,
}

impl Axis {
    /// Linear axis over `[min, max]` with an optional label and unit.
    #[must_use]
    pub fn linear(min: f64, max: f64, label: Option<&str>, unit: Option<&str>) -> Self {
        Self {
            min,
            max,
            label: label.map(str::to_owned),
            unit: unit.map(str::to_owned),
            scale: AxisScale::Linear,
        }
    }
}

/// Minimum and maximum sample value over a waveform bucket.
///
/// One [`MinMax`] per horizontal pixel of the waveform envelope, mirroring the
/// engine's waveform pyramid buckets but holding no engine handle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MinMax {
    /// Lowest sample value in the bucket.
    pub min: f32,
    /// Highest sample value in the bucket.
    pub max: f32,
}

/// Unit a pitch line is expressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PitchUnit {
    /// Hertz.
    Hertz,
    /// Semitones re 1 Hz.
    Semitones,
}

/// One formant candidate embedded in a speckle frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpecklePoint {
    /// Formant frequency in hertz.
    pub frequency: f64,
    /// Formant bandwidth in hertz.
    pub bandwidth: f64,
}

/// Formant candidates at one analysis time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeckleFrame {
    /// Frame center time in seconds.
    pub time: f64,
    /// Formant candidates at this time.
    pub points: Vec<SpecklePoint>,
}

/// An embedded interval from an interval tier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntervalData {
    /// Start time in seconds.
    pub xmin: f64,
    /// End time in seconds.
    pub xmax: f64,
    /// Interval label; empty for an unlabeled interval.
    pub label: String,
}

/// An embedded point from a point tier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointData {
    /// Point time in seconds.
    pub time: f64,
    /// Point label.
    pub label: String,
}

/// Tier payload embedded in a figure: intervals or points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TierContent {
    /// Contiguous labeled intervals.
    Intervals(Vec<IntervalData>),
    /// Labeled points.
    Points(Vec<PointData>),
}

/// A named tier with its embedded content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierData {
    /// Tier display name.
    pub name: String,
    /// Interval or point content.
    pub content: TierContent,
}

/// A drawable layer with all of its data embedded.
///
/// No variant references an audio id or engine handle: a figure is fully
/// determined by its own bytes. The [`Layer::Spectrogram`] variant stores raw
/// decibels together with the display mapping and colormap, so a backend calls
/// [`phx_render::colorize`] at export time — the theme decides the pixels, the
/// figure never bakes them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Layer {
    /// Waveform min/max envelope over `span`.
    Waveform {
        /// One bucket per envelope column.
        minmax: Vec<MinMax>,
        /// Time span the envelope covers.
        #[serde(with = "TimeSpanDef")]
        span: TimeSpan,
        /// Stroke style.
        style: LineStyle,
    },
    /// Raw-decibel spectrogram, colorized per theme at export time.
    Spectrogram {
        /// Row-major decibel matrix, row 0 at the lowest frequency, length
        /// `width * height`.
        db: Vec<f32>,
        /// Column count.
        width: u32,
        /// Row count.
        height: u32,
        /// Time range `[t0, t1]` in seconds.
        t: [f64; 2],
        /// Frequency range `[f0, f1]` in hertz.
        f: [f64; 2],
        /// dB→normalized display mapping applied before colormap lookup.
        #[serde(with = "DisplayMappingDef")]
        display: DisplayMapping,
        /// Colormap sampled at export time.
        #[serde(with = "ColormapDef")]
        colormap: Colormap,
    },
    /// Pitch contour as `(time, value)` points in `unit`.
    PitchLine {
        /// Voiced points; unvoiced frames are omitted.
        points: Vec<(f64, f64)>,
        /// Unit of the second coordinate.
        unit: PitchUnit,
        /// Stroke style.
        style: LineStyle,
    },
    /// Formant candidates drawn as a speckle.
    FormantSpeckle {
        /// Candidate frames in time order.
        frames: Vec<SpeckleFrame>,
        /// Whether the candidates are Viterbi-smoothed tracks.
        smoothed: bool,
        /// Marker style.
        style: SpeckleStyle,
    },
    /// Intensity contour as `(time, dB)` points.
    IntensityLine {
        /// One point per analysis frame.
        points: Vec<(f64, f64)>,
        /// Stroke style.
        style: LineStyle,
    },
    /// Annotation tiers.
    Tiers {
        /// Embedded tiers in draw order.
        tiers: Vec<TierData>,
        /// Tier style.
        style: TierStyle,
    },
    /// A single spectral slice as `(frequency, dB)` bins.
    SpectralSlice {
        /// One bin per frequency coordinate.
        bins: Vec<(f64, f64)>,
        /// Stroke style.
        style: LineStyle,
    },
}

impl Layer {
    /// The [`LayerKind`] tag for this layer.
    #[must_use]
    pub fn kind(&self) -> LayerKind {
        match self {
            Layer::Waveform { .. } => LayerKind::Waveform,
            Layer::Spectrogram { .. } => LayerKind::Spectrogram,
            Layer::PitchLine { .. } => LayerKind::Pitch,
            Layer::FormantSpeckle { .. } => LayerKind::Formant,
            Layer::IntensityLine { .. } => LayerKind::Intensity,
            Layer::Tiers { .. } => LayerKind::Tiers,
            Layer::SpectralSlice { .. } => LayerKind::SpectralSlice,
        }
    }
}

/// A tag naming a kind of layer, used in caption provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    /// Waveform envelope.
    Waveform,
    /// Spectrogram.
    Spectrogram,
    /// Pitch contour.
    Pitch,
    /// Formant speckle.
    Formant,
    /// Intensity contour.
    Intensity,
    /// Annotation tiers.
    Tiers,
    /// Spectral slice.
    SpectralSlice,
}

/// A stacked panel: layers sharing one time axis and one value axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    /// Layers drawn back to front.
    pub layers: Vec<Layer>,
    /// Shared horizontal (time) axis.
    pub time_axis: Axis,
    /// Shared vertical (value) axis.
    pub value_axis: Axis,
    /// Fraction of the figure height this panel receives, relative to the sum
    /// of all panels' shares.
    pub height_share: f64,
}

/// Provenance for one analysis feeding a figure.
///
/// `params` keeps analysis parameters in sorted key order so the record
/// serializes deterministically. `smoothed` is set for formant layers per the
/// formant-tracking caveat: raw and smoothed tracks are marked distinctly, and
/// exports carry the mark until the tracking weights graduate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    /// Layer this record describes.
    pub layer: LayerKind,
    /// Analysis parameters as name→value strings, in sorted key order.
    pub params: BTreeMap<String, String>,
    /// `Some(true)` for a smoothed formant track, `Some(false)` for a raw one,
    /// `None` when smoothing does not apply.
    pub smoothed: Option<bool>,
}

/// Caption metadata: the provenance of every analysis in the figure.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CaptionMeta {
    /// One record per analysis, in the order added.
    pub sources: Vec<ProvenanceRecord>,
}

/// A complete, self-contained figure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Figure {
    /// Physical size.
    pub size: SizeSpec,
    /// Theme the figure is described against; spectrogram layers colorize to
    /// it at export time.
    #[serde(with = "ThemeDef")]
    pub theme: Theme,
    /// Stacked panels, top to bottom.
    pub panels: Vec<Panel>,
    /// Analysis provenance for the caption.
    pub caption_meta: CaptionMeta,
}

impl Figure {
    /// Checks the invariants the model guarantees to every backend.
    ///
    /// # Errors
    /// Returns the first invariant violation found: a non-positive or
    /// non-finite size, a non-positive panel height share, a spectrogram whose
    /// `db` length does not equal `width * height`, or a zero-dimensioned
    /// spectrogram.
    pub fn validate(&self) -> Result<(), FigureError> {
        let size_ok = self.size.width.is_finite()
            && self.size.width > 0.0
            && self.size.height.is_finite()
            && self.size.height > 0.0;
        if !size_ok {
            return Err(FigureError::NonPositiveSize);
        }
        for panel in &self.panels {
            if !(panel.height_share.is_finite() && panel.height_share > 0.0) {
                return Err(FigureError::NonPositiveHeightShare);
            }
            for layer in &panel.layers {
                if let Layer::Spectrogram {
                    db, width, height, ..
                } = layer
                {
                    if *width == 0 || *height == 0 {
                        return Err(FigureError::EmptySpectrogram);
                    }
                    let expected = *width as usize * *height as usize;
                    if db.len() != expected {
                        return Err(FigureError::SpectrogramShape {
                            expected,
                            found: db.len(),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Serializes the figure to a JSON string.
    ///
    /// The output is deterministic: serializing the same figure twice yields
    /// byte-identical JSON.
    ///
    /// # Errors
    /// Propagates any [`serde_json`] serialization error.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserializes a figure from a JSON string produced by [`Self::to_json`].
    ///
    /// # Errors
    /// Propagates any [`serde_json`] deserialization error.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// A figure model invariant violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigureError {
    /// Figure width or height is not a positive finite number.
    NonPositiveSize,
    /// A panel height share is not a positive finite number.
    NonPositiveHeightShare,
    /// A spectrogram layer has a zero width or height.
    EmptySpectrogram,
    /// A spectrogram `db` length does not match `width * height`.
    SpectrogramShape {
        /// Length implied by `width * height`.
        expected: usize,
        /// Actual `db` length.
        found: usize,
    },
}

impl std::fmt::Display for FigureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigureError::NonPositiveSize => {
                write!(f, "figure size must be positive and finite")
            }
            FigureError::NonPositiveHeightShare => {
                write!(f, "panel height share must be positive and finite")
            }
            FigureError::EmptySpectrogram => {
                write!(f, "spectrogram layer has a zero dimension")
            }
            FigureError::SpectrogramShape { expected, found } => write!(
                f,
                "spectrogram db length {found} does not equal width*height {expected}"
            ),
        }
    }
}

impl std::error::Error for FigureError {}

// serde shims for the committed types this crate embeds but does not own.
// Remote derive adds serialization without touching the source crates; the
// mirror fields match the remote types exactly.

/// serde mirror of [`phx_render::Theme`].
#[derive(Serialize, Deserialize)]
#[serde(remote = "Theme")]
enum ThemeDef {
    Light,
    Dark,
}

/// serde mirror of [`phx_render::Colormap`].
#[derive(Serialize, Deserialize)]
#[serde(remote = "Colormap")]
enum ColormapDef {
    Phonia,
    Viridis,
    Magma,
    Inferno,
    Plasma,
    Cividis,
    Golden,
    Grayscale,
}

/// serde mirror of [`phx_render::DisplayMapping`].
#[derive(Serialize, Deserialize)]
#[serde(remote = "DisplayMapping")]
struct DisplayMappingDef {
    dynamic_range_db: f64,
    max_db: Option<f64>,
}

/// serde mirror of [`phx_pitch::TimeSpan`].
#[derive(Serialize, Deserialize)]
#[serde(remote = "TimeSpan")]
struct TimeSpanDef {
    start: f64,
    end: f64,
}
