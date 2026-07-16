//! The engine command surface: one `#[tauri::command]` per worker-protocol
//! method, each locking the shared [`Engine`] for a single call and returning
//! the same shape the WASM worker returns to the shared UI.

use phx_engine::{
    AlignMode, AnnotationId, Applied, AudioId, BoundaryId, Colormap, Command, DisplayMapping,
    Engine, EngineError, Figure, FigureFormat, FigureRequest, FormantParams, IntensityParams,
    IntervalId, LabelPattern, LabelQuery, LabelTarget, PitchParams, PointId, SpectrogramParams,
    Theme, Tier, TierId, TierRelation, TileRequest, export_figure as engine_export_figure,
    figure_to_svg,
};
use serde::Deserialize;
use tauri::State;
use tauri::ipc::Response;

use crate::state::{
    AppState, AppliedDto, ExportBundleDto, FormantTrackDto, IntensityTrackDto, IntervalDto,
    LabelHitDto, PitchTrackDto, PointDto, SidecarDto, TierInfoDto,
};

/// Maps an [`EngineError`] to the string the IPC layer rejects with.
fn err(e: EngineError) -> String {
    e.to_string()
}

/// Locks the shared engine, mapping poisoning to an error string.
fn lock<'a>(state: &'a State<'_, AppState>) -> Result<std::sync::MutexGuard<'a, Engine>, String> {
    state
        .engine
        .lock()
        .map_err(|_| "engine lock poisoned".to_string())
}

/// A spectrogram tile request over the IPC boundary, matching the TS interface.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileReq {
    t0: f64,
    t1: f64,
    f0: f64,
    f1: f64,
    width_px: u32,
    height_px: u32,
    window_length: f64,
    max_frequency: f64,
    time_step: f64,
    frequency_step: f64,
    dynamic_range_db: f64,
    max_db: Option<f64>,
    colormap: String,
    theme: String,
}

fn colormap_of(name: &str) -> Colormap {
    match name {
        "Magma" => Colormap::Magma,
        "Grayscale" => Colormap::Grayscale,
        _ => Colormap::Viridis,
    }
}

fn theme_of(name: &str) -> Theme {
    match name {
        "Dark" => Theme::Dark,
        _ => Theme::Light,
    }
}

#[tauri::command]
pub fn waveform_slice(
    state: State<AppState>,
    id: u64,
    t0: f64,
    t1: f64,
    px: u32,
) -> Result<Response, String> {
    let engine = lock(&state)?;
    let slice = engine
        .waveform_slice(AudioId::from_u64(id), t0, t1, px)
        .map_err(err)?;
    let mut bytes = Vec::with_capacity(slice.len() * 8);
    for bucket in slice {
        bytes.extend_from_slice(&bucket.min.to_le_bytes());
        bytes.extend_from_slice(&bucket.max.to_le_bytes());
    }
    Ok(Response::new(bytes))
}

#[tauri::command]
pub fn spectrogram_tile(state: State<AppState>, id: u64, req: TileReq) -> Result<Response, String> {
    let tile = TileRequest {
        t0: req.t0,
        t1: req.t1,
        f0: req.f0,
        f1: req.f1,
        width_px: req.width_px,
        height_px: req.height_px,
        params: SpectrogramParams {
            window_length: req.window_length,
            max_frequency: req.max_frequency,
            time_step: req.time_step,
            frequency_step: req.frequency_step,
            ..SpectrogramParams::default()
        },
    };
    let display = DisplayMapping {
        dynamic_range_db: req.dynamic_range_db,
        max_db: req.max_db,
    };
    let engine = lock(&state)?;
    let rgba = engine
        .spectrogram_tile_rgba(
            AudioId::from_u64(id),
            &tile,
            &display,
            colormap_of(&req.colormap),
            theme_of(&req.theme),
        )
        .map_err(err)?;
    Ok(Response::new(rgba))
}

#[tauri::command]
pub fn pitch_track(
    state: State<AppState>,
    id: u64,
    floor_hz: f64,
    ceiling_hz: f64,
) -> Result<PitchTrackDto, String> {
    let params = PitchParams {
        floor_hz,
        ceiling_hz,
        ..PitchParams::default()
    };
    let engine = lock(&state)?;
    let track = engine
        .pitch_track(AudioId::from_u64(id), &params)
        .map_err(err)?;
    Ok(pitch_dto(track.frames(), 0.0))
}

