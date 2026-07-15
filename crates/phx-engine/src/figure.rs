//! Assembling and exporting publication figures from live session state.
//!
//! [`Engine::build_figure`] reads the audio store and annotation documents the
//! rest of the engine already serves the editor overlays from, and produces a
//! self-contained [`phx_figure::Figure`]: every layer carries embedded data, so
//! the figure travels as JSON and renders identically wherever it lands.
//! [`export_figure`] turns that figure into a downloadable [`ExportBundle`]
//! through one of the [`phx_figure`] backends. The SVG backend is the preview
//! and the export at once, so a dialog previewing `to_svg` shows the exact bytes
//! an SVG export writes.

use std::collections::BTreeMap;

use phx_figure::{
    Axis, CodeExport, CodeLang, Figure, FigureBuilder, LayerKind, LengthUnit, LineStyle, Panel,
    PitchUnit, ProvenanceRecord, SizeSpec, SpeckleStyle, TextExport, TierStyle, formant_layer,
    intensity_layer, pitch_layer, spectrogram_layer, tiers_layer, to_code, to_graphml, to_svg,
    to_tikz, to_typst, to_vega, waveform_layer, waveform_minmax,
};
use phx_pitch::TimeSpan;
use phx_render::{Colormap, DisplayMapping, Theme};
use phx_spectrogram::{SpectrogramParams, TileRequest, compute_tile};
use serde::Deserialize;

use crate::document::AnnotationId;
use crate::error::EngineError;
use crate::store::AudioId;
use crate::{Engine, FormantParams, IntensityParams, PitchParams};

/// Which layers a figure includes, mirroring the editor's per-track toggles.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct LayerToggles {
    /// Waveform envelope panel.
    pub waveform: bool,
    /// Spectrogram panel.
    pub spectrogram: bool,
    /// Pitch contour panel.
    pub pitch: bool,
    /// Formant speckle, drawn over the spectrogram's frequency panel.
    pub formant: bool,
    /// Intensity contour panel.
    pub intensity: bool,
    /// Annotation tier panel.
    pub tiers: bool,
}

/// Physical length unit for a figure request.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FigureUnit {
    /// Centimeters.
    Cm,
    /// Inches.
    In,
    /// Typographic points.
    Pt,
}

impl From<FigureUnit> for LengthUnit {
    fn from(unit: FigureUnit) -> Self {
        match unit {
            FigureUnit::Cm => LengthUnit::Cm,
            FigureUnit::In => LengthUnit::In,
            FigureUnit::Pt => LengthUnit::Pt,
        }
    }
}

/// Theme a figure is described against; the spectrogram colorizes to it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FigureTheme {
    /// Light background, the print default.
    Light,
    /// Dark background, for screen.
    Dark,
}

impl From<FigureTheme> for Theme {
    fn from(theme: FigureTheme) -> Self {
        match theme {
            FigureTheme::Light => Theme::Light,
            FigureTheme::Dark => Theme::Dark,
        }
    }
}

/// Spectrogram palette, including the grayscale ramp for print.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FigureColormap {
    /// Perceptually uniform purple→teal→yellow ramp.
    Viridis,
    /// Perceptually uniform black→purple→orange ramp.
    Magma,
    /// Achromatic ramp for grayscale print.
    Grayscale,
}

impl From<FigureColormap> for Colormap {
    fn from(colormap: FigureColormap) -> Self {
        match colormap {
            FigureColormap::Viridis => Colormap::Viridis,
            FigureColormap::Magma => Colormap::Magma,
            FigureColormap::Grayscale => Colormap::Grayscale,
        }
    }
}

/// Pitch axis unit for the pitch panel.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FigurePitchUnit {
    /// Hertz.
    Hertz,
    /// Semitones re 1 Hz.
    Semitones,
}

impl From<FigurePitchUnit> for PitchUnit {
    fn from(unit: FigurePitchUnit) -> Self {
        match unit {
            FigurePitchUnit::Hertz => PitchUnit::Hertz,
            FigurePitchUnit::Semitones => PitchUnit::Semitones,
        }
    }
}

