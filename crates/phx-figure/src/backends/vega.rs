//! Vega-Lite v5 backend.
//!
//! [`to_vega`] emits a Vega-Lite v5 specification: a vertical concatenation of
//! one view per panel over a shared time axis. Tracks and tiers travel as
//! inline data; each spectrogram is a data-URI PNG drawn with an `image` mark
//! across its time/frequency extent.
//!
//! The JSON is built through `serde_json`, so it is always well-formed and
//! serializes deterministically.

use base64::Engine as _;
use serde_json::{Value, json};

use crate::backends::bundle::{axis_title, db_window, spectrogram_png};
use crate::model::{Axis, Figure, Layer, Panel, TierContent};

/// Renders `fig` to a Vega-Lite v5 JSON string.
#[must_use]
pub fn to_vega(fig: &Figure) -> String {
    let width = (fig.size.width_in() * 96.0).round().max(120.0);
    let total_h = (fig.size.height_in() * 96.0).round().max(120.0);
    let share_sum: f64 = fig.panels.iter().map(|p| p.height_share.max(0.0)).sum();
    let share_sum = if share_sum > 0.0 { share_sum } else { 1.0 };

    let views: Vec<Value> = fig
        .panels
        .iter()
        .enumerate()
        .map(|(idx, panel)| {
            let is_last = idx + 1 == fig.panels.len();
            let ph = (total_h * panel.height_share.max(0.0) / share_sum).max(24.0);
            panel_view(panel, width, ph, is_last, fig.theme)
        })
        .collect();

    let spec = json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "description": caption_text(fig),
        "resolve": { "scale": { "x": "independent", "y": "independent" } },
        "spacing": 8,
        "vconcat": views,
    });

    serde_json::to_string_pretty(&spec).unwrap_or_else(|_| "{}".to_owned())
}

fn caption_text(fig: &Figure) -> String {
    let mut parts = vec!["Phonetics figure over a shared time axis (s).".to_owned()];
    for src in &fig.caption_meta.sources {
        let mut line = format!("{:?}", src.layer);
        if !src.params.is_empty() {
            let ps: Vec<String> = src.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            line.push_str(&format!(" ({})", ps.join(", ")));
        }
        if src.smoothed == Some(true) {
            line.push_str(" [smoothed track]");
        }
        parts.push(line);
    }
    parts.join(" ")
}

fn time_axis_encoding(axis: &Axis, show_labels: bool) -> Value {
    json!({
        "field": "t",
        "type": "quantitative",
        "scale": { "domain": [axis.min, axis.max], "nice": false, "zero": false },
        "axis": {
            "title": if show_labels { axis_title(axis) } else { None },
            "labels": show_labels,
            "ticks": show_labels,
        },
    })
}

fn value_axis_encoding(field: &str, axis: &Axis) -> Value {
    json!({
        "field": field,
        "type": "quantitative",
        "scale": { "domain": [axis.min, axis.max], "nice": false, "zero": false },
        "axis": { "title": axis_title(axis) },
    })
}

fn panel_view(
    panel: &Panel,
    width: f64,
    height: f64,
    is_last: bool,
    theme: phx_render::Theme,
) -> Value {
    // A panel becomes a single layered view: its layers stack in draw order.
    let mut layers: Vec<Value> = Vec::new();
    for layer in &panel.layers {
        layers.extend(layer_specs(layer, panel, width, height, theme));
    }
    if layers.is_empty() {
        layers.push(json!({ "data": { "values": [] }, "mark": "point" }));
    }

    json!({
        "width": width,
        "height": height,
        "encoding": {
            "x": time_axis_encoding(&panel.time_axis, is_last),
        },
        "layer": layers,
    })
}

