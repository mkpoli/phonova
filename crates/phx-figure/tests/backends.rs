//! Backend export tests over the shared `reference_figure()` gate input.

use phx_figure::{
    Axis, DashStyle, Figure, FigureBuilder, IntervalData, Layer, LengthUnit, LineStyle, MinMax,
    Panel, PitchUnit, PointData, RgbaColor, SizeSpec, SpeckleFrame, SpecklePoint, SpeckleStyle,
    TierContent, TierData, TierStyle, reference_figure, to_svg,
};
use phx_pitch::TimeSpan;
use phx_render::{Colormap, DisplayMapping, Theme};

/// Reads the width and height fields of a PNG's IHDR chunk.
#[cfg(feature = "raster")]
fn png_dims(bytes: &[u8]) -> (u32, u32) {
    assert!(bytes.len() > 24, "png too short");
    assert_eq!(&bytes[1..4], b"PNG", "missing PNG signature");
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w, h)
}

/// A figure exercising every [`Layer`] variant in one export.
fn all_layers_figure(theme: Theme) -> Figure {
    let db: Vec<f32> = (0..(8 * 6)).map(|i| -60.0 + i as f32).collect();
    let spectrogram = Layer::Spectrogram {
        db,
        width: 8,
        height: 6,
        t: [0.0, 1.0],
        f: [0.0, 5000.0],
        display: DisplayMapping::default(),
        colormap: Colormap::Viridis,
    };
    let waveform = Layer::Waveform {
        minmax: vec![
            MinMax {
                min: -0.5,
                max: 0.5,
            },
            MinMax {
                min: -0.8,
                max: 0.7,
            },
            MinMax {
                min: -0.2,
                max: 0.3,
            },
        ],
        span: TimeSpan::new(0.0, 1.0),
        style: LineStyle::default(),
    };
    let pitch = Layer::PitchLine {
        points: vec![(0.1, 120.0), (0.2, 130.0), (0.8, 140.0)],
        unit: PitchUnit::Hertz,
        style: LineStyle::solid(1.5, RgbaColor::rgb(30, 90, 220)),
    };
    let formants = Layer::FormantSpeckle {
        frames: vec![
            SpeckleFrame {
                time: 0.2,
                points: vec![
                    SpecklePoint {
                        frequency: 700.0,
                        bandwidth: 80.0,
                    },
                    SpecklePoint {
                        frequency: 1800.0,
                        bandwidth: 200.0,
                    },
                ],
            },
            SpeckleFrame {
                time: 0.6,
                points: vec![SpecklePoint {
                    frequency: 900.0,
                    bandwidth: 400.0,
                }],
            },
        ],
        smoothed: true,
        style: SpeckleStyle::default(),
    };
    let intensity = Layer::IntensityLine {
        points: vec![(0.0, 40.0), (0.5, 65.0), (1.0, 55.0)],
        style: LineStyle::solid(1.0, RgbaColor::rgb(20, 160, 60)),
    };
    let slice = Layer::SpectralSlice {
        bins: vec![(0.0, -60.0), (2500.0, -20.0), (5000.0, -50.0)],
        style: LineStyle::default(),
    };
    let tiers = Layer::Tiers {
        tiers: vec![
            TierData {
                name: "words".to_owned(),
                content: TierContent::Intervals(vec![
                    IntervalData {
                        xmin: 0.0,
                        xmax: 0.5,
                        label: "hello".to_owned(),
                    },
                    IntervalData {
                        xmin: 0.5,
                        xmax: 1.0,
                        label: "world".to_owned(),
                    },
                ]),
            },
            TierData {
                name: "marks".to_owned(),
                content: TierContent::Points(vec![PointData {
                    time: 0.5,
                    label: "M".to_owned(),
                }]),
            },
        ],
        style: TierStyle::default(),
    };

    let time = || Axis::linear(0.0, 1.0, Some("Time"), Some("s"));
    FigureBuilder::new(SizeSpec::new(14.0, 12.0, LengthUnit::Cm), theme)
        .panel(Panel {
            layers: vec![waveform],
            time_axis: time(),
            value_axis: Axis::linear(-1.0, 1.0, Some("Amplitude"), None),
            height_share: 0.2,
        })
        .panel(Panel {
            layers: vec![spectrogram, formants],
            time_axis: time(),
            value_axis: Axis::linear(0.0, 5000.0, Some("Frequency"), Some("Hz")),
            height_share: 0.3,
        })
        .panel(Panel {
            layers: vec![pitch],
            time_axis: time(),
            value_axis: Axis::linear(75.0, 600.0, Some("Pitch"), Some("Hz")),
            height_share: 0.15,
        })
        .panel(Panel {
            layers: vec![intensity],
            time_axis: time(),
            value_axis: Axis::linear(30.0, 90.0, Some("Intensity"), Some("dB")),
            height_share: 0.15,
        })
        .panel(Panel {
            layers: vec![slice],
            time_axis: Axis::linear(0.0, 5000.0, Some("Frequency"), Some("Hz")),
            value_axis: Axis::linear(-70.0, 0.0, Some("Power"), Some("dB")),
            height_share: 0.1,
        })
        .panel(Panel {
            layers: vec![tiers],
            time_axis: time(),
            value_axis: Axis::linear(0.0, 1.0, None, None),
            height_share: 0.1,
        })
        .build()
}

