//! Assembling figures out of the committed analysis types.
//!
//! Every conversion from an analysis result to an embedded [`Layer`] lives
//! here: a spectrogram [`Tile`] becomes a raw-decibel layer, a [`PitchTrack`]
//! becomes a point series, a [`FormantTrack`] becomes a speckle, an
//! [`IntensityTrack`] becomes a contour, and annotation [`TierSlot`]s become
//! embedded tiers. The functions copy data out of the analysis types, so the
//! resulting [`Figure`] holds no reference back to them.

use std::collections::BTreeMap;

use phx_annot::{Tier, TierSlot};
use phx_formant::FormantTrack;
use phx_intensity::IntensityTrack;
use phx_pitch::{PitchParams, PitchTrack, TimeSpan};
use phx_render::{Colormap, DisplayMapping, Theme};
use phx_spectrogram::{Slice, SpectrogramParams, Tile, TileRequest};

use crate::model::{
    Axis, CaptionMeta, Figure, IntervalData, Layer, LayerKind, MinMax, Panel, PitchUnit, PointData,
    ProvenanceRecord, SizeSpec, SpeckleFrame, SpecklePoint, TierContent, TierData,
};
use crate::style::{LineStyle, SpeckleStyle, TierStyle};

/// Fluent assembler for a [`Figure`].
///
/// Panels stack top to bottom in the order they are added; provenance records
/// accumulate in the caption in the order they are added.
#[derive(Debug, Clone)]
pub struct FigureBuilder {
    size: SizeSpec,
    theme: Theme,
    panels: Vec<Panel>,
    caption_meta: CaptionMeta,
}

impl FigureBuilder {
    /// Starts a builder for a figure of physical `size` described against
    /// `theme`.
    #[must_use]
    pub fn new(size: SizeSpec, theme: Theme) -> Self {
        Self {
            size,
            theme,
            panels: Vec::new(),
            caption_meta: CaptionMeta::default(),
        }
    }

    /// Appends a panel below the previously added panels.
    #[must_use]
    pub fn panel(mut self, panel: Panel) -> Self {
        self.panels.push(panel);
        self
    }

    /// Appends a caption provenance record.
    #[must_use]
    pub fn source(mut self, record: ProvenanceRecord) -> Self {
        self.caption_meta.sources.push(record);
        self
    }

    /// Finishes the figure.
    #[must_use]
    pub fn build(self) -> Figure {
        Figure {
            size: self.size,
            theme: self.theme,
            panels: self.panels,
            caption_meta: self.caption_meta,
        }
    }
}

/// A waveform envelope layer over `span`.
#[must_use]
pub fn waveform_layer(minmax: Vec<MinMax>, span: TimeSpan, style: LineStyle) -> Layer {
    Layer::Waveform {
        minmax,
        span,
        style,
    }
}

/// Computes a waveform min/max envelope of `buckets` columns from `samples`.
///
/// Each bucket spans a contiguous, near-equal share of the samples and holds
/// the exact minimum and maximum over its range. Returns an empty envelope
/// when `samples` is empty or `buckets` is zero.
#[must_use]
pub fn waveform_minmax(samples: &[f32], buckets: usize) -> Vec<MinMax> {
    if samples.is_empty() || buckets == 0 {
        return Vec::new();
    }
    let buckets = buckets.min(samples.len());
    let mut out = Vec::with_capacity(buckets);
    for i in 0..buckets {
        let lo = i * samples.len() / buckets;
        let hi = (i + 1) * samples.len() / buckets;
        let span = &samples[lo..hi];
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for &s in span {
            min = min.min(s);
            max = max.max(s);
        }
        out.push(MinMax { min, max });
    }
    out
}

/// A spectrogram layer built from a [`Tile`], storing raw decibels for
/// export-time colorization.
///
/// The time and frequency ranges come from the tile's snapped axes; `display`
/// and `colormap` are carried unevaluated so a backend calls
/// [`phx_render::colorize`] at export.
#[must_use]
pub fn spectrogram_layer(tile: &Tile, display: DisplayMapping, colormap: Colormap) -> Layer {
    let width = tile.t_axis.len() as u32;
    let height = tile.f_axis.len() as u32;
    let t0 = tile.t_axis.first().copied().unwrap_or(0.0);
    let t1 = tile.t_axis.last().copied().unwrap_or(0.0);
    let f0 = tile.f_axis.first().copied().unwrap_or(0.0);
    let f1 = tile.f_axis.last().copied().unwrap_or(0.0);
    Layer::Spectrogram {
        db: tile.db.clone(),
        width,
        height,
        t: [t0, t1],
        f: [f0, f1],
        display,
        colormap,
    }
}