fn layer_specs(
    layer: &Layer,
    panel: &Panel,
    width: f64,
    height: f64,
    theme: phx_render::Theme,
) -> Vec<Value> {
    match layer {
        Layer::Waveform {
            minmax,
            span,
            style,
        } => {
            let n = minmax.len();
            let values: Vec<Value> = minmax
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let frac = if n > 1 {
                        i as f64 / (n - 1) as f64
                    } else {
                        0.0
                    };
                    let t = span.start + frac * (span.end - span.start);
                    json!({ "t": t, "lo": m.min, "hi": m.max })
                })
                .collect();
            vec![json!({
                "data": { "values": values },
                "mark": { "type": "area", "color": hex(style.color), "opacity": 0.85, "line": false },
                "encoding": {
                    "y": value_axis_encoding("lo", &panel.value_axis),
                    "y2": { "field": "hi" },
                },
            })]
        }
        Layer::Spectrogram {
            db,
            width: sw,
            height: sh,
            t,
            f,
            display,
            colormap,
        } => {
            let png = spectrogram_png(db, *sw, *sh, display, *colormap, theme);
            let uri = format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&png)
            );
            let _ = db_window(db, display); // window is baked into the colorized PNG
            vec![
                json!({
                    "data": { "values": [ { "img": uri } ] },
                    "mark": {
                        "type": "image",
                        "width": width,
                        "height": height,
                        "align": "left",
                        "baseline": "top",
                        "aspect": false,
                    },
                    "encoding": {
                        "url": { "field": "img", "type": "nominal" },
                        "x": { "value": 0 },
                        "y": { "value": 0 },
                    },
                }),
                // A transparent point layer forces the frequency axis to render the
                // panel's value domain over the image.
                json!({
                    "data": { "values": [ { "t": t[0], "f": f[0] }, { "t": t[1], "f": f[1] } ] },
                    "mark": { "type": "point", "opacity": 0 },
                    "encoding": {
                        "y": value_axis_encoding("f", &panel.value_axis),
                    },
                }),
            ]
        }
        Layer::PitchLine { points, style, .. } | Layer::IntensityLine { points, style } => {
            vec![line_layer(points, &panel.value_axis, style)]
        }
        Layer::SpectralSlice { bins, style } => {
            vec![line_layer(bins, &panel.value_axis, style)]
        }
        Layer::FormantSpeckle { frames, style, .. } => {
            let mut values = Vec::new();
            for frame in frames {
                for p in &frame.points {
                    if p.frequency < panel.value_axis.min || p.frequency > panel.value_axis.max {
                        continue;
                    }
                    values.push(json!({ "t": frame.time, "f": p.frequency }));
                }
            }
            vec![json!({
                "data": { "values": values },
                "mark": { "type": "point", "filled": true, "size": 6, "color": hex(style.color) },
                "encoding": {
                    "y": value_axis_encoding("f", &panel.value_axis),
                },
            })]
        }
        Layer::Tiers { tiers, style } => tier_layers(tiers, style),
    }
}

fn line_layer(points: &[(f64, f64)], value_axis: &Axis, style: &crate::style::LineStyle) -> Value {
    // Insert a null break where the time gap jumps past the typical step, so the
    // line does not bridge an unvoiced hole.
    let mut gaps: Vec<f64> = points
        .windows(2)
        .map(|p| p[1].0 - p[0].0)
        .filter(|d| *d > 0.0)
        .collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let typical = if gaps.is_empty() {
        f64::INFINITY
    } else {
        gaps[gaps.len() / 2]
    };
    let break_gap = typical * 2.5;

    let mut values = Vec::new();
    let mut prev = f64::NAN;
    for &(t, v) in points {
        if prev.is_finite() && t - prev > break_gap {
            values.push(json!({ "t": (prev + t) / 2.0, "v": Value::Null }));
        }
        values.push(json!({ "t": t, "v": v }));
        prev = t;
    }
    json!({
        "data": { "values": values },
        "mark": { "type": "line", "color": hex(style.color), "strokeWidth": style.width_pt },
        "encoding": {
            "y": value_axis_encoding("v", value_axis),
        },
    })
}

fn tier_layers(tiers: &[crate::model::TierData], style: &crate::style::TierStyle) -> Vec<Value> {
    let mut rules = Vec::new();
    let mut labels = Vec::new();
    for tier in tiers {
        match &tier.content {
            TierContent::Intervals(intervals) => {
                for iv in intervals {
                    rules.push(json!({ "t": iv.xmin }));
                    if !iv.label.is_empty() {
                        labels.push(json!({ "t": (iv.xmin + iv.xmax) / 2.0, "label": iv.label }));
                    }
                }
                if let Some(last) = intervals.last() {
                    rules.push(json!({ "t": last.xmax }));
                }
            }
            TierContent::Points(points) => {
                for pt in points {
                    rules.push(json!({ "t": pt.time }));
                    if !pt.label.is_empty() {
                        labels.push(json!({ "t": pt.time, "label": pt.label }));
                    }
                }
            }
        }
    }
    vec![
        json!({
            "data": { "values": rules },
            "mark": { "type": "rule", "color": hex(style.boundary.color) },
        }),
        json!({
            "data": { "values": labels },
            "mark": { "type": "text", "fontSize": style.text_pt, "baseline": "middle" },
            "encoding": { "text": { "field": "label", "type": "nominal" } },
        }),
    ]
}

fn hex(color: crate::style::RgbaColor) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}
