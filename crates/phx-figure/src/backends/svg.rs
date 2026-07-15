//! SVG backend: the scene-graph source of truth.
//!
//! [`to_svg`] draws a [`Figure`] as a single self-contained SVG string. Every
//! other raster or vector backend derives from this output rather than
//! re-running layout: PNG rasterizes the SVG, PDF converts it. The renderer
//! owns the layout math (panel stacking, axis placement, tick selection) so
//! there is exactly one place a figure turns into geometry.
//!
//! The spectrogram layer is the only raster content. Its raw decibels are
//! colorized through [`phx_render::colorize`] against the figure theme at
//! export, encoded as a PNG, and embedded as a base64 `<image>` positioned on
//! the panel's pixel grid; axes, tracks, tiers, and text stay vector.
//!
//! Output is deterministic. The renderer emits no timestamps and no random
//! identifiers — clip-path ids are the panel index — so rendering the same
//! figure twice yields byte-identical strings.

use std::fmt::Write as _;

use base64::Engine as _;
use phx_render::colorize;

use crate::model::{Axis, AxisScale, CaptionMeta, Figure, Layer, Panel};
use crate::style::{DashStyle, LineStyle, RgbaColor, SpeckleStyle, TierStyle};

/// SVG user-space resolution: one user unit is one pixel at this DPI, so a
/// physical size resolves to a concrete coordinate box for the layout math.
const SVG_DPI: f64 = 96.0;
/// Points-to-pixels factor for stroke widths and text sizes carried in points.
const PT_TO_PX: f64 = SVG_DPI / 72.0;

/// The font stack written on every text element. The bundled DejaVu Sans face
/// (used by the PNG and PDF backends) is named first; a generic fallback keeps
/// a browser or viewer without it legible.
const FONT_STACK: &str = "'DejaVu Sans', 'Helvetica Neue', Arial, sans-serif";

/// An opaque RGB triple in the rendering color space.
type Rgb = (u8, u8, u8);

/// Theme-resolved surface and ink colors.
struct Palette {
    /// The theme these colors resolve, forwarded to spectrogram colorization.
    theme: phx_render::Theme,
    /// Figure background outside the panels.
    canvas: Rgb,
    /// Plot background inside a non-spectrogram panel.
    panel_bg: Rgb,
    /// Default foreground for axes, ticks, and text.
    fg: Rgb,
    /// Axis line color.
    axis: Rgb,
    /// Interior grid line color.
    grid: Rgb,
    /// Panel border color.
    border: Rgb,
    /// Relative luminance of `panel_bg`, cached for contrast adaptation.
    panel_lum: f64,
}

impl Palette {
    fn for_theme(theme: phx_render::Theme) -> Self {
        let (canvas, panel_bg, fg, axis, grid, border) = match theme {
            phx_render::Theme::Dark => (
                (0x14, 0x16, 0x1a),
                (0x1b, 0x1e, 0x24),
                (0xd8, 0xdd, 0xe3),
                (0x8a, 0x92, 0x9c),
                (0x33, 0x39, 0x42),
                (0x2c, 0x31, 0x3a),
            ),
            phx_render::Theme::Light => (
                (0xff, 0xff, 0xff),
                (0xff, 0xff, 0xff),
                (0x1c, 0x1c, 0x1c),
                (0x44, 0x44, 0x44),
                (0xe2, 0xe2, 0xe2),
                (0xbb, 0xbb, 0xbb),
            ),
        };
        Self {
            theme,
            canvas,
            panel_bg,
            fg,
            axis,
            grid,
            border,
            panel_lum: relative_luminance(panel_bg),
        }
    }

    /// Adapt a baked layer color to the panel background: a color whose
    /// luminance sits too close to the background (a near-black stroke on a
    /// dark panel) is replaced by the theme foreground so it stays legible;
    /// a distinctly-hued color passes through unchanged.
    fn legible(&self, c: RgbaColor) -> Rgb {
        let color = (c.r, c.g, c.b);
        if (relative_luminance(color) - self.panel_lum).abs() < 0.12 {
            self.fg
        } else {
            color
        }
    }
}