#[tauri::command]
pub fn pitch_track_span(
    state: State<AppState>,
    id: u64,
    floor_hz: f64,
    ceiling_hz: f64,
    t0: f64,
    t1: f64,
) -> Result<PitchTrackDto, String> {
    let params = PitchParams {
        floor_hz,
        ceiling_hz,
        ..PitchParams::default()
    };
    let engine = lock(&state)?;
    let (track, start) = engine
        .pitch_track_span(AudioId::from_u64(id), &params, t0, t1)
        .map_err(err)?;
    Ok(pitch_dto(track.frames(), start))
}

fn pitch_dto(frames: &[phx_engine::PitchFrame], offset: f64) -> PitchTrackDto {
    let mut times = Vec::with_capacity(frames.len());
    let mut f0 = Vec::with_capacity(frames.len());
    let mut max_hz = 0.0_f64;
    for frame in frames {
        times.push(offset + frame.time);
        match frame.f0 {
            Some(hz) => {
                max_hz = max_hz.max(hz);
                f0.push(hz);
            }
            None => f0.push(f64::NAN),
        }
    }
    PitchTrackDto { times, f0, max_hz }
}

#[tauri::command]
pub fn formant_track(
    state: State<AppState>,
    id: u64,
    ceiling_hz: f64,
    max_formants: usize,
    smoothed: bool,
) -> Result<FormantTrackDto, String> {
    let params = FormantParams {
        ceiling_hz,
        max_formants,
        ..FormantParams::default()
    };
    let audio = AudioId::from_u64(id);
    let engine = lock(&state)?;
    let track = if smoothed {
        engine.formant_track_smoothed(audio, &params).map_err(err)?
    } else {
        engine.formant_track(audio, &params).map_err(err)?
    };
    let mut points = Vec::new();
    let mut max_hz = 0.0_f64;
    for frame in &track.frames {
        for formant in &frame.formants {
            points.push(frame.time);
            points.push(formant.frequency);
            points.push(formant.bandwidth);
            max_hz = max_hz.max(formant.frequency);
        }
    }
    Ok(FormantTrackDto { points, max_hz })
}

#[tauri::command]
pub fn intensity_track(
    state: State<AppState>,
    id: u64,
    floor_hz: f64,
) -> Result<IntensityTrackDto, String> {
    let params = IntensityParams {
        pitch_floor_hz: floor_hz,
        ..IntensityParams::default()
    };
    let engine = lock(&state)?;
    let track = engine
        .intensity_track(AudioId::from_u64(id), &params)
        .map_err(err)?;
    let (times, db) = track.iter().unzip();
    Ok(IntensityTrackDto { times, db })
}