#[test]
fn svg_render_is_byte_deterministic() {
    let fig = reference_figure();
    let a = to_svg(&fig);
    let b = to_svg(&fig);
    assert_eq!(
        a, b,
        "two renders of the same figure must be byte-identical"
    );
    // The embedded spectrogram PNG is part of that determinism.
    assert!(a.contains("data:image/png;base64,"));
}

#[test]
fn svg_is_well_formed_and_carries_every_surface() {
    let fig = reference_figure();
    let svg = to_svg(&fig);
    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));
    // One embedded raster per spectrogram layer, vector paths for the tracks.
    assert_eq!(svg.matches("<image").count(), 1);
    assert!(svg.contains("<path"), "tracks render as vector paths");
    // Axis titles and a known tier label survive into the document.
    assert!(svg.contains("Frequency (Hz)"));
    assert!(svg.contains("Time (s)"));
    assert!(svg.contains("danger"));
    // Caption provenance is present.
    assert!(svg.contains("Spectrogram"));
    assert!(svg.contains("window_length_s"));
}

#[test]
fn svg_themes_produce_distinct_documents() {
    let mut dark = reference_figure();
    dark.theme = Theme::Dark;
    let mut light = dark.clone();
    light.theme = Theme::Light;
    assert_ne!(
        to_svg(&dark),
        to_svg(&light),
        "themes must render differently"
    );
}

#[test]
fn every_layer_variant_renders() {
    for theme in [Theme::Dark, Theme::Light] {
        let svg = to_svg(&all_layers_figure(theme));
        assert_eq!(svg.matches("<image").count(), 1);
        assert!(svg.contains("hello") && svg.contains("world"));
        assert!(
            svg.contains("<circle"),
            "formant speckles render as circles"
        );
    }
}

#[test]
fn dashed_line_style_reaches_the_svg() {
    let fig = FigureBuilder::new(SizeSpec::new(8.0, 4.0, LengthUnit::Cm), Theme::Light)
        .panel(Panel {
            layers: vec![Layer::IntensityLine {
                points: vec![(0.0, 40.0), (0.2, 50.0), (0.4, 45.0)],
                style: LineStyle {
                    width_pt: 1.0,
                    color: RgbaColor::rgb(200, 0, 0),
                    dash: DashStyle::Dashed,
                },
            }],
            time_axis: Axis::linear(0.0, 0.4, Some("Time"), Some("s")),
            value_axis: Axis::linear(30.0, 60.0, Some("Intensity"), Some("dB")),
            height_share: 1.0,
        })
        .build();
    assert!(to_svg(&fig).contains("stroke-dasharray"));
}

#[cfg(feature = "raster")]
#[test]
fn png_has_expected_dimensions_at_dpi() {
    use phx_figure::to_png;
    let fig = reference_figure();
    for dpi in [96.0, 192.0] {
        let png = to_png(&fig, dpi).expect("png renders");
        let (w, h) = png_dims(&png);
        assert_eq!((w, h), fig.size.px_at(dpi), "png size tracks dpi");
    }
}

#[cfg(feature = "raster")]
#[test]
fn png_parses_the_svg_and_covers_all_layers() {
    use phx_figure::to_png;
    // A successful render proves the SVG parses under usvg (resvg's parser).
    let png = to_png(&all_layers_figure(Theme::Dark), 96.0).expect("png renders");
    assert!(png.len() > 1000);
    assert_eq!(&png[1..4], b"PNG");
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_is_a_nontrivial_pdf_document() {
    use phx_figure::to_pdf;
    let fig = reference_figure();
    let pdf = to_pdf(&fig).expect("pdf converts");
    assert!(pdf.starts_with(b"%PDF-"), "PDF header present");
    assert!(
        pdf.len() > 4000,
        "PDF carries real content, got {} bytes",
        pdf.len()
    );
    // svg2pdf parsing the SVG is itself proof the scene graph is valid.
}