/// WCAG relative luminance of an sRGB triple, on `[0, 1]`.
fn relative_luminance((r, g, b): Rgb) -> f64 {
    0.2126 * f64::from(r) / 255.0 + 0.7152 * f64::from(g) / 255.0 + 0.0722 * f64::from(b) / 255.0
}

/// Renders `fig` to a deterministic, self-contained SVG document.
///
/// The figure's physical size sets the SVG's width and height at [`SVG_DPI`];
/// the `viewBox` matches, so the document scales without re-layout. The figure
/// is not validated here — a caller wanting the model invariants checked calls
/// [`Figure::validate`] first — but a spectrogram whose `db` length disagrees
/// with `width * height` is skipped rather than panicking.
#[must_use]
pub fn to_svg(fig: &Figure) -> String {
    let (w_px, h_px) = fig.size.px_at(SVG_DPI);
    let w = f64::from(w_px.max(1));
    let h = f64::from(h_px.max(1));
    let pal = Palette::for_theme(fig.theme);

    let mut s = String::with_capacity(64 * 1024);
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
         width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">",
        w = fmt(w),
        h = fmt(h),
    );
    rect(&mut s, 0.0, 0.0, w, h, Some(pal.canvas), None, 0.0);

    // Vertical budget: panels stack between the top pad and the shared time
    // axis; the caption sits below the time axis.
    let pad_top = 10.0;
    let pad_side = 10.0;
    let left_gutter = 60.0;
    let right_gutter = 60.0;
    let time_axis_h = 42.0;
    let caption_px = 9.0;
    let caption_lh = caption_px + 3.0;
    let caption_lines = fig.caption_meta.sources.len();
    let caption_h = if caption_lines == 0 {
        6.0
    } else {
        caption_lines as f64 * caption_lh + 10.0
    };
    let panel_gap = 12.0;

    let x0 = pad_side + left_gutter;
    let x1 = w - pad_side - right_gutter;
    let content_top = pad_top;
    let content_bottom = h - caption_h - time_axis_h;

    let n = fig.panels.len();
    let share_sum: f64 = fig.panels.iter().map(|p| p.height_share.max(0.0)).sum();
    let share_sum = if share_sum > 0.0 { share_sum } else { 1.0 };
    let usable_h =
        (content_bottom - content_top - panel_gap * (n.saturating_sub(1)) as f64).max(0.0);

    // First pass over the panels: place each plot rectangle.
    let mut rects: Vec<PlotRect> = Vec::with_capacity(n);
    let mut y = content_top;
    for panel in &fig.panels {
        let ph = usable_h * panel.height_share.max(0.0) / share_sum;
        rects.push(PlotRect {
            x0,
            x1,
            y0: y,
            y1: y + ph,
        });
        y += ph + panel_gap;
    }

    for (idx, (panel, r)) in fig.panels.iter().zip(&rects).enumerate() {
        draw_panel(&mut s, idx, panel, r, &pal);
    }

    // The time axis is shared: draw it once under the bottom panel, spanning
    // the full plotted width so every panel reads against the same ticks.
    if let (Some(last), Some(bottom_panel)) = (rects.last(), fig.panels.last()) {
        draw_time_axis(&mut s, &bottom_panel.time_axis, x0, x1, last.y1, &pal);
    }

    draw_caption(
        &mut s,
        &fig.caption_meta,
        pad_side,
        content_bottom + time_axis_h,
        w - 2.0 * pad_side,
        caption_px,
        caption_lh,
        &pal,
    );

    s.push_str("</svg>");
    s
}