/// Everything needed to assemble a figure from live session state.
///
/// The request names the audio and optional annotation, the time and frequency
/// window, the layers to include, the physical size and palette, and the
/// analysis parameters each track is computed with — the same parameters the
/// inspector edits.
#[derive(Debug, Clone, Deserialize)]
pub struct FigureRequest {
    /// Audio buffer to draw.
    pub audio: u64,
    /// Annotation document whose tiers to draw, when the tier layer is on.
    #[serde(default)]
    pub annotation: Option<u64>,
    /// Window start time in seconds.
    pub t0: f64,
    /// Window end time in seconds.
    pub t1: f64,
    /// Spectrogram frequency floor in hertz.
    pub f0: f64,
    /// Spectrogram frequency ceiling in hertz.
    pub f1: f64,
    /// Which layers to include.
    pub layers: LayerToggles,
    /// Figure width in `unit`.
    pub width: f64,
    /// Figure height in `unit`.
    pub height: f64,
    /// Physical unit of `width` and `height`.
    pub unit: FigureUnit,
    /// Theme the figure renders against.
    pub theme: FigureTheme,
    /// Spectrogram palette.
    pub colormap: FigureColormap,
    /// Spectrogram dynamic range in decibels.
    pub dynamic_range_db: f64,
    /// Spectrogram ceiling in decibels; autoscales when absent.
    #[serde(default)]
    pub max_db: Option<f64>,
    /// Spectrogram tile column count.
    pub spectrogram_width_px: u32,
    /// Spectrogram tile row count.
    pub spectrogram_height_px: u32,
    /// Spectrogram analysis window length in seconds.
    pub window_length: f64,
    /// Pitch floor in hertz.
    pub pitch_floor_hz: f64,
    /// Pitch ceiling in hertz.
    pub pitch_ceiling_hz: f64,
    /// Pitch axis unit.
    pub pitch_unit: FigurePitchUnit,
    /// Formant ceiling in hertz.
    pub formant_ceiling_hz: f64,
    /// Maximum formants tracked.
    pub formant_max: usize,
    /// Whether the formant layer carries Viterbi-smoothed tracks.
    pub formant_smoothed: bool,
    /// Intensity pitch floor in hertz, setting the analysis window length.
    pub intensity_floor_hz: f64,
}

/// A downloadable figure export: one main file plus any sidecar files.
///
/// A backend that emits a single self-contained document (SVG, Vega, GraphML)
/// leaves `sidecars` empty; TikZ, Typst, and the generated scripts add the
/// spectrogram image and per-layer data files the main document references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBundle {
    /// Suggested file name for the main document.
    pub main_name: String,
    /// Main document contents.
    pub main_bytes: Vec<u8>,
    /// MIME type of the main document.
    pub mime: String,
    /// Whether the main document is UTF-8 text.
    pub is_text: bool,
    /// Files the main document references.
    pub sidecars: Vec<phx_figure::SidecarFile>,
}

/// A figure export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FigureFormat {
    /// SVG scene graph; the source of truth and the preview.
    Svg,
    /// Rasterized PNG (native only).
    Png,
    /// PDF derived from the SVG (native only).
    Pdf,
    /// Vega-Lite v5 JSON.
    Vega,
    /// PGFPlots / TikZ source.
    Tikz,
    /// Typst / CeTZ source.
    Typst,
    /// matplotlib script plus data.
    Python,
    /// ggplot2 script plus data.
    R,
    /// Makie script plus data.
    Julia,
    /// GraphML of the annotation tiers.
    Graphml,
}

