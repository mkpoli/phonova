//! Typst/CeTZ backend.
//!
//! [`to_typst`] emits a Typst document that draws the figure on a CeTZ canvas.
//! Panels stack over a shared time axis, tracks and tiers draw as CeTZ
//! primitives, and each spectrogram is a sidecar PNG placed with `image()`
//! across its time/frequency extent.
//!
//! The canvas works in centimeters: one CeTZ unit is one centimeter, so the
//! figure's physical size sets the page and the drawing coordinates directly.
//! Output is deterministic — no timestamps, no generated identifiers.

use std::fmt::Write as _;

use phx_render::Theme;

use crate::backends::bundle::{
    SidecarFile, TextExport, axis_title, fmt_tick, fnum, hex, nice_ticks, spectrogram_png,
    tick_step,
};
use crate::model::{Axis, AxisScale, Figure, Layer, Panel, TierContent};
use crate::style::{DashStyle, LineStyle};

/// The CeTZ version this document imports, pinned so a bundle compiles against
/// a known drawing API.
const CETZ_IMPORT: &str = "@preview/cetz:0.5.2";

/// Theme-resolved canvas colors, mirroring the SVG backend's palette.
struct Palette {
    canvas: &'static str,
    panel_bg: &'static str,
    fg: &'static str,
    grid: &'static str,
    border: &'static str,
}

impl Palette {
    fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                canvas: "#14161a",
                panel_bg: "#1b1e24",
                fg: "#d8dde3",
                grid: "#333942",
                border: "#2c313a",
            },
            Theme::Light => Self {
                canvas: "#ffffff",
                panel_bg: "#ffffff",
                fg: "#1c1c1c",
                grid: "#e2e2e2",
                border: "#bbbbbb",
            },
        }
    }
}