/// A panel's plotting rectangle in SVG user space.
struct PlotRect {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl PlotRect {
    fn width(&self) -> f64 {
        self.x1 - self.x0
    }
    fn height(&self) -> f64 {
        self.y1 - self.y0
    }
    /// Map a time value through `axis` to an x coordinate.
    fn map_x(&self, axis: &Axis, t: f64) -> f64 {
        self.x0 + axis_norm(axis, t) * self.width()
    }
    /// Map a value through `axis` to a y coordinate (inverted: max at top).
    fn map_y(&self, axis: &Axis, v: f64) -> f64 {
        self.y1 - axis_norm(axis, v) * self.height()
    }
}

/// Normalized `[0, 1]` position of `v` within `axis`, honoring log scale and
/// clamped so out-of-range data does not escape the plot rectangle.
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
            let (lo, hi, x) = (
                axis.min.max(f64::MIN_POSITIVE).log10(),
                axis.max.max(f64::MIN_POSITIVE).log10(),
                v.max(f64::MIN_POSITIVE).log10(),
            );
            if (hi - lo).abs() < f64::EPSILON {
                0.0
            } else {
                (x - lo) / (hi - lo)
            }
        }
    };
    t.clamp(0.0, 1.0)
}

/// Whether a panel is dominated by a spectrogram, which suppresses interior
/// grid lines that would fight the raster.
fn is_spectrogram_panel(panel: &Panel) -> bool {
    panel
        .layers
        .iter()
        .any(|l| matches!(l, Layer::Spectrogram { .. }))
}

/// Whether a panel carries a pitch line, whose Hz scale is drawn on the right.
fn has_pitch(panel: &Panel) -> bool {
    panel
        .layers
        .iter()
        .any(|l| matches!(l, Layer::PitchLine { .. }))
}

fn is_tier_panel(panel: &Panel) -> bool {
    panel
        .layers
        .iter()
        .any(|l| matches!(l, Layer::Tiers { .. }))
}

fn draw_panel(s: &mut String, idx: usize, panel: &Panel, r: &PlotRect, pal: &Palette) {
    if r.height() <= 0.0 || r.width() <= 0.0 {
        return;
    }
    let tier_panel = is_tier_panel(panel);
    let spectro = is_spectrogram_panel(panel);

    // Plot background. Tier panels share the canvas; data panels get their own
    // surface so ink reads against a controlled background.
    if !tier_panel {
        rect(
            s,
            r.x0,
            r.y0,
            r.width(),
            r.height(),
            Some(pal.panel_bg),
            None,
            0.0,
        );
    }

    // Interior grid for data panels that are not spectrograms: value grid plus
    // time grid, both suppressed over a spectrogram so the lines do not cut
    // through the raster.
    let value_ticks = nice_ticks(panel.value_axis.min, panel.value_axis.max, 4);
    if !spectro && !tier_panel {
        for &v in &value_ticks {
            let y = r.map_y(&panel.value_axis, v);
            line(s, r.x0, y, r.x1, y, pal.grid, 0.6, DashStyle::Solid);
        }
        for &t in &nice_ticks(panel.time_axis.min, panel.time_axis.max, 6) {
            if t < panel.time_axis.min || t > panel.time_axis.max {
                continue;
            }
            let x = r.map_x(&panel.time_axis, t);
            line(s, x, r.y0, x, r.y1, pal.grid, 0.5, DashStyle::Solid);
        }
    }

    // Layers, clipped to the plot rectangle.
    let clip = format!("phx-clip-{idx}");
    let _ = write!(
        s,
        "<clipPath id=\"{clip}\"><rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/></clipPath>",
        x = fmt(r.x0),
        y = fmt(r.y0),
        w = fmt(r.width()),
        h = fmt(r.height()),
    );
    let _ = write!(s, "<g clip-path=\"url(#{clip})\">");
    for layer in &panel.layers {
        draw_layer(s, layer, panel, r, pal);
    }
    s.push_str("</g>");

    // Border.
    rect(
        s,
        r.x0,
        r.y0,
        r.width(),
        r.height(),
        None,
        Some(pal.border),
        1.0,
    );

    // Value axis: right side for a pitch panel, left otherwise; tier panels
    // carry no value axis.
    if !tier_panel {
        let side = if has_pitch(panel) {
            AxisSide::Right
        } else {
            AxisSide::Left
        };
        draw_value_axis(s, &panel.value_axis, &value_ticks, r, side, pal);
    }
}

