//! Backend-agnostic figure model: the single input every export backend
//! consumes.
//!
//! A [`Figure`] is a self-contained description of a phonetics figure with all
//! data embedded — waveform envelopes, raw-decibel spectrogram matrices, pitch
//! and intensity point series, formant speckles, and annotation tiers. Nothing
//! in the model references an engine, an audio store, or a live analysis, so a
//! figure serializes to JSON deterministically and travels as the
//! dialog↔worker wire format. Spectrograms store raw decibels and defer
//! colorization to export time, so each backend re-colorizes per theme rather
//! than replaying baked pixels.
//!
//! [`builder`] holds every conversion from a committed analysis type
//! ([`phx_spectrogram::Tile`], [`phx_pitch::PitchTrack`],
//! [`phx_formant::FormantTrack`], [`phx_intensity::IntensityTrack`],
//! [`phx_annot::TierSlot`]) into an embedded [`Layer`], and
//! [`builder::reference_figure`] assembles the roadmap gate figure that every
//! export backend reuses as its input.
#![warn(missing_docs)]

pub mod backends;
pub mod builder;
pub mod model;
pub mod style;

pub use backends::to_svg;
pub use backends::{
    CodeExport, CodeLang, SidecarFile, TextExport, figure_tiers, to_code, to_graphml, to_tikz,
    to_typst, to_vega,
};
#[cfg(feature = "pdf")]
pub use backends::{PdfError, to_pdf};
#[cfg(feature = "raster")]
pub use backends::{PngError, to_png};
pub use builder::{
    FigureBuilder, formant_layer, intensity_layer, pitch_layer, reference_figure,
    spectral_slice_layer, spectrogram_layer, tier_data, tiers_layer, waveform_layer,
    waveform_minmax,
};
pub use model::{
    Axis, AxisScale, CaptionMeta, Figure, FigureError, IntervalData, Layer, LayerKind, LengthUnit,
    MinMax, Panel, PitchUnit, PointData, ProvenanceRecord, SizeSpec, SpeckleFrame, SpecklePoint,
    TierContent, TierData,
};
pub use style::{DashStyle, LineStyle, RgbaColor, SpeckleStyle, TierStyle};

#[cfg(test)]
mod tests {
    use super::*;
    use phx_annot::{
        Annotation, BoundaryId, Interval, IntervalId, IntervalTier, Tier, TierId, TierRelation,
        TierSlot,
    };
    use phx_audio::Audio;
    use phx_formant::{FormantParams, formant_track};
    use phx_intensity::{IntensityParams, intensity_track};
    use phx_pitch::{PitchParams, TimeSpan, pitch_track};
    use phx_render::{Colormap, DisplayMapping, Theme};
    use phx_spectrogram::{SpectrogramParams, TileRequest, compute_tile};