impl Engine {
    /// Assembles a self-contained [`Figure`] from the current session state.
    ///
    /// Each enabled layer is built from the same analysis the editor overlays
    /// read: the spectrogram tile for the window, the whole-signal pitch,
    /// formant, and intensity tracks (clipped to the window at render time), the
    /// windowed waveform envelope, and the named annotation's tiers. Panels stack
    /// waveform, spectrogram (with the formant speckle over it), pitch,
    /// intensity, then tiers, keeping each panel's value axis honest.
    ///
    /// # Errors
    /// Returns [`EngineError::UnknownAudioId`] when `audio` names no live buffer,
    /// [`EngineError::UnknownAnnotationId`] when the tier layer names no live
    /// document, and [`EngineError::InvalidRequest`] when the window or an
    /// analysis parameter is unusable (a non-finite bound, a window too short for
    /// one spectrogram frame).
    pub fn build_figure(&self, req: &FigureRequest) -> Result<Figure, EngineError> {
        if !(req.t0.is_finite() && req.t1.is_finite()) {
            return Err(EngineError::InvalidRequest {
                reason: "figure time window must be finite".to_string(),
            });
        }
        let audio_id = AudioId::from_u64(req.audio);
        let audio = self.store.audio(audio_id)?;
        let sample_rate = audio.sample_rate();
        let frames = audio.frames();
        let duration = audio.duration();
        let lo = req.t0.min(req.t1).clamp(0.0, duration);
        let hi = req.t0.max(req.t1).clamp(0.0, duration);
        if hi <= lo {
            return Err(EngineError::InvalidRequest {
                reason: "figure time window is empty".to_string(),
            });
        }
        let time_axis = || Axis::linear(lo, hi, Some("Time"), Some("s"));

        let theme: Theme = req.theme.into();
        let mut panels: Vec<Panel> = Vec::new();
        let mut sources: Vec<ProvenanceRecord> = Vec::new();

        // Waveform: windowed min/max envelope so the visible span keeps its
        // resolution regardless of the whole-file length.
        if req.layers.waveform {
            let start = ((lo * sample_rate).floor() as usize).min(frames);
            let end = ((hi * sample_rate).ceil() as usize).clamp(start, frames);
            let mono = audio.slice_samples(start..end).mono_mix();
            let buckets = req.spectrogram_width_px.max(1) as usize;
            let envelope = waveform_minmax(&mono, buckets);
            let span = TimeSpan::new(start as f64 / sample_rate, end as f64 / sample_rate);
            panels.push(Panel {
                layers: vec![waveform_layer(envelope, span, LineStyle::default())],
                time_axis: time_axis(),
                value_axis: Axis::linear(-1.0, 1.0, Some("Amplitude"), None),
                height_share: 0.20,
            });
        }

        // Spectrogram and formant speckle share the frequency panel.
        if req.layers.spectrogram || req.layers.formant {
            let mut layers = Vec::new();
            let spec_params = SpectrogramParams {
                window_length: req.window_length,
                ..SpectrogramParams::default()
            };
            let f_lo = req.f0.max(0.0);
            let f_hi = req.f1.clamp(f_lo + 1.0, spec_params.max_frequency);

            if req.layers.spectrogram {
                let tile_req = TileRequest {
                    t0: lo,
                    t1: hi,
                    f0: f_lo,
                    f1: f_hi,
                    width_px: req.spectrogram_width_px,
                    height_px: req.spectrogram_height_px,
                    params: spec_params,
                };
                crate::validate_tile_request(&tile_req)?;
                let view = audio.slice_samples(0..frames);
                let tile = compute_tile(view, &tile_req);
                let expected =
                    req.spectrogram_width_px as usize * req.spectrogram_height_px as usize;
                if tile.db.len() != expected {
                    return Err(EngineError::InvalidRequest {
                        reason: "audio too short for the requested spectrogram window".to_string(),
                    });
                }
                let display = DisplayMapping {
                    dynamic_range_db: req.dynamic_range_db,
                    max_db: req.max_db,
                };
                layers.push(spectrogram_layer(&tile, display, req.colormap.into()));
                sources.push(spectrogram_provenance(&spec_params));
            }

            if req.layers.formant {
                let params = FormantParams {
                    ceiling_hz: req.formant_ceiling_hz,
                    max_formants: req.formant_max,
                    ..FormantParams::default()
                };
                let track = if req.formant_smoothed {
                    self.formant_track_smoothed(audio_id, &params)?
                } else {
                    self.formant_track(audio_id, &params)?
                };
                layers.push(formant_layer(
                    &track,
                    req.formant_smoothed,
                    SpeckleStyle::default(),
                ));
                sources.push(formant_provenance(&params, req.formant_smoothed));
            }

            panels.push(Panel {
                layers,
                time_axis: time_axis(),
                value_axis: Axis::linear(f_lo, f_hi, Some("Frequency"), Some("Hz")),
                height_share: 0.42,
            });
        }

        if req.layers.pitch {
            let params = PitchParams {
                floor_hz: req.pitch_floor_hz,
                ceiling_hz: req.pitch_ceiling_hz,
                ..PitchParams::default()
            };
            let track = self.pitch_track(audio_id, &params)?;
            let (unit, lo_v, hi_v, label) = match req.pitch_unit {
                FigurePitchUnit::Hertz => (
                    PitchUnit::Hertz,
                    req.pitch_floor_hz,
                    req.pitch_ceiling_hz,
                    "Pitch",
                ),
                FigurePitchUnit::Semitones => (
                    PitchUnit::Semitones,
                    12.0 * req.pitch_floor_hz.max(1.0).log2(),
                    12.0 * req.pitch_ceiling_hz.max(1.0).log2(),
                    "Pitch",
                ),
            };
            let unit_label = match req.pitch_unit {
                FigurePitchUnit::Hertz => "Hz",
                FigurePitchUnit::Semitones => "st",
            };
            panels.push(Panel {
                layers: vec![pitch_layer(&track, unit, LineStyle::default())],
                time_axis: time_axis(),
                value_axis: Axis::linear(lo_v, hi_v, Some(label), Some(unit_label)),
                height_share: 0.20,
            });
            sources.push(pitch_provenance(&params));
        }

        if req.layers.intensity {
            let params = IntensityParams {
                pitch_floor_hz: req.intensity_floor_hz,
                ..IntensityParams::default()
            };
            let track = self.intensity_track(audio_id, &params)?;
            let (mut db_lo, mut db_hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for db in track.values() {
                if db.is_finite() {
                    db_lo = db_lo.min(*db);
                    db_hi = db_hi.max(*db);
                }
            }
            if !(db_lo.is_finite() && db_hi.is_finite() && db_hi > db_lo) {
                db_lo = 0.0;
                db_hi = 100.0;
            } else {
                let pad = (db_hi - db_lo) * 0.05;
                db_lo -= pad;
                db_hi += pad;
            }
            panels.push(Panel {
                layers: vec![intensity_layer(&track, LineStyle::default())],
                time_axis: time_axis(),
                value_axis: Axis::linear(db_lo, db_hi, Some("Intensity"), Some("dB")),
                height_share: 0.16,
            });
            sources.push(intensity_provenance(&params));
        }

        if req.layers.tiers
            && let Some(annotation_id) = req.annotation
        {
            let annotation = self.annotation(AnnotationId::from_u64(annotation_id))?;
            let tiers = annotation.tiers();
            if !tiers.is_empty() {
                panels.push(Panel {
                    layers: vec![tiers_layer(tiers, TierStyle::default())],
                    time_axis: time_axis(),
                    value_axis: Axis::linear(0.0, 1.0, None, None),
                    height_share: 0.14,
                });
            }
        }

        if panels.is_empty() {
            return Err(EngineError::InvalidRequest {
                reason: "a figure needs at least one layer".to_string(),
            });
        }

        let size = SizeSpec::new(req.width, req.height, req.unit.into());
        let mut builder = FigureBuilder::new(size, theme);
        for panel in panels {
            builder = builder.panel(panel);
        }
        for source in sources {
            builder = builder.source(source);
        }
        let figure = builder.build();
        Ok(figure)
    }
}

/// Renders a figure to its SVG scene graph — the preview and the SVG export at
/// once, so the two are byte-identical by construction.
#[must_use]
pub fn figure_to_svg(figure: &Figure) -> String {
    to_svg(figure)
}

/// Exports a figure to a downloadable [`ExportBundle`] in `format`.
///
/// SVG, Vega, TikZ, Typst, the generated scripts, and GraphML are available on
/// every target including wasm. PNG and PDF sit behind the `figure-raster` and
/// `figure-pdf` features, which the wasm build leaves off; requesting them there
/// returns [`EngineError::InvalidRequest`] rather than a broken export.
///
/// # Errors
/// Returns [`EngineError::InvalidRequest`] when a native-only format is
/// requested on a build without the corresponding feature, or when the raster
/// or PDF backend rejects the figure.
pub fn export_figure(figure: &Figure, format: FigureFormat) -> Result<ExportBundle, EngineError> {
    match format {
        FigureFormat::Svg => Ok(text_bundle(
            "figure.svg",
            to_svg(figure),
            "image/svg+xml",
            Vec::new(),
        )),
        FigureFormat::Vega => Ok(text_bundle(
            "figure.vega.json",
            to_vega(figure),
            "application/json",
            Vec::new(),
        )),
        FigureFormat::Tikz => Ok(from_text_export(to_tikz(figure), "application/x-tex")),
        FigureFormat::Typst => Ok(from_text_export(to_typst(figure), "text/plain")),
        FigureFormat::Python => Ok(from_code_export(
            to_code(figure, CodeLang::Python),
            "text/x-python",
        )),
        FigureFormat::R => Ok(from_code_export(to_code(figure, CodeLang::R), "text/x-r")),
        FigureFormat::Julia => Ok(from_code_export(
            to_code(figure, CodeLang::Julia),
            "text/x-julia",
        )),
        FigureFormat::Graphml => Ok(text_bundle(
            "figure.graphml",
            to_graphml(&phx_figure::figure_tiers(figure)),
            "application/graphml+xml",
            Vec::new(),
        )),
        FigureFormat::Png => export_png(figure),
        FigureFormat::Pdf => export_pdf(figure),
    }
}

fn text_bundle(
    name: &str,
    contents: String,
    mime: &str,
    sidecars: Vec<phx_figure::SidecarFile>,
) -> ExportBundle {
    ExportBundle {
        main_name: name.to_string(),
        main_bytes: contents.into_bytes(),
        mime: mime.to_string(),
        is_text: true,
        sidecars,
    }
}

fn from_text_export(export: TextExport, mime: &str) -> ExportBundle {
    ExportBundle {
        main_name: export.main_name,
        main_bytes: export.main.into_bytes(),
        mime: mime.to_string(),
        is_text: true,
        sidecars: export.sidecars,
    }
}

fn from_code_export(export: CodeExport, mime: &str) -> ExportBundle {
    ExportBundle {
        main_name: export.script_name,
        main_bytes: export.script.into_bytes(),
        mime: mime.to_string(),
        is_text: true,
        sidecars: export.data_files,
    }
}

#[cfg(feature = "figure-raster")]
fn export_png(figure: &Figure) -> Result<ExportBundle, EngineError> {
    let bytes = phx_figure::to_png(figure, 192.0).map_err(|err| EngineError::InvalidRequest {
        reason: format!("PNG export failed: {err}"),
    })?;
    Ok(ExportBundle {
        main_name: "figure.png".to_string(),
        main_bytes: bytes,
        mime: "image/png".to_string(),
        is_text: false,
        sidecars: Vec::new(),
    })
}

#[cfg(not(feature = "figure-raster"))]
fn export_png(_figure: &Figure) -> Result<ExportBundle, EngineError> {
    Err(EngineError::InvalidRequest {
        reason: "PNG export is native-only; rasterize the SVG preview instead".to_string(),
    })
}

#[cfg(feature = "figure-pdf")]
fn export_pdf(figure: &Figure) -> Result<ExportBundle, EngineError> {
    let bytes = phx_figure::to_pdf(figure).map_err(|err| EngineError::InvalidRequest {
        reason: format!("PDF export failed: {err}"),
    })?;
    Ok(ExportBundle {
        main_name: "figure.pdf".to_string(),
        main_bytes: bytes,
        mime: "application/pdf".to_string(),
        is_text: false,
        sidecars: Vec::new(),
    })
}

#[cfg(not(feature = "figure-pdf"))]
fn export_pdf(_figure: &Figure) -> Result<ExportBundle, EngineError> {
    Err(EngineError::InvalidRequest {
        reason: "PDF export is native-only".to_string(),
    })
}

fn provenance(
    layer: LayerKind,
    params: &[(&str, String)],
    smoothed: Option<bool>,
) -> ProvenanceRecord {
    ProvenanceRecord {
        layer,
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        smoothed,
    }
}

fn spectrogram_provenance(params: &SpectrogramParams) -> ProvenanceRecord {
    provenance(
        LayerKind::Spectrogram,
        &[
            ("window_length_s", format!("{}", params.window_length)),
            ("max_frequency_hz", format!("{}", params.max_frequency)),
            ("time_step_s", format!("{}", params.time_step)),
            ("frequency_step_hz", format!("{}", params.frequency_step)),
        ],
        None,
    )
}

fn pitch_provenance(params: &PitchParams) -> ProvenanceRecord {
    provenance(
        LayerKind::Pitch,
        &[
            ("floor_hz", format!("{}", params.floor_hz)),
            ("ceiling_hz", format!("{}", params.ceiling_hz)),
        ],
        None,
    )
}

fn formant_provenance(params: &FormantParams, smoothed: bool) -> ProvenanceRecord {
    provenance(
        LayerKind::Formant,
        &[
            ("ceiling_hz", format!("{}", params.ceiling_hz)),
            ("max_formants", format!("{}", params.max_formants)),
        ],
        Some(smoothed),
    )
}

fn intensity_provenance(params: &IntensityParams) -> ProvenanceRecord {
    provenance(
        LayerKind::Intensity,
        &[("pitch_floor_hz", format!("{}", params.pitch_floor_hz))],
        None,
    )
}

/// Default figure request over `[t0, t1]` of `audio`, all layers on, matching
/// the inspector's default analysis parameters and a 16×12 cm dark figure.
///
/// The editor builds its own request from live inspector state; this keeps the
/// tests and any caller wanting a sensible starting figure in one place.
#[must_use]
pub fn default_figure_request(
    audio: u64,
    annotation: Option<u64>,
    t0: f64,
    t1: f64,
) -> FigureRequest {
    FigureRequest {
        audio,
        annotation,
        t0,
        t1,
        f0: 0.0,
        f1: 5000.0,
        layers: LayerToggles {
            waveform: true,
            spectrogram: true,
            pitch: true,
            formant: true,
            intensity: true,
            tiers: annotation.is_some(),
        },
        width: 16.0,
        height: 12.0,
        unit: FigureUnit::Cm,
        theme: FigureTheme::Dark,
        colormap: FigureColormap::Viridis,
        dynamic_range_db: 50.0,
        max_db: None,
        spectrogram_width_px: 800,
        spectrogram_height_px: 256,
        window_length: SpectrogramParams::default().window_length,
        pitch_floor_hz: 75.0,
        pitch_ceiling_hz: 600.0,
        pitch_unit: FigurePitchUnit::Hertz,
        formant_ceiling_hz: 5500.0,
        formant_max: 5,
        formant_smoothed: false,
        intensity_floor_hz: 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;
    use phx_annot::TierRelation;
    use phx_figure::{Layer, LayerKind};

    const FIXTURE_WAV: &[u8] = include_bytes!("../../../tests/fixtures/audio/arctic_bdl_a0001.wav");

    fn engine_with_audio() -> (Engine, u64, f64) {
        let mut engine = Engine::new();
        let id = engine.import_wav_bytes(FIXTURE_WAV).unwrap();
        let duration = engine.audio_info(id).unwrap().duration;
        (engine, id.as_u64(), duration)
    }

    fn layer_kinds(figure: &Figure) -> Vec<LayerKind> {
        figure
            .panels
            .iter()
            .flat_map(|panel| panel.layers.iter().map(Layer::kind))
            .collect()
    }

    #[test]
    fn build_figure_assembles_requested_layers() {
        let (engine, audio, duration) = engine_with_audio();
        let req = default_figure_request(audio, None, 0.0, duration);
        let figure = engine.build_figure(&req).unwrap();
        figure.validate().unwrap();
        let kinds = layer_kinds(&figure);
        assert!(kinds.contains(&LayerKind::Waveform));
        assert!(kinds.contains(&LayerKind::Spectrogram));
        assert!(kinds.contains(&LayerKind::Formant));
        assert!(kinds.contains(&LayerKind::Pitch));
        assert!(kinds.contains(&LayerKind::Intensity));
        // No annotation was named, so the tier layer is absent.
        assert!(!kinds.contains(&LayerKind::Tiers));
        // Provenance records the four re-runnable analyses.
        assert_eq!(figure.caption_meta.sources.len(), 4);
    }

    #[test]
    fn layer_toggles_select_panels() {
        let (engine, audio, duration) = engine_with_audio();
        let mut req = default_figure_request(audio, None, 0.0, duration);
        req.layers = LayerToggles {
            waveform: true,
            spectrogram: false,
            pitch: false,
            formant: false,
            intensity: false,
            tiers: false,
        };
        let figure = engine.build_figure(&req).unwrap();
        assert_eq!(layer_kinds(&figure), vec![LayerKind::Waveform]);
    }

    #[test]
    fn tier_layer_needs_a_live_annotation() {
        let (mut engine, audio, duration) = engine_with_audio();
        let annotation = match engine
            .apply(Command::AttachAnnotation {
                audio: AudioId::from_u64(audio),
                annotation: phx_annot::Annotation::new(0.0, duration).unwrap(),
            })
            .unwrap()
        {
            crate::Applied::AnnotationAttached { annotation, .. } => annotation.as_u64(),
            other => panic!("expected attach, got {other:?}"),
        };
        engine
            .apply(Command::AddIntervalTier {
                annotation: AnnotationId::from_u64(annotation),
                name: "words".to_string(),
                relation: TierRelation::Independent,
            })
            .unwrap();
        let mut req = default_figure_request(audio, Some(annotation), 0.0, duration);
        req.layers = LayerToggles {
            waveform: false,
            spectrogram: false,
            pitch: false,
            formant: false,
            intensity: false,
            tiers: true,
        };
        let figure = engine.build_figure(&req).unwrap();
        assert_eq!(layer_kinds(&figure), vec![LayerKind::Tiers]);
    }

    #[test]
    fn preview_svg_equals_svg_export_byte_for_byte() {
        let (engine, audio, duration) = engine_with_audio();
        let req = default_figure_request(audio, None, 0.0, duration);
        let figure = engine.build_figure(&req).unwrap();
        let preview = figure_to_svg(&figure);
        let export = export_figure(&figure, FigureFormat::Svg).unwrap();
        assert_eq!(preview.into_bytes(), export.main_bytes);
        assert!(export.is_text);
        assert_eq!(export.main_name, "figure.svg");
    }

    #[test]
    fn text_exports_carry_their_backends() {
        let (engine, audio, duration) = engine_with_audio();
        let req = default_figure_request(audio, None, 0.0, duration);
        let figure = engine.build_figure(&req).unwrap();
        for (format, ext) in [
            (FigureFormat::Vega, "figure.vega.json"),
            (FigureFormat::Tikz, "figure.tex"),
            (FigureFormat::Typst, "figure.typ"),
            (FigureFormat::Python, "figure.py"),
        ] {
            let bundle = export_figure(&figure, format).unwrap();
            assert!(!bundle.main_bytes.is_empty());
            if format == FigureFormat::Vega {
                assert_eq!(bundle.main_name, ext);
            }
        }
    }

    #[test]
    fn native_only_formats_reject_without_their_feature() {
        let (engine, audio, duration) = engine_with_audio();
        let req = default_figure_request(audio, None, 0.0, duration);
        let figure = engine.build_figure(&req).unwrap();
        #[cfg(not(feature = "figure-raster"))]
        assert!(matches!(
            export_figure(&figure, FigureFormat::Png),
            Err(EngineError::InvalidRequest { .. })
        ));
        #[cfg(not(feature = "figure-pdf"))]
        assert!(matches!(
            export_figure(&figure, FigureFormat::Pdf),
            Err(EngineError::InvalidRequest { .. })
        ));
        #[cfg(feature = "figure-raster")]
        assert!(
            export_figure(&figure, FigureFormat::Png)
                .unwrap()
                .main_bytes
                .len()
                > 8
        );
        #[cfg(feature = "figure-pdf")]
        assert!(
            !export_figure(&figure, FigureFormat::Pdf)
                .unwrap()
                .main_bytes
                .is_empty()
        );
    }
}