/// Which side of the plot a value axis is drawn on.
#[derive(Clone, Copy, PartialEq)]
enum AxisSide {
    Left,
    Right,
}

fn draw_value_axis(
    s: &mut String,
    axis: &Axis,
    ticks: &[f64],
    r: &PlotRect,
    side: AxisSide,
    pal: &Palette,
) {
    let step = tick_step(ticks);
    let x_line = match side {
        AxisSide::Left => r.x0,
        AxisSide::Right => r.x1,
    };
    for &v in ticks {
        if v < axis.min - step * 1e-6 || v > axis.max + step * 1e-6 {
            continue;
        }
        let y = r.map_y(axis, v);
        let (tx0, tx1, anchor, lx) = match side {
            AxisSide::Left => (x_line - 4.0, x_line, "end", x_line - 6.0),
            AxisSide::Right => (x_line, x_line + 4.0, "start", x_line + 6.0),
        };
        line(s, tx0, y, tx1, y, pal.axis, 1.0, DashStyle::Solid);
        text(s, lx, y + 3.0, &fmt_tick(v, step), 9.0, pal.fg, anchor);
    }
    // Axis title, rotated along the axis.
    if let Some(title) = axis_title(axis) {
        let cy = (r.y0 + r.y1) / 2.0;
        let (tx, rot) = match side {
            AxisSide::Left => (r.x0 - 44.0, -90.0),
            AxisSide::Right => (r.x1 + 46.0, 90.0),
        };
        let _ = write!(
            s,
            "<text x=\"{x}\" y=\"{y}\" transform=\"rotate({rot} {x} {y})\" \
             font-family=\"{font}\" font-size=\"10\" fill=\"{fill}\" \
             text-anchor=\"middle\">{t}</text>",
            x = fmt(tx),
            y = fmt(cy),
            rot = fmt(rot),
            font = FONT_STACK,
            fill = css(pal.fg),
            t = escape(&title),
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
            y_bottom + 4.0,
            pal.axis,
            1.0,
            DashStyle::Solid,
        );
        text(
            s,
            x,
            y_bottom + 16.0,
            &fmt_tick(t, step),
            9.0,
            pal.fg,
            "middle",
        );
    }
    if let Some(title) = axis_title(axis) {
        text_px(
            s,
            (x0 + x1) / 2.0,
            y_bottom + 32.0,
            &title,
            12.0,
            pal.fg,
            "middle",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_caption(
    s: &mut String,
    meta: &CaptionMeta,
    x: f64,
    y_top: f64,
    max_width: f64,
    size_px: f64,
    line_h: f64,
    pal: &Palette,
) {
    // A conservative average glyph advance keeps the label inside the figure
    // without shaping: overrun is trimmed to an ellipsis.
    let max_chars = ((max_width / (size_px * 0.55)).floor() as usize).max(8);
    let mut y = y_top + size_px;
    for src in &meta.sources {
        let mut line_txt = format!("{:?}", src.layer);
        if !src.params.is_empty() {
            let params: Vec<String> = src.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            line_txt.push_str(" — ");
            line_txt.push_str(&params.join(", "));
        }
        if src.smoothed == Some(true) {
            line_txt.push_str(" (smoothed track)");
        }
        if line_txt.chars().count() > max_chars {
            line_txt = line_txt.chars().take(max_chars - 1).collect::<String>() + "…";
        }
        text_px(s, x, y, &line_txt, size_px, pal.fg, "start");
        y += line_h;
    }
}

fn draw_layer(s: &mut String, layer: &Layer, panel: &Panel, r: &PlotRect, pal: &Palette) {
    match layer {
        Layer::Waveform {
            minmax,
            span,
            style,
        } => {
            draw_waveform(s, minmax, span.start, span.end, panel, r, pal, *style);
        }
        Layer::Spectrogram {
            db,
            width,
            height,
            t,
            f,
            display,
            colormap,
        } => {
            draw_spectrogram(
                s, db, *width, *height, *t, *f, display, *colormap, panel, r, pal,
            );
        }
        Layer::PitchLine { points, style, .. } => {
            draw_line_series(s, points, panel, r, pal, *style);
        }
        Layer::IntensityLine { points, style } => {
            draw_line_series(s, points, panel, r, pal, *style);
        }
        Layer::SpectralSlice { bins, style } => {
            // A spectral slice plots frequency (x) against dB (y): its own
            // axes already carry those ranges.
            draw_line_series(s, bins, panel, r, pal, *style);
        }
        Layer::FormantSpeckle { frames, style, .. } => {
            draw_formants(s, frames, panel, r, *style);
        }
        Layer::Tiers { tiers, style } => {
            draw_tiers(s, tiers, panel, r, pal, *style);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_waveform(
    s: &mut String,
    minmax: &[crate::model::MinMax],
    t_start: f64,
    t_end: f64,
    panel: &Panel,
    r: &PlotRect,
    pal: &Palette,
    style: LineStyle,
) {
    if minmax.is_empty() {
        return;
    }
    let color = pal.legible(style.color);
    let n = minmax.len();
    // A filled envelope: the max edge left-to-right, then the min edge back.
    let mut d = String::with_capacity(n * 24);
    for (i, m) in minmax.iter().enumerate() {
        let frac = if n > 1 {
            i as f64 / (n - 1) as f64
        } else {
            0.0
        };
        let t = t_start + frac * (t_end - t_start);
        let x = r.map_x(&panel.time_axis, t);
        let y = r.map_y(&panel.value_axis, f64::from(m.max));
        d.push_str(if i == 0 { "M" } else { "L" });
        let _ = write!(d, "{},{} ", fmt(x), fmt(y));
    }
    for (i, m) in minmax.iter().enumerate().rev() {
        let frac = if n > 1 {
            i as f64 / (n - 1) as f64
        } else {
            0.0
        };
        let t = t_start + frac * (t_end - t_start);
        let x = r.map_x(&panel.time_axis, t);
        let y = r.map_y(&panel.value_axis, f64::from(m.min));
        let _ = write!(d, "L{},{} ", fmt(x), fmt(y));
    }
    d.push('Z');
    let _ = write!(
        s,
        "<path d=\"{d}\" fill=\"{fill}\" fill-opacity=\"0.85\" stroke=\"{fill}\" \
         stroke-width=\"{sw}\" stroke-linejoin=\"round\"/>",
        fill = css(color),
        sw = fmt(0.6 * PT_TO_PX),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_spectrogram(
    s: &mut String,
    db: &[f32],
    width: u32,
    height: u32,
    t: [f64; 2],
    f: [f64; 2],
    display: &phx_render::DisplayMapping,
    colormap: phx_render::Colormap,
    panel: &Panel,
    r: &PlotRect,
    pal: &Palette,
) {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || db.len() != w * h {
        return;
    }
    // Colorize raw dB against the figure theme, then flip vertically: tile row
    // 0 is the lowest frequency, but PNG row 0 is the image top, which is the
    // highest frequency in the plot.
    let rgba = colorize_flipped(db, width, height, display, colormap, pal.theme);
    let png = encode_png(&rgba, width, height);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);

    // Place the image on the panel's pixel grid: the tile's time/frequency
    // extent mapped through the panel axes gives the exact rectangle, so
    // column and row edges land on their axis positions.
    let ix0 = r.map_x(&panel.time_axis, t[0]);
    let ix1 = r.map_x(&panel.time_axis, t[1]);
    let iy_top = r.map_y(&panel.value_axis, f[1]);
    let iy_bot = r.map_y(&panel.value_axis, f[0]);
    let iw = (ix1 - ix0).max(0.0);
    let ih = (iy_bot - iy_top).max(0.0);
    let _ = write!(
        s,
        "<image x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" \
         preserveAspectRatio=\"none\" image-rendering=\"auto\" \
         xlink:href=\"data:image/png;base64,{b64}\"/>",
        x = fmt(ix0),
        y = fmt(iy_top),
        w = fmt(iw),
        h = fmt(ih),
    );
}

fn draw_line_series(
    s: &mut String,
    points: &[(f64, f64)],
    panel: &Panel,
    r: &PlotRect,
    pal: &Palette,
    style: LineStyle,
) {
    if points.is_empty() {
        return;
    }
    let color = pal.legible(style.color);
    let sw = style.width_pt * PT_TO_PX;
    // Break the path where the time gap between successive points jumps well
    // past the typical step, so an unvoiced hole is not bridged by a line.
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

    let mut d = String::with_capacity(points.len() * 16);
    // A segment that never gains a second point draws as nothing, so track the
    // start of the current segment and emit a dot for it if it stays alone.
    let mut singleton_dots: Vec<(f64, f64)> = Vec::new();
    let mut seg_start: Option<(f64, f64)> = None;
    let mut prev_t = f64::NAN;
    for (i, &(t, v)) in points.iter().enumerate() {
        let x = r.map_x(&panel.time_axis, t);
        let y = r.map_y(&panel.value_axis, v);
        let new_seg = i == 0 || (t - prev_t) > break_gap;
        if new_seg {
            if let Some(start) = seg_start.take() {
                singleton_dots.push(start);
            }
            d.push('M');
            seg_start = Some((x, y));
        } else {
            d.push('L');
            // The segment has a real second point; it is a line, not a dot.
            seg_start = None;
        }
        let _ = write!(d, "{},{} ", fmt(x), fmt(y));
        prev_t = t;
    }
    if let Some(start) = seg_start.take() {
        singleton_dots.push(start);
    }
    let _ = write!(
        s,
        "<path d=\"{d}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{sw}\" \
         stroke-linejoin=\"round\" stroke-linecap=\"round\"{dash}/>",
        stroke = css(color),
        sw = fmt(sw.max(0.4)),
        dash = dash_attr(style.dash, sw),
    );
    // Isolated single-point segments would be invisible as a zero-length path;
    // render them as small dots.
    for (x, y) in singleton_dots {
        circle(s, x, y, (sw * 0.8).max(0.8), color, 1.0);
    }
}

fn draw_formants(
    s: &mut String,
    frames: &[crate::model::SpeckleFrame],
    panel: &Panel,
    r: &PlotRect,
    style: SpeckleStyle,
) {
    let base = style.radius_pt * PT_TO_PX;
    let color = (style.color.r, style.color.g, style.color.b);
    let alpha = f64::from(style.color.a) / 255.0;
    for frame in frames {
        let x = r.map_x(&panel.time_axis, frame.time);
        for p in &frame.points {
            if p.frequency < panel.value_axis.min || p.frequency > panel.value_axis.max {
                continue;
            }
            let y = r.map_y(&panel.value_axis, p.frequency);
            // Wider bandwidth means a less certain, more diffuse candidate:
            // grow the marker with bandwidth off a small floor.
            let radius = (base * (0.6 + p.bandwidth / 250.0)).clamp(base * 0.6, base * 5.0);
            circle(s, x, y, radius, color, alpha);
        }
    }
}

fn draw_tiers(
    s: &mut String,
    tiers: &[crate::model::TierData],
    panel: &Panel,
    r: &PlotRect,
    pal: &Palette,
    style: TierStyle,
) {
    if tiers.is_empty() {
        return;
    }
    let lane_h = r.height() / tiers.len() as f64;
    let boundary = pal.legible(style.boundary.color);
    let bw = style.boundary.width_pt * PT_TO_PX;
    let text_color = pal.legible(style.text_color);
    for (j, tier) in tiers.iter().enumerate() {
        let ly0 = r.y0 + j as f64 * lane_h;
        let ly1 = ly0 + lane_h;
        let mid = (ly0 + ly1) / 2.0;
        if j > 0 {
            line(s, r.x0, ly0, r.x1, ly0, pal.border, 0.7, DashStyle::Solid);
        }
        // Tier name in the left gutter.
        text(s, r.x0 - 6.0, mid + 3.0, &tier.name, 8.0, pal.fg, "end");
        match &tier.content {
            crate::model::TierContent::Intervals(intervals) => {
                for iv in intervals {
                    let xa = r.map_x(&panel.time_axis, iv.xmin);
                    let xb = r.map_x(&panel.time_axis, iv.xmax);
                    line(
                        s,
                        xa,
                        ly0,
                        xa,
                        ly1,
                        boundary,
                        bw.max(0.4),
                        style.boundary.dash,
                    );
                    if !iv.label.is_empty() {
                        text(
                            s,
                            (xa + xb) / 2.0,
                            mid + 3.0,
                            &iv.label,
                            style.text_pt,
                            text_color,
                            "middle",
                        );
                    }
                }
                if let Some(last) = intervals.last() {
                    let xb = r.map_x(&panel.time_axis, last.xmax);
                    line(
                        s,
                        xb,
                        ly0,
                        xb,
                        ly1,
                        boundary,
                        bw.max(0.4),
                        style.boundary.dash,
                    );
                }
            }
            crate::model::TierContent::Points(pts) => {
                for pt in pts {
                    let x = r.map_x(&panel.time_axis, pt.time);
                    line(
                        s,
                        x,
                        ly0,
                        x,
                        ly1,
                        boundary,
                        bw.max(0.4),
                        style.boundary.dash,
                    );
                    if !pt.label.is_empty() {
                        text(
                            s,
                            x,
                            ly0 + style.text_pt * PT_TO_PX,
                            &pt.label,
                            style.text_pt,
                            text_color,
                            "middle",
                        );
                    }
                }
            }
        }
    }
}

// ---- colorization + PNG encoding --------------------------------------------

/// Colorize a raw-dB tile against `theme` and flip rows so PNG row 0 is the
/// highest frequency (image top): tile row 0 is the lowest frequency, and the
/// image top edge sits at the highest frequency in the plot.
fn colorize_flipped(
    db: &[f32],
    width: u32,
    height: u32,
    display: &phx_render::DisplayMapping,
    colormap: phx_render::Colormap,
    theme: phx_render::Theme,
) -> Vec<u8> {
    let flat = colorize(db, width, height, display, colormap, theme);
    let (w, h) = (width as usize, height as usize);
    let stride = w * 4;
    let mut out = vec![0u8; flat.len()];
    for row in 0..h {
        let src = (h - 1 - row) * stride;
        let dst = row * stride;
        out[dst..dst + stride].copy_from_slice(&flat[src..src + stride]);
    }
    out
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        // Fixed, non-adaptive settings keep encoding deterministic run to run.
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(rgba).expect("png data");
    }
    out
}

// ---- primitive SVG writers --------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn rect(
    s: &mut String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: Option<Rgb>,
    stroke: Option<Rgb>,
    sw: f64,
) {
    let _ = write!(
        s,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
        fmt(x),
        fmt(y),
        fmt(w),
        fmt(h)
    );
    match fill {
        Some(c) => {
            let _ = write!(s, " fill=\"{}\"", css(c));
        }
        None => s.push_str(" fill=\"none\""),
    }
    if let Some(c) = stroke {
        let _ = write!(s, " stroke=\"{}\" stroke-width=\"{}\"", css(c), fmt(sw));
    }
    s.push_str("/>");
}

#[allow(clippy::too_many_arguments)]
fn line(s: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, color: Rgb, w: f64, dash: DashStyle) {
    let _ = write!(
        s,
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"{}/>",
        fmt(x0),
        fmt(y0),
        fmt(x1),
        fmt(y1),
        css(color),
        fmt(w),
        dash_attr(dash, w),
    );
}

fn circle(s: &mut String, cx: f64, cy: f64, r: f64, color: Rgb, alpha: f64) {
    let _ = write!(
        s,
        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"{}/>",
        fmt(cx),
        fmt(cy),
        fmt(r),
        css(color),
        if alpha < 1.0 {
            format!(" fill-opacity=\"{}\"", fmt(alpha))
        } else {
            String::new()
        },
    );
}

/// Draw text whose size is given in typographic points (converted to px).
fn text(s: &mut String, x: f64, y: f64, content: &str, size_pt: f64, color: Rgb, anchor: &str) {
    text_px(s, x, y, content, size_pt * PT_TO_PX, color, anchor);
}

/// Draw text whose size is given directly in pixels.
fn text_px(s: &mut String, x: f64, y: f64, content: &str, size_px: f64, color: Rgb, anchor: &str) {
    let _ = write!(
        s,
        "<text x=\"{x}\" y=\"{y}\" font-family=\"{font}\" font-size=\"{size}\" \
         fill=\"{fill}\" text-anchor=\"{anchor}\">{t}</text>",
        x = fmt(x),
        y = fmt(y),
        font = FONT_STACK,
        size = fmt(size_px),
        fill = css(color),
        anchor = anchor,
        t = escape(content),
    );
}

fn dash_attr(dash: DashStyle, w: f64) -> String {
    match dash {
        DashStyle::Solid => String::new(),
        DashStyle::Dashed => format!(" stroke-dasharray=\"{},{}\"", fmt(w * 4.0), fmt(w * 3.0)),
        DashStyle::Dotted => format!(" stroke-dasharray=\"{},{}\"", fmt(w), fmt(w * 2.0)),
    }
}

fn css((r, g, b): Rgb) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Format a coordinate with two decimals, collapsing negative zero.
fn fmt(v: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    let mut out = format!("{v:.2}");
    // Trim a trailing ".00" and dangling zero to keep the document compact and
    // still deterministic.
    if out.contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
    out
}

/// Format a tick label at a precision matched to the tick step.
fn fmt_tick(v: f64, step: f64) -> String {
    let v = if v.abs() < step * 1e-9 { 0.0 } else { v };
    let dec = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    };
    format!("{v:.dec$}")
}

fn axis_title(axis: &Axis) -> Option<String> {
    match (&axis.label, &axis.unit) {
        (Some(l), Some(u)) => Some(format!("{l} ({u})")),
        (Some(l), None) => Some(l.clone()),
        (None, Some(u)) => Some(u.clone()),
        (None, None) => None,
    }
}

fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Deterministic "nice" tick positions across `[min, max]`, roughly `target`
/// of them, snapped to 1/2/5 × a power of ten.
fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    if !min.is_finite() || !max.is_finite() || max <= min {
        return vec![min];
    }
    let range = max - min;
    let raw = range / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = if norm < 1.5 {
        1.0
    } else if norm < 3.0 {
        2.0
    } else if norm < 7.0 {
        5.0
    } else {
        10.0
    } * mag;
    let start = (min / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut x = start;
    let mut guard = 0;
    while x <= max + step * 1e-6 && guard < 1000 {
        ticks.push(x);
        x += step;
        guard += 1;
    }
    if ticks.is_empty() {
        ticks.push(min);
    }
    ticks
}

fn tick_step(ticks: &[f64]) -> f64 {
    if ticks.len() >= 2 {
        (ticks[1] - ticks[0]).abs()
    } else {
        1.0
    }
}