    // A short voiced test signal: a 220 Hz tone with a little harmonic content
    // so pitch and formant analyses have something to track.
    fn sine_audio(freq: f64, seconds: f64, sr: f64) -> Audio {
        let n = (seconds * sr) as usize;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                let w = std::f64::consts::TAU * freq * t;
                (0.6 * w.sin() + 0.2 * (2.0 * w).sin() + 0.1 * (3.0 * w).sin()) as f32
            })
            .collect();
        Audio::new(vec![samples], sr).expect("valid audio")
    }

    fn two_interval_annotation() -> Annotation {
        let tier = TierSlot {
            id: TierId::new(1),
            relation: TierRelation::Independent,
            tier: Tier::Interval(IntervalTier {
                name: "words".to_owned(),
                xmin: 0.0,
                xmax: 1.0,
                intervals: vec![
                    Interval {
                        id: IntervalId::new(1),
                        start_boundary: BoundaryId::new(1),
                        end_boundary: BoundaryId::new(2),
                        xmin: 0.0,
                        xmax: 0.5,
                        label: "hello".to_owned(),
                    },
                    Interval {
                        id: IntervalId::new(2),
                        start_boundary: BoundaryId::new(2),
                        end_boundary: BoundaryId::new(3),
                        xmin: 0.5,
                        xmax: 1.0,
                        label: "world".to_owned(),
                    },
                ],
            }),
        };
        Annotation::from_raw(0.0, 1.0, vec![tier]).expect("valid raw document")
    }

    #[test]
    fn px_at_resolves_physical_units() {
        // 2.54 cm is one inch; at 100 dpi that is 100 px.
        let cm = SizeSpec::new(2.54, 5.08, LengthUnit::Cm);
        assert_eq!(cm.px_at(100.0), (100, 200));
        // Inches scale straight by dpi.
        let inch = SizeSpec::new(2.0, 3.0, LengthUnit::In);
        assert_eq!(inch.px_at(300.0), (600, 900));
        // 72 pt is one inch.
        let pt = SizeSpec::new(72.0, 144.0, LengthUnit::Pt);
        assert_eq!(pt.px_at(96.0), (96, 192));
    }

    #[test]
    fn validate_accepts_the_reference_figure() {
        reference_figure().validate().expect("reference is valid");
    }

    #[test]
    fn validate_rejects_a_spectrogram_shape_mismatch() {
        let bad = Layer::Spectrogram {
            db: vec![0.0; 5],
            width: 2,
            height: 3,
            t: [0.0, 1.0],
            f: [0.0, 5000.0],
            display: DisplayMapping::default(),
            colormap: Colormap::Viridis,
        };
        let fig = FigureBuilder::new(SizeSpec::new(10.0, 10.0, LengthUnit::Cm), Theme::Light)
            .panel(Panel {
                layers: vec![bad],
                time_axis: Axis::linear(0.0, 1.0, None, None),
                value_axis: Axis::linear(0.0, 5000.0, None, None),
                height_share: 1.0,
            })
            .build();
        assert_eq!(
            fig.validate(),
            Err(FigureError::SpectrogramShape {
                expected: 6,
                found: 5
            })
        );
    }

    #[test]
    fn validate_rejects_non_positive_size_and_height_share() {
        let fig =
            FigureBuilder::new(SizeSpec::new(0.0, 10.0, LengthUnit::Cm), Theme::Light).build();
        assert_eq!(fig.validate(), Err(FigureError::NonPositiveSize));

        let fig = FigureBuilder::new(SizeSpec::new(10.0, 10.0, LengthUnit::Cm), Theme::Light)
            .panel(Panel {
                layers: vec![],
                time_axis: Axis::linear(0.0, 1.0, None, None),
                value_axis: Axis::linear(0.0, 1.0, None, None),
                height_share: 0.0,
            })
            .build();
        assert_eq!(fig.validate(), Err(FigureError::NonPositiveHeightShare));
    }

    #[test]
    fn spectrogram_layer_embeds_raw_db_matching_tile_shape() {
        let audio = sine_audio(220.0, 0.3, 16000.0);
        let frames = audio.frames();
        let params = SpectrogramParams::default();
        let tile = compute_tile(
            audio.slice_samples(0..frames),
            &TileRequest {
                t0: 0.0,
                t1: audio.duration(),
                f0: 0.0,
                f1: params.max_frequency,
                width_px: 40,
                height_px: 32,
                params,
            },
        );
        let layer = spectrogram_layer(&tile, DisplayMapping::default(), Colormap::Magma);
        match layer {
            Layer::Spectrogram {
                db, width, height, ..
            } => {
                assert_eq!(width as usize, tile.t_axis.len());
                assert_eq!(height as usize, tile.f_axis.len());
                assert_eq!(db.len(), width as usize * height as usize);
                assert_eq!(db, tile.db);
                // Raw dB, not baked bytes: values are decibels, well below 0.
                assert!(db.iter().any(|&v| v < 0.0));
            }
            _ => panic!("expected a spectrogram layer"),
        }
    }

    #[test]
    fn pitch_layer_carries_only_voiced_finite_points() {
        let audio = sine_audio(220.0, 0.4, 16000.0);
        let frames = audio.frames();
        let track = pitch_track(audio.slice_samples(0..frames), &PitchParams::default());
        let layer = pitch_layer(&track, PitchUnit::Hertz, LineStyle::default());
        match layer {
            Layer::PitchLine { points, unit, .. } => {
                assert_eq!(unit, PitchUnit::Hertz);
                assert!(!points.is_empty());
                assert!(
                    points
                        .iter()
                        .all(|(t, v)| t.is_finite() && v.is_finite() && *v > 0.0)
                );
            }
            _ => panic!("expected a pitch layer"),
        }
    }

    #[test]
    fn intensity_layer_matches_track_iteration() {
        let audio = sine_audio(220.0, 0.4, 16000.0);
        let frames = audio.frames();
        let track = intensity_track(audio.slice_samples(0..frames), &IntensityParams::default());
        let expected: Vec<(f64, f64)> = track.iter().collect();
        let layer = intensity_layer(&track, LineStyle::default());
        match layer {
            Layer::IntensityLine { points, .. } => {
                assert_eq!(points, expected);
            }
            _ => panic!("expected an intensity layer"),
        }
    }

    #[test]
    fn formant_layer_records_smoothing_and_frame_shape() {
        let audio = sine_audio(220.0, 0.3, 16000.0);
        let frames = audio.frames();
        let track = formant_track(audio.slice_samples(0..frames), &FormantParams::default());
        let layer = formant_layer(&track, true, SpeckleStyle::default());
        match layer {
            Layer::FormantSpeckle {
                frames: speckle,
                smoothed,
                ..
            } => {
                assert!(smoothed);
                assert_eq!(speckle.len(), track.frames.len());
                for (sf, ff) in speckle.iter().zip(&track.frames) {
                    assert_eq!(sf.time, ff.time);
                    assert_eq!(sf.points.len(), ff.formants.len());
                }
            }
            _ => panic!("expected a formant speckle layer"),
        }
    }

    #[test]
    fn tiers_layer_embeds_interval_data() {
        let annot = two_interval_annotation();
        let layer = tiers_layer(annot.tiers(), TierStyle::default());
        match layer {
            Layer::Tiers { tiers, .. } => {
                assert_eq!(tiers.len(), 1);
                assert_eq!(tiers[0].name, "words");
                match &tiers[0].content {
                    TierContent::Intervals(intervals) => {
                        assert_eq!(intervals.len(), 2);
                        assert_eq!(intervals[0].label, "hello");
                        assert_eq!(intervals[1].xmin, 0.5);
                    }
                    TierContent::Points(_) => panic!("expected intervals"),
                }
            }
            _ => panic!("expected a tiers layer"),
        }
    }

    #[test]
    fn waveform_minmax_buckets_cover_all_samples() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0) - 0.5).collect();
        let env = waveform_minmax(&samples, 10);
        assert_eq!(env.len(), 10);
        // The global extremes appear in the first and last buckets.
        assert_eq!(env[0].min, -0.5);
        assert!(env[9].max > 0.49);
        // Every bucket is well-formed.
        assert!(env.iter().all(|m| m.min <= m.max));
        assert!(waveform_minmax(&[], 10).is_empty());
        assert!(waveform_minmax(&samples, 0).is_empty());
    }

    #[test]
    fn reference_figure_has_the_gate_layers() {
        let fig = reference_figure();
        let kinds: Vec<LayerKind> = fig
            .panels
            .iter()
            .flat_map(|p| p.layers.iter().map(Layer::kind))
            .collect();
        assert!(kinds.contains(&LayerKind::Waveform));
        assert!(kinds.contains(&LayerKind::Spectrogram));
        assert!(kinds.contains(&LayerKind::Pitch));
        assert!(kinds.contains(&LayerKind::Tiers));
        // Provenance records the spectrogram and pitch analyses.
        assert_eq!(fig.caption_meta.sources.len(), 2);
    }

    #[test]
    fn two_reference_builds_serialize_byte_identically() {
        // Building the gate figure twice must produce byte-identical JSON:
        // the model carries no HashMap iteration order, timestamp, or other
        // run-to-run nondeterminism, so exports and wire traffic are stable.
        let first = reference_figure().to_json().expect("serialize first");
        let second = reference_figure().to_json().expect("serialize second");
        assert_eq!(first, second);
    }

    #[test]
    fn json_round_trip_is_byte_identical() {
        let fig = reference_figure();
        let first = fig.to_json().expect("serialize");
        let decoded = Figure::from_json(&first).expect("deserialize");
        let second = decoded.to_json().expect("reserialize");
        assert_eq!(first, second);
        assert_eq!(fig, decoded);
    }

    #[test]
    fn json_round_trip_preserves_remote_typed_fields() {
        let audio = sine_audio(220.0, 0.2, 16000.0);
        let frames = audio.frames();
        let params = SpectrogramParams::default();
        let tile = compute_tile(
            audio.slice_samples(0..frames),
            &TileRequest {
                t0: 0.0,
                t1: audio.duration(),
                f0: 0.0,
                f1: params.max_frequency,
                width_px: 16,
                height_px: 16,
                params,
            },
        );
        let fig = FigureBuilder::new(SizeSpec::new(8.0, 6.0, LengthUnit::In), Theme::Dark)
            .panel(Panel {
                layers: vec![
                    spectrogram_layer(
                        &tile,
                        DisplayMapping {
                            dynamic_range_db: 42.0,
                            max_db: Some(-3.0),
                        },
                        Colormap::Grayscale,
                    ),
                    waveform_layer(
                        vec![MinMax {
                            min: -0.4,
                            max: 0.4,
                        }],
                        TimeSpan::new(0.0, audio.duration()),
                        LineStyle::default(),
                    ),
                ],
                time_axis: Axis::linear(0.0, 0.2, Some("Time"), Some("s")),
                value_axis: Axis::linear(0.0, 5000.0, None, Some("Hz")),
                height_share: 1.0,
            })
            .build();

        let decoded = Figure::from_json(&fig.to_json().unwrap()).expect("round trip");
        assert_eq!(fig, decoded);
    }
}