/// A pitch contour layer in `unit`.
///
/// Only voiced frames contribute points; unvoiced frames are omitted, so the
/// point series carries no non-finite values.
#[must_use]
pub fn pitch_layer(track: &PitchTrack, unit: PitchUnit, style: LineStyle) -> Layer {
    let points = track
        .frames()
        .iter()
        .filter_map(|frame| {
            frame.f0.map(|hz| {
                let value = match unit {
                    PitchUnit::Hertz => hz,
                    // Semitones re 1 Hz, Praat's Hertz-to-semitone reference.
                    PitchUnit::Semitones => 12.0 * hz.log2(),
                };
                (frame.time, value)
            })
        })
        .collect();
    Layer::PitchLine {
        points,
        unit,
        style,
    }
}

/// A formant speckle layer.
///
/// `smoothed` records whether `track` carries Viterbi-smoothed slots or raw
/// candidates, per the formant-tracking caveat.
#[must_use]
pub fn formant_layer(track: &FormantTrack, smoothed: bool, style: SpeckleStyle) -> Layer {
    let frames = track
        .frames
        .iter()
        .map(|frame| SpeckleFrame {
            time: frame.time,
            points: frame
                .formants
                .iter()
                .map(|p| SpecklePoint {
                    frequency: p.frequency,
                    bandwidth: p.bandwidth,
                })
                .collect(),
        })
        .collect();
    Layer::FormantSpeckle {
        frames,
        smoothed,
        style,
    }
}

/// An intensity contour layer, one point per analysis frame.
#[must_use]
pub fn intensity_layer(track: &IntensityTrack, style: LineStyle) -> Layer {
    Layer::IntensityLine {
        points: track.iter().collect(),
        style,
    }
}

/// Converts one annotation [`TierSlot`] into embedded [`TierData`].
#[must_use]
pub fn tier_data(slot: &TierSlot) -> TierData {
    match &slot.tier {
        Tier::Interval(tier) => TierData {
            name: tier.name.clone(),
            content: TierContent::Intervals(
                tier.intervals
                    .iter()
                    .map(|iv| IntervalData {
                        xmin: iv.xmin,
                        xmax: iv.xmax,
                        label: iv.label.clone(),
                    })
                    .collect(),
            ),
        },
        Tier::Point(tier) => TierData {
            name: tier.name.clone(),
            content: TierContent::Points(
                tier.points
                    .iter()
                    .map(|p| PointData {
                        time: p.time,
                        label: p.label.clone(),
                    })
                    .collect(),
            ),
        },
    }
}

/// A tiers layer embedding `slots` in order.
#[must_use]
pub fn tiers_layer(slots: &[TierSlot], style: TierStyle) -> Layer {
    Layer::Tiers {
        tiers: slots.iter().map(tier_data).collect(),
        style,
    }
}

/// A spectral-slice layer of `(frequency, dB)` bins built from a [`Slice`].
#[must_use]
pub fn spectral_slice_layer(slice: &Slice, style: LineStyle) -> Layer {
    let bins = slice
        .f_axis
        .iter()
        .zip(slice.db.iter())
        .map(|(&f, &db)| (f, f64::from(db)))
        .collect();
    Layer::SpectralSlice { bins, style }
}