#[tauri::command]
pub fn band_energy(
    state: State<AppState>,
    id: u64,
    t0: f64,
    t1: f64,
    f0: f64,
    f1: f64,
) -> Result<f64, String> {
    let engine = lock(&state)?;
    engine
        .band_energy(AudioId::from_u64(id), t0, t1, f0, f1)
        .map_err(err)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn selection_readout(
    state: State<AppState>,
    id: u64,
    t0: f64,
    t1: f64,
    f0: f64,
    f1: f64,
    pitch_floor_hz: f64,
    pitch_ceiling_hz: f64,
    intensity_floor_hz: f64,
) -> Result<serde_json::Value, String> {
    let engine = lock(&state)?;
    let r = engine
        .selection_readout(
            AudioId::from_u64(id),
            t0,
            t1,
            f0,
            f1,
            pitch_floor_hz,
            pitch_ceiling_hz,
            intensity_floor_hz,
        )
        .map_err(err)?;
    Ok(serde_json::json!({
        "t0": r.t0,
        "t1": r.t1,
        "f0": r.f0,
        "f1": r.f1,
        "duration": r.duration,
        "f0MeanHz": r.f0_mean_hz,
        "f0MinHz": r.f0_min_hz,
        "f0MaxHz": r.f0_max_hz,
        "bandEnergyDb": r.band_energy_db,
        "intensityMeanDb": r.intensity_mean_db,
        "hnrMeanDb": r.hnr_mean_db,
    }))
}

#[tauri::command]
pub fn formant_span_means(
    state: State<AppState>,
    id: u64,
    ceiling_hz: f64,
    max_formants: usize,
    smoothed: bool,
    t0: f64,
    t1: f64,
) -> Result<Vec<f64>, String> {
    let params = FormantParams {
        ceiling_hz,
        max_formants,
        ..FormantParams::default()
    };
    let engine = lock(&state)?;
    let means = engine
        .formant_span_means(AudioId::from_u64(id), &params, smoothed, t0, t1)
        .map_err(err)?;
    Ok(means.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect())
}

#[tauri::command]
pub fn voice_report(
    state: State<AppState>,
    id: u64,
    t0: f64,
    t1: f64,
    pitch_floor_hz: f64,
    pitch_ceiling_hz: f64,
) -> Result<serde_json::Value, String> {
    let audio = AudioId::from_u64(id);
    let engine = lock(&state)?;
    let report = engine
        .voice_report(audio, t0, t1, pitch_floor_hz, pitch_ceiling_hz)
        .map_err(err)?;
    let moments = engine
        .spectral_moments_in_span(audio, t0, t1, 2.0)
        .map_err(err)?;
    Ok(serde_json::json!({
        "t0": report.span.start,
        "t1": report.span.end,
        "pitch": {
            "meanHz": report.pitch.mean_hz,
            "medianHz": report.pitch.median_hz,
            "minHz": report.pitch.min_hz,
            "maxHz": report.pitch.max_hz,
        },
        "jitter": {
            "local": report.jitter.local,
            "localAbsolute": report.jitter.local_absolute,
            "rap": report.jitter.rap,
            "ppq5": report.jitter.ppq5,
            "ddp": report.jitter.ddp,
        },
        "shimmer": {
            "local": report.shimmer.local,
            "localDb": report.shimmer.local_db,
            "apq3": report.shimmer.apq3,
            "apq5": report.shimmer.apq5,
            "apq11": report.shimmer.apq11,
            "dda": report.shimmer.dda,
        },
        "meanHnrDb": report.mean_hnr_db,
        "cppDb": report.cpp_db,
        "cppsDb": report.cpps_db,
        "voiceBreaks": {
            "thresholdSeconds": report.voice_breaks.threshold_seconds,
            "totalSeconds": report.voice_breaks.total_seconds,
            "count": report.voice_breaks.gaps.len(),
        },
        "moments": {
            "centreOfGravityHz": moments.centre_of_gravity_hz,
            "standardDeviationHz": moments.standard_deviation_hz,
            "skewness": moments.skewness,
            "kurtosis": moments.kurtosis,
        },
        "pulseCount": report.pulses.times().len(),
        "params": {
            "pitchFloorHz": report.pitch_params.floor_hz,
            "pitchCeilingHz": report.pitch_params.ceiling_hz,
            "harmonicityFloorHz": report.harmonicity_params.floor_hz,
            "periodsPerWindow": report.harmonicity_params.periods_per_window,
            "cppFrameLengthSeconds": report.cpp_params.frame_length_seconds,
            "cppMinF0Hz": report.cpp_params.min_f0_hz,
            "cppMaxF0Hz": report.cpp_params.max_f0_hz,
        },
    }))
}

// --- Journaled annotation surface -----------------------------------------

fn apply(engine: &mut Engine, cmd: Command) -> Result<Applied, String> {
    engine.apply(cmd).map_err(err)
}

#[tauri::command]
pub fn create_annotation(
    state: State<AppState>,
    audio_id: u64,
    xmin: f64,
    xmax: f64,
) -> Result<u64, String> {
    let annotation = phx_engine::Annotation::new(xmin, xmax).map_err(|e| e.to_string())?;
    let mut engine = lock(&state)?;
    match apply(
        &mut engine,
        Command::AttachAnnotation {
            audio: AudioId::from_u64(audio_id),
            annotation,
        },
    )? {
        Applied::AnnotationAttached { annotation, .. } => Ok(annotation.as_u64()),
        _ => Err("attach did not report an annotation id".into()),
    }
}

#[tauri::command]
pub fn add_interval_tier(
    state: State<AppState>,
    annotation_id: u64,
    name: String,
) -> Result<u64, String> {
    let mut engine = lock(&state)?;
    match apply(
        &mut engine,
        Command::AddIntervalTier {
            annotation: AnnotationId::from_u64(annotation_id),
            name,
            relation: TierRelation::Independent,
        },
    )? {
        Applied::TierAdded { tier, .. } => Ok(tier.get()),
        _ => Err("add tier did not report a tier id".into()),
    }
}

#[tauri::command]
pub fn add_point_tier(
    state: State<AppState>,
    annotation_id: u64,
    name: String,
) -> Result<u64, String> {
    let mut engine = lock(&state)?;
    match apply(
        &mut engine,
        Command::AddPointTier {
            annotation: AnnotationId::from_u64(annotation_id),
            name,
            points: Vec::new(),
            relation: TierRelation::Independent,
        },
    )? {
        Applied::TierAdded { tier, .. } => Ok(tier.get()),
        _ => Err("add tier did not report a tier id".into()),
    }
}

#[tauri::command]
pub fn remove_tier(
    state: State<AppState>,
    annotation_id: u64,
    tier_id: u64,
) -> Result<AppliedDto, String> {
    let mut engine = lock(&state)?;
    Ok(apply(
        &mut engine,
        Command::RemoveTier {
            annotation: AnnotationId::from_u64(annotation_id),
            tier: TierId::new(tier_id),
        },
    )?
    .into())
}

#[tauri::command]
pub fn insert_boundary(
    state: State<AppState>,
    annotation_id: u64,
    tier_id: u64,
    at: f64,
) -> Result<u64, String> {
    let mut engine = lock(&state)?;
    match apply(
        &mut engine,
        Command::InsertBoundary {
            annotation: AnnotationId::from_u64(annotation_id),
            tier: TierId::new(tier_id),
            at,
        },
    )? {
        Applied::BoundaryInserted { boundary, .. } => Ok(boundary.get()),
        _ => Err("insert did not report a boundary id".into()),
    }
}

#[tauri::command]
pub fn move_boundary(
    state: State<AppState>,
    annotation_id: u64,
    boundary_id: u64,
    to: f64,
    linked: bool,
) -> Result<AppliedDto, String> {
    let mode = if linked {
        AlignMode::Linked
    } else {
        AlignMode::SingleTier
    };
    let mut engine = lock(&state)?;
    Ok(apply(
        &mut engine,
        Command::MoveBoundary {
            annotation: AnnotationId::from_u64(annotation_id),
            boundary: BoundaryId::new(boundary_id),
            to,
            mode,
        },
    )?
    .into())
}

#[tauri::command]
pub fn remove_boundary(
    state: State<AppState>,
    annotation_id: u64,
    boundary_id: u64,
) -> Result<AppliedDto, String> {
    let mut engine = lock(&state)?;
    Ok(apply(
        &mut engine,
        Command::RemoveBoundary {
            annotation: AnnotationId::from_u64(annotation_id),
            boundary: BoundaryId::new(boundary_id),
        },
    )?
    .into())
}

#[tauri::command]
pub fn set_interval_label(
    state: State<AppState>,
    annotation_id: u64,
    tier_id: u64,
    interval_id: u64,
    text: String,
) -> Result<AppliedDto, String> {
    let mut engine = lock(&state)?;
    Ok(apply(
        &mut engine,
        Command::SetLabel {
            annotation: AnnotationId::from_u64(annotation_id),
            target: LabelTarget::Interval {
                tier: TierId::new(tier_id),
                interval: IntervalId::new(interval_id),
            },
            text,
        },
    )?
    .into())
}

#[tauri::command]
pub fn set_point_label(
    state: State<AppState>,
    annotation_id: u64,
    tier_id: u64,
    point_id: u64,
    text: String,
) -> Result<AppliedDto, String> {
    let mut engine = lock(&state)?;
    Ok(apply(
        &mut engine,
        Command::SetLabel {
            annotation: AnnotationId::from_u64(annotation_id),
            target: LabelTarget::Point {
                tier: TierId::new(tier_id),
                point: PointId::new(point_id),
            },
            text,
        },
    )?
    .into())
}

#[tauri::command]
pub fn undo(state: State<AppState>) -> Result<Option<AppliedDto>, String> {
    let mut engine = lock(&state)?;
    Ok(engine.undo().map_err(err)?.map(AppliedDto::from))
}

#[tauri::command]
pub fn redo(state: State<AppState>) -> Result<Option<AppliedDto>, String> {
    let mut engine = lock(&state)?;
    Ok(engine.redo().map_err(err)?.map(AppliedDto::from))
}

#[tauri::command]
pub fn undo_depth(state: State<AppState>) -> Result<u32, String> {
    Ok(lock(&state)?.undo_depth() as u32)
}

#[tauri::command]
pub fn redo_depth(state: State<AppState>) -> Result<u32, String> {
    Ok(lock(&state)?.redo_depth() as u32)
}

/// The full document-model hash as a decimal string: a `u64` exceeds the range
/// JSON numbers hold exactly, so the client widens the string back to `bigint`.
#[tauri::command]
pub fn state_hash(state: State<AppState>) -> Result<String, String> {
    Ok(lock(&state)?.state_hash().to_string())
}

#[tauri::command]
pub fn annotation_tiers(
    state: State<AppState>,
    annotation_id: u64,
) -> Result<Vec<TierInfoDto>, String> {
    let engine = lock(&state)?;
    let annotation = engine
        .annotation(AnnotationId::from_u64(annotation_id))
        .map_err(err)?;
    let mut out = Vec::new();
    for slot in annotation.tiers() {
        let (name, kind) = match &slot.tier {
            Tier::Interval(t) => (t.name.clone(), "interval"),
            Tier::Point(t) => (t.name.clone(), "point"),
        };
        out.push(TierInfoDto {
            id: slot.id.get(),
            name,
            kind,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn intervals_in_range(
    state: State<AppState>,
    annotation_id: u64,
    tier_id: u64,
    t0: f64,
    t1: f64,
) -> Result<Vec<IntervalDto>, String> {
    let engine = lock(&state)?;
    let annotation = engine
        .annotation(AnnotationId::from_u64(annotation_id))
        .map_err(err)?;
    let slot = annotation
        .tier(TierId::new(tier_id))
        .ok_or_else(|| "unknown tier id".to_string())?;
    let Tier::Interval(tier) = &slot.tier else {
        return Err("tier is not an interval tier".into());
    };
    let (lo, hi) = (t0.min(t1), t0.max(t1));
    let mut out = Vec::new();
    for interval in &tier.intervals {
        if interval.xmax > lo && interval.xmin < hi {
            out.push(IntervalDto {
                id: interval.id.get(),
                start_boundary: interval.start_boundary.get(),
                end_boundary: interval.end_boundary.get(),
                xmin: interval.xmin,
                xmax: interval.xmax,
                label: interval.label.clone(),
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn points_in_range(
    state: State<AppState>,
    annotation_id: u64,
    tier_id: u64,
    t0: f64,
    t1: f64,
) -> Result<Vec<PointDto>, String> {
    let engine = lock(&state)?;
    let annotation = engine
        .annotation(AnnotationId::from_u64(annotation_id))
        .map_err(err)?;
    let slot = annotation
        .tier(TierId::new(tier_id))
        .ok_or_else(|| "unknown tier id".to_string())?;
    let Tier::Point(tier) = &slot.tier else {
        return Err("tier is not a point tier".into());
    };
    let (lo, hi) = (t0.min(t1), t0.max(t1));
    let mut out = Vec::new();
    for point in &tier.points {
        if point.time >= lo && point.time < hi {
            out.push(PointDto {
                id: point.id.get(),
                time: point.time,
                label: point.label.clone(),
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn search_labels(
    state: State<AppState>,
    pattern: String,
    regex: bool,
) -> Result<Vec<LabelHitDto>, String> {
    let query = LabelQuery {
        pattern: if regex {
            LabelPattern::Regex(pattern)
        } else {
            LabelPattern::Substring(pattern)
        },
        tiers: None,
    };
    let engine = lock(&state)?;
    let mut out = Vec::new();
    for hit in engine.search_labels(&query) {
        let (kind, target) = match hit.hit.target {
            LabelTarget::Interval { interval, .. } => ("interval", interval.get()),
            LabelTarget::Point { point, .. } => ("point", point.get()),
        };
        out.push(LabelHitDto {
            annotation: hit.annotation.as_u64(),
            tier: hit.hit.tier.get(),
            kind,
            target,
            start: hit.hit.span.start as u32,
            end: hit.hit.span.end as u32,
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn import_text_grid(
    state: State<AppState>,
    audio_id: u64,
    bytes: Vec<u8>,
) -> Result<u64, String> {
    let (annotation, _source) = phx_textgrid::read(&bytes).map_err(|e| e.to_string())?;
    let mut engine = lock(&state)?;
    match apply(
        &mut engine,
        Command::AttachAnnotation {
            audio: AudioId::from_u64(audio_id),
            annotation,
        },
    )? {
        Applied::AnnotationAttached { annotation, .. } => Ok(annotation.as_u64()),
        _ => Err("attach did not report an annotation id".into()),
    }
}

#[tauri::command]
pub fn export_text_grid(state: State<AppState>, annotation_id: u64) -> Result<Vec<u8>, String> {
    let engine = lock(&state)?;
    let annotation = engine
        .annotation(AnnotationId::from_u64(annotation_id))
        .map_err(err)?;
    Ok(phx_textgrid::write(annotation))
}

#[tauri::command]
pub fn annotation_json(state: State<AppState>, annotation_id: u64) -> Result<String, String> {
    let engine = lock(&state)?;
    let annotation = engine
        .annotation(AnnotationId::from_u64(annotation_id))
        .map_err(err)?;
    serde_json::to_string(annotation).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn attach_annotation_json(
    state: State<AppState>,
    audio_id: u64,
    json: String,
) -> Result<u64, String> {
    let annotation: phx_engine::Annotation =
        serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let mut engine = lock(&state)?;
    match apply(
        &mut engine,
        Command::AttachAnnotation {
            audio: AudioId::from_u64(audio_id),
            annotation,
        },
    )? {
        Applied::AnnotationAttached { annotation, .. } => Ok(annotation.as_u64()),
        _ => Err("attach did not report an annotation id".into()),
    }
}

// --- Figures ---------------------------------------------------------------

#[tauri::command]
pub fn build_figure(state: State<AppState>, spec_json: String) -> Result<String, String> {
    let request: FigureRequest =
        serde_json::from_str(&spec_json).map_err(|e| format!("invalid figure request: {e}"))?;
    let engine = lock(&state)?;
    let figure = engine.build_figure(&request).map_err(err)?;
    figure.to_json().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn render_figure_svg(figure_json: String) -> Result<String, String> {
    let figure = Figure::from_json(&figure_json).map_err(|e| e.to_string())?;
    Ok(figure_to_svg(&figure))
}

#[tauri::command]
pub fn export_figure(figure_json: String, format: String) -> Result<ExportBundleDto, String> {
    let figure = Figure::from_json(&figure_json).map_err(|e| e.to_string())?;
    let format = parse_format(&format)?;
    let bundle = engine_export_figure(&figure, format).map_err(err)?;
    Ok(ExportBundleDto {
        main_name: bundle.main_name,
        main_bytes: bundle.main_bytes,
        mime: bundle.mime,
        is_text: bundle.is_text,
        sidecars: bundle
            .sidecars
            .into_iter()
            .map(|s| SidecarDto {
                name: s.name,
                bytes: s.bytes,
            })
            .collect(),
    })
}

fn parse_format(name: &str) -> Result<FigureFormat, String> {
    Ok(match name {
        "svg" => FigureFormat::Svg,
        "png" => FigureFormat::Png,
        "pdf" => FigureFormat::Pdf,
        "vega" => FigureFormat::Vega,
        "tikz" => FigureFormat::Tikz,
        "typst" => FigureFormat::Typst,
        "python" => FigureFormat::Python,
        "r" => FigureFormat::R,
        "julia" => FigureFormat::Julia,
        "graphml" => FigureFormat::Graphml,
        other => return Err(format!("unknown figure format: {other}")),
    })
}