/// A panel's plotting rectangle in canvas centimeters (y increases upward).
struct Rect {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl Rect {
    fn map_x(&self, axis: &Axis, t: f64) -> f64 {
        self.x0 + axis_norm(axis, t) * (self.x1 - self.x0)
    }
    fn map_y(&self, axis: &Axis, v: f64) -> f64 {
        self.y0 + axis_norm(axis, v) * (self.y1 - self.y0)
    }
}

fn axis_norm(axis: &Axis, v: f64) -> f64 {
    let t = match axis.scale {
        AxisScale::Linear => {
            let span = axis.max - axis.min;
            if span.abs() < f64::EPSILON {
                0.0
            } else {
                (v - axis.min) / span
            }
        }
        AxisScale::Log => {
            let lo = axis.min.max(f64::MIN_POSITIVE).log10();
            let hi = axis.max.max(f64::MIN_POSITIVE).log10();
            let x = v.max(f64::MIN_POSITIVE).log10();
            if (hi - lo).abs() < f64::EPSILON {
                0.0
            } else {
                (x - lo) / (hi - lo)
            }
        }
    };
    t.clamp(0.0, 1.0)
}

/// Renders `fig` to a Typst/CeTZ document plus spectrogram sidecars.
#[must_use]
pub fn to_typst(fig: &Figure) -> TextExport {
    let pal = Palette::for_theme(fig.theme);
    let w = fig.size.width_in() * 2.54;
    let h = fig.size.height_in() * 2.54;

    let mut sidecars = Vec::new();
    let mut body = String::with_capacity(16 * 1024);

    // Canvas background fixes the bounding box to the full physical size.
    let _ = writeln!(
        body,
        "  rect((0, 0), ({w}, {h}), fill: rgb(\"{canvas}\"), stroke: none)",
        w = fnum(w),
        h = fnum(h),
        canvas = pal.canvas,
    );

    let left_gutter = 1.5;
    let right_gutter = 1.4;
    let top_pad = 0.3;
    let time_axis_h = 1.1;
    let caption_lh = 0.4;
    let caption_h = if fig.caption_meta.sources.is_empty() {
        0.2
    } else {
        fig.caption_meta.sources.len() as f64 * caption_lh + 0.2
    };
    let panel_gap = 0.35;

    let x0 = left_gutter;
    let x1 = w - right_gutter;
    let content_top = h - top_pad;
    let content_bottom = time_axis_h + caption_h;

    let n = fig.panels.len();
    let share_sum: f64 = fig.panels.iter().map(|p| p.height_share.max(0.0)).sum();
    let share_sum = if share_sum > 0.0 { share_sum } else { 1.0 };
    let usable = (content_top - content_bottom - panel_gap * (n.saturating_sub(1)) as f64).max(0.5);

    let mut rects: Vec<Rect> = Vec::with_capacity(n);
    let mut top = content_top;
    for panel in &fig.panels {
        let ph = usable * panel.height_share.max(0.0) / share_sum;
        rects.push(Rect {
            x0,
            x1,
            y0: top - ph,
            y1: top,
        });
        top -= ph + panel_gap;
    }

    for (idx, (panel, r)) in fig.panels.iter().zip(&rects).enumerate() {
        draw_panel(&mut body, idx, panel, r, &pal, fig.theme, &mut sidecars);
    }

    if let (Some(last), Some(bottom)) = (rects.last(), fig.panels.last()) {
        draw_time_axis(&mut body, &bottom.time_axis, x0, x1, last.y0, &pal);
    }

    draw_caption(&mut body, fig, x0, caption_h, caption_lh, &pal);

    let mut s = String::with_capacity(body.len() + 512);
    s.push_str(
        "// Phonetics figure exported as a Typst/CeTZ document.\n\
         // Panels stack top to bottom over a shared time axis (seconds); value\n\
         // axes carry each panel's unit. A spectrogram panel places its\n\
         // colorized image sidecar across its time and frequency extent.\n",
    );
    let _ = writeln!(s, "#import \"{CETZ_IMPORT}\"");
    let _ = writeln!(
        s,
        "#set page(width: {w}cm, height: {h}cm, margin: 0pt, fill: rgb(\"{canvas}\"))",
        w = fnum(w),
        h = fnum(h),
        canvas = pal.canvas,
    );
    s.push_str("#cetz.canvas(length: 1cm, {\n  import cetz.draw: *\n");
    s.push_str(&body);
    s.push_str("})\n");

    TextExport {
        main_name: "figure.typ".to_owned(),
        main: s,
        sidecars,
    }
}

fn draw_panel(
    s: &mut String,
    idx: usize,
    panel: &Panel,
    r: &Rect,
    pal: &Palette,
    theme: Theme,
    sidecars: &mut Vec<SidecarFile>,
) {
    if r.y1 - r.y0 <= 0.0 || r.x1 - r.x0 <= 0.0 {
        return;
    }
    let is_tier = panel
        .layers
        .iter()
        .any(|l| matches!(l, Layer::Tiers { .. }));
    let is_spectro = panel
        .layers
        .iter()
        .any(|l| matches!(l, Layer::Spectrogram { .. }));

    if !is_tier {
        let _ = writeln!(
            s,
            "  rect(({x0}, {y0}), ({x1}, {y1}), fill: rgb(\"{bg}\"), stroke: none)",
            x0 = fnum(r.x0),
            y0 = fnum(r.y0),
            x1 = fnum(r.x1),
            y1 = fnum(r.y1),
            bg = pal.panel_bg,
        );
    }

    let value_ticks = nice_ticks(panel.value_axis.min, panel.value_axis.max, 4);
    if !is_spectro && !is_tier {
        for &v in &value_ticks {
            if v < panel.value_axis.min || v > panel.value_axis.max {
                continue;
            }
            let y = r.map_y(&panel.value_axis, v);
            line(s, r.x0, y, r.x1, y, pal.grid, 0.4, DashStyle::Solid);
        }
    }

    // Layers.
    for layer in &panel.layers {
        draw_layer(s, idx, panel, r, layer, pal, theme, sidecars);
    }

    // Border.
    let _ = writeln!(
        s,
        "  rect(({x0}, {y0}), ({x1}, {y1}), stroke: (paint: rgb(\"{c}\"), thickness: 0.5pt))",
        x0 = fnum(r.x0),
        y0 = fnum(r.y0),
        x1 = fnum(r.x1),
        y1 = fnum(r.y1),
        c = pal.border,
    );

    if !is_tier {
        draw_value_axis(s, &panel.value_axis, &value_ticks, r, pal);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_layer(
    s: &mut String,
    idx: usize,
    panel: &Panel,
    r: &Rect,
    layer: &Layer,
    pal: &Palette,
    theme: Theme,
    sidecars: &mut Vec<SidecarFile>,
) {
    match layer {
        Layer::Waveform {
            minmax,
            span,
            style,
        } => draw_waveform(s, minmax, span.start, span.end, panel, r, *style),
        Layer::Spectrogram {
            db,
            width,
            height,
            t,
            f,
            display,
            colormap,
        } => {
            let name = format!("spectrogram-p{idx}.png");
            let png = spectrogram_png(db, *width, *height, display, *colormap, theme);
            sidecars.push(SidecarFile::binary(&name, png));
            let ix0 = r.map_x(&panel.time_axis, t[0]);
            let ix1 = r.map_x(&panel.time_axis, t[1]);
            let iy0 = r.map_y(&panel.value_axis, f[0]);
            let iy1 = r.map_y(&panel.value_axis, f[1]);
            let _ = writeln!(
                s,
                "  content(({x0}, {y0}), ({x1}, {y1}), \
                 image(\"{name}\", width: 100%, height: 100%, fit: \"stretch\"))",
                x0 = fnum(ix0),
                y0 = fnum(iy0),
                x1 = fnum(ix1),
                y1 = fnum(iy1),
            );
        }
        Layer::PitchLine { points, style, .. } | Layer::IntensityLine { points, style } => {
            draw_line(s, points, panel, r, *style);
        }
        Layer::SpectralSlice { bins, style } => draw_line(s, bins, panel, r, *style),
        Layer::FormantSpeckle { frames, style, .. } => {
            let color = hex(style.color);
            let radius = style.radius_pt / 28.35; // points to centimeters
            for frame in frames {
                for p in &frame.points {
                    if p.frequency < panel.value_axis.min || p.frequency > panel.value_axis.max {
                        continue;
                    }
                    let x = r.map_x(&panel.time_axis, frame.time);
                    let y = r.map_y(&panel.value_axis, p.frequency);
                    let _ = writeln!(
                        s,
                        "  circle(({x}, {y}), radius: {rad}, fill: rgb(\"{color}\"), stroke: none)",
                        x = fnum(x),
                        y = fnum(y),
                        rad = fnum(radius.max(0.01)),
                    );
                }
            }
        }
        Layer::Tiers { tiers, style } => draw_tiers(s, tiers, panel, r, pal, *style),
    }
}

fn draw_waveform(
    s: &mut String,
    minmax: &[crate::model::MinMax],
    t0: f64,
    t1: f64,
    panel: &Panel,
    r: &Rect,
    style: LineStyle,
) {
    if minmax.is_empty() {
        return;
    }
    let color = hex(style.color);
    let n = minmax.len();
    let x_at = |i: usize| {
        let frac = if n > 1 {
            i as f64 / (n - 1) as f64
        } else {
            0.0
        };
        t0 + frac * (t1 - t0)
    };
    // One closed, filled polygon: the max edge left to right, then the min edge
    // back to the start.
    s.push_str("  line(");
    let mut first = true;
    for (i, m) in minmax.iter().enumerate() {
        let x = r.map_x(&panel.time_axis, x_at(i));
        let y = r.map_y(&panel.value_axis, f64::from(m.max));
        if !first {
            s.push_str(", ");
        }
        first = false;
        let _ = write!(s, "({}, {})", fnum(x), fnum(y));
    }
    for (i, m) in minmax.iter().enumerate().rev() {
        let x = r.map_x(&panel.time_axis, x_at(i));
        let y = r.map_y(&panel.value_axis, f64::from(m.min));
        let _ = write!(s, ", ({}, {})", fnum(x), fnum(y));
    }
    let _ = writeln!(s, ", close: true, fill: rgb(\"{color}\"), stroke: none)");
}

fn draw_line(s: &mut String, points: &[(f64, f64)], panel: &Panel, r: &Rect, style: LineStyle) {
    if points.is_empty() {
        return;
    }
    let color = hex(style.color);
    let dash = match style.dash {
        DashStyle::Solid => "",
        DashStyle::Dashed => ", dash: \"dashed\"",
        DashStyle::Dotted => ", dash: \"dotted\"",
    };
    for segment in gap_segments(points) {
        if segment.len() < 2 {
            if let Some(&(t, v)) = segment.first() {
                let x = r.map_x(&panel.time_axis, t);
                let y = r.map_y(&panel.value_axis, v);
                let _ = writeln!(
                    s,
                    "  circle(({x}, {y}), radius: {rad}, fill: rgb(\"{color}\"), stroke: none)",
                    x = fnum(x),
                    y = fnum(y),
                    rad = fnum((style.width_pt / 28.35).max(0.02)),
                );
            }
            continue;
        }
        s.push_str("  line(");
        for (k, &(t, v)) in segment.iter().enumerate() {
            let x = r.map_x(&panel.time_axis, t);
            let y = r.map_y(&panel.value_axis, v);
            if k > 0 {
                s.push_str(", ");
            }
            let _ = write!(s, "({}, {})", fnum(x), fnum(y));
        }
        let _ = writeln!(
            s,
            ", stroke: (paint: rgb(\"{color}\"), thickness: {w}pt{dash}))",
            w = fnum(style.width_pt),
        );
    }
}

fn draw_tiers(
    s: &mut String,
    tiers: &[crate::model::TierData],
    panel: &Panel,
    r: &Rect,
    pal: &Palette,
    style: crate::style::TierStyle,
) {
    if tiers.is_empty() {
        return;
    }
    let lane_h = (r.y1 - r.y0) / tiers.len() as f64;
    let boundary = hex(style.boundary.color);
    for (j, tier) in tiers.iter().enumerate() {
        // Lane 0 is the top lane.
        let ly1 = r.y1 - j as f64 * lane_h;
        let ly0 = ly1 - lane_h;
        let mid = (ly0 + ly1) / 2.0;
        if j > 0 {
            line(s, r.x0, ly1, r.x1, ly1, pal.border, 0.4, DashStyle::Solid);
        }
        label(s, r.x0 - 0.15, mid, &tier.name, 7.0, pal.fg, "east");
        match &tier.content {
            TierContent::Intervals(intervals) => {
                for iv in intervals {
                    let x = r.map_x(&panel.time_axis, iv.xmin);
                    line(
                        s,
                        x,
                        ly0,
                        x,
                        ly1,
                        &boundary,
                        style.boundary.width_pt,
                        style.boundary.dash,
                    );
                    if !iv.label.is_empty() {
                        let xm = r.map_x(&panel.time_axis, (iv.xmin + iv.xmax) / 2.0);
                        label(s, xm, mid, &iv.label, style.text_pt, pal.fg, "center");
                    }
                }
                if let Some(last) = intervals.last() {
                    let x = r.map_x(&panel.time_axis, last.xmax);
                    line(
                        s,
                        x,
                        ly0,
                        x,
                        ly1,
                        &boundary,
                        style.boundary.width_pt,
                        style.boundary.dash,
                    );
                }
            }
            TierContent::Points(points) => {
                for pt in points {
                    let x = r.map_x(&panel.time_axis, pt.time);
                    line(
                        s,
                        x,
                        ly0,
                        x,
                        ly1,
                        &boundary,
                        style.boundary.width_pt,
                        style.boundary.dash,
                    );
                    if !pt.label.is_empty() {
                        label(s, x, ly1, &pt.label, style.text_pt, pal.fg, "north");
                    }
                }
            }
        }
    }
}

fn draw_value_axis(s: &mut String, axis: &Axis, ticks: &[f64], r: &Rect, pal: &Palette) {
    let step = tick_step(ticks);
    for &v in ticks {
        if v < axis.min - step * 1e-6 || v > axis.max + step * 1e-6 {
            continue;
        }
        let y = r.map_y(axis, v);
        line(s, r.x0 - 0.1, y, r.x0, y, pal.fg, 0.5, DashStyle::Solid);
        label(s, r.x0 - 0.18, y, &fmt_tick(v, step), 7.0, pal.fg, "east");
    }
    if let Some(title) = axis_title(axis) {
        let cy = (r.y0 + r.y1) / 2.0;
        let _ = writeln!(
            s,
            "  content(({x}, {y}), text(size: 8pt, fill: rgb(\"{c}\"), \"{t}\"), angle: 90deg, anchor: \"center\")",
            x = fnum(r.x0 - 1.0),
            y = fnum(cy),
            c = pal.fg,
            t = typst_str(&title),
        );
    }
}

fn draw_time_axis(s: &mut String, axis: &Axis, x0: f64, x1: f64, y_bottom: f64, pal: &Palette) {
    let ticks = nice_ticks(axis.min, axis.max, 6);
    let step = tick_step(&ticks);
    let width = x1 - x0;
    for &t in &ticks {
        if t < axis.min - step * 1e-6 || t > axis.max + step * 1e-6 {
            continue;
        }
        let span = axis.max - axis.min;
        let x = if span.abs() < f64::EPSILON {
            x0
        } else {
            x0 + (t - axis.min) / span * width
        };
        line(
            s,
            x,
            y_bottom,
            x,
            y_bottom - 0.1,
            pal.fg,
            0.5,
            DashStyle::Solid,
        );
        label(
            s,
            x,
            y_bottom - 0.28,
            &fmt_tick(t, step),
            7.0,
            pal.fg,
            "north",
        );
    }
    if let Some(title) = axis_title(axis) {
        label(
            s,
            (x0 + x1) / 2.0,
            y_bottom - 0.62,
            &title,
            9.0,
            pal.fg,
            "north",
        );
    }
}

fn draw_caption(s: &mut String, fig: &Figure, x: f64, caption_h: f64, line_h: f64, pal: &Palette) {
    let mut y = (caption_h - 0.15).max(0.1);
    for src in &fig.caption_meta.sources {
        let mut line_txt = format!("{:?}", src.layer);
        if !src.params.is_empty() {
            let params: Vec<String> = src.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            line_txt.push_str(" — ");
            line_txt.push_str(&params.join(", "));
        }
        if src.smoothed == Some(true) {
            line_txt.push_str(" (smoothed track)");
        }
        label(s, x, y, &line_txt, 7.0, pal.fg, "west");
        y -= line_h;
    }
}

// ---- CeTZ primitive writers ------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn line(s: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, color: &str, w: f64, dash: DashStyle) {
    let dash = match dash {
        DashStyle::Solid => String::new(),
        DashStyle::Dashed => ", dash: \"dashed\"".to_owned(),
        DashStyle::Dotted => ", dash: \"dotted\"".to_owned(),
    };
    let _ = writeln!(
        s,
        "  line(({x0}, {y0}), ({x1}, {y1}), stroke: (paint: rgb(\"{color}\"), thickness: {w}pt{dash}))",
        x0 = fnum(x0),
        y0 = fnum(y0),
        x1 = fnum(x1),
        y1 = fnum(y1),
        w = fnum(w.max(0.1)),
    );
}

fn label(s: &mut String, x: f64, y: f64, text: &str, size_pt: f64, color: &str, anchor: &str) {
    let _ = writeln!(
        s,
        "  content(({x}, {y}), text(size: {sz}pt, fill: rgb(\"{c}\"), \"{t}\"), anchor: \"{a}\")",
        x = fnum(x),
        y = fnum(y),
        sz = fnum(size_pt),
        c = color,
        t = typst_str(text),
        a = anchor,
    );
}

/// Escape a label for a Typst string literal: backslash and double quote, with
/// control characters flattened to spaces.
fn typst_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// Split a point series where the time gap jumps well past the typical step.
fn gap_segments(points: &[(f64, f64)]) -> Vec<&[(f64, f64)]> {
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

    let mut out = Vec::new();
    let mut start = 0;
    for i in 1..points.len() {
        if points[i].0 - points[i - 1].0 > break_gap {
            out.push(&points[start..i]);
            start = i;
        }
    }
    out.push(&points[start..]);
    out
}