fn provenance(
    layer: LayerKind,
    params: &[(&str, String)],
    smoothed: Option<bool>,
) -> ProvenanceRecord {
    let params = params
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect::<BTreeMap<_, _>>();
    ProvenanceRecord {
        layer,
        params,
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
    let step = params
        .time_step
        .map_or_else(|| "auto".to_owned(), |s| format!("{s}"));
    provenance(
        LayerKind::Pitch,
        &[
            ("floor_hz", format!("{}", params.floor_hz)),
            ("ceiling_hz", format!("{}", params.ceiling_hz)),
            ("time_step_s", step),
        ],
        None,
    )
}

const REFERENCE_WAV: &[u8] = include_bytes!("../../../tests/fixtures/audio/arctic_bdl_a0001.wav");
const REFERENCE_TEXTGRID: &[u8] =
    include_bytes!("../../../tests/fixtures/textgrids/arctic_bdl_a0001_long_utf8.TextGrid");

/// Builds the roadmap gate figure: waveform, spectrogram, pitch, and one tier
/// from `arctic_bdl_a0001` and its fixture TextGrid.
///
/// Every export backend reuses this figure as its gate input. The figure runs
/// the default spectrogram and pitch analyses over the whole clip, envelopes
/// the mono mix, and embeds the TextGrid's `words` interval tier.
///
/// # Panics
/// Panics if the bundled fixture WAV or TextGrid fails to decode, which would
/// mean the fixtures themselves are corrupt.
#[must_use]
pub fn reference_figure() -> Figure {
    use phx_audio::Audio;
    use phx_pitch::pitch_track;
    use phx_spectrogram::compute_tile;

    let audio = Audio::from_wav_bytes(REFERENCE_WAV).expect("reference fixture WAV must decode");
    let frames = audio.frames();
    let duration = audio.duration();

    let spec_params = SpectrogramParams::default();
    let tile = compute_tile(
        audio.slice_samples(0..frames),
        &TileRequest {
            t0: 0.0,
            t1: duration,
            f0: 0.0,
            f1: spec_params.max_frequency,
            width_px: 800,
            height_px: 256,
            params: spec_params,
        },
    );

    let pitch_params = PitchParams::default();
    let pitch = pitch_track(audio.slice_samples(0..frames), &pitch_params);

    let mono = audio.mono_mix();
    let envelope = waveform_minmax(&mono, 1000);
    let span = TimeSpan::new(0.0, duration);

    let (annotation, _) =
        phx_textgrid::read(REFERENCE_TEXTGRID).expect("reference fixture TextGrid must parse");
    let words = annotation
        .tiers()
        .iter()
        .find(|slot| tier_name(slot) == "words")
        .or_else(|| annotation.tiers().first())
        .expect("reference TextGrid must have at least one tier");

    let time_axis = || Axis::linear(0.0, duration, Some("Time"), Some("s"));

    let waveform_panel = Panel {
        layers: vec![waveform_layer(envelope, span, LineStyle::default())],
        time_axis: time_axis(),
        value_axis: Axis::linear(-1.0, 1.0, Some("Amplitude"), None),
        height_share: 0.22,
    };

    let spectrogram_panel = Panel {
        layers: vec![spectrogram_layer(
            &tile,
            DisplayMapping::default(),
            Colormap::Viridis,
        )],
        time_axis: time_axis(),
        value_axis: Axis::linear(
            0.0,
            spec_params.max_frequency,
            Some("Frequency"),
            Some("Hz"),
        ),
        height_share: 0.44,
    };

    let pitch_panel = Panel {
        layers: vec![pitch_layer(&pitch, PitchUnit::Hertz, LineStyle::default())],
        time_axis: time_axis(),
        value_axis: Axis::linear(
            pitch_params.floor_hz,
            pitch_params.ceiling_hz,
            Some("Pitch"),
            Some("Hz"),
        ),
        height_share: 0.22,
    };

    let tier_panel = Panel {
        layers: vec![tiers_layer(
            std::slice::from_ref(words),
            TierStyle::default(),
        )],
        time_axis: time_axis(),
        value_axis: Axis::linear(0.0, 1.0, None, None),
        height_share: 0.12,
    };

    FigureBuilder::new(
        SizeSpec::new(16.0, 12.0, crate::model::LengthUnit::Cm),
        Theme::Dark,
    )
    .panel(waveform_panel)
    .panel(spectrogram_panel)
    .panel(pitch_panel)
    .panel(tier_panel)
    .source(spectrogram_provenance(&spec_params))
    .source(pitch_provenance(&pitch_params))
    .build()
}

fn tier_name(slot: &TierSlot) -> &str {
    match &slot.tier {
        Tier::Interval(tier) => &tier.name,
        Tier::Point(tier) => &tier.name,
    }
}
