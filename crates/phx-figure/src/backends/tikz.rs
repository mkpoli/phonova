//! PGFPlots/TikZ backend.
//!
//! [`to_tikz`] emits a standalone LaTeX document that draws the figure with
//! PGFPlots. Panels stack in a `groupplot` sharing one time axis; tracks and
//! tiers are inline coordinate data, and each spectrogram is a sidecar PNG
//! placed with `\addplot graphics` across its time/frequency extent.
//!
//! The colorized spectrogram is the only binary sidecar. Everything else is
//! text, so the document reads and edits as ordinary PGFPlots source. Output is
//! deterministic: no timestamps, no generated identifiers beyond the panel
//! index.

use std::fmt::Write as _;

use crate::backends::bundle::{SidecarFile, TextExport, axis_title, fnum, spectrogram_png};
use crate::model::{Axis, Figure, Layer, Panel, TierContent};
use crate::style::{DashStyle, LineStyle};

/// The largest number of vertices kept per waveform envelope edge. A longer
/// envelope is merged down to this many columns (min of mins, max of maxes per
/// merged column) so the filled band stays light enough for TeX without losing
/// its extremes.
const MAX_WAVEFORM_COLUMNS: usize = 500;

/// Renders `fig` to a standalone PGFPlots document plus spectrogram sidecars.
#[must_use]
pub fn to_tikz(fig: &Figure) -> TextExport {
    let mut sidecars = Vec::new();
    let mut s = String::with_capacity(16 * 1024);

    s.push_str(
        "% Phonetics figure exported as PGFPlots source.\n\
         % Panels stack top to bottom over a shared time axis (seconds).\n\
         % Value axes carry each panel's unit; a spectrogram panel places its\n\
         % colorized image sidecar across its time and frequency extent.\n\
         \\documentclass[border=2pt]{standalone}\n\
         \\usepackage{pgfplots}\n\
         \\usepackage{graphicx}\n\
         \\pgfplotsset{compat=1.18}\n\
         \\usepgfplotslibrary{groupplots}\n\
         \\begin{document}\n\
         \\begin{tikzpicture}\n",
    );

    let n = fig.panels.len();
    let width_cm = fig.size.width_in() * 2.54;
    let usable_cm = (fig.size.height_in() * 2.54 - 1.5).max(1.0);
    let share_sum: f64 = fig.panels.iter().map(|p| p.height_share.max(0.0)).sum();
    let share_sum = if share_sum > 0.0 { share_sum } else { 1.0 };

    let _ = write!(
        s,
        "\\begin{{groupplot}}[\n  group style={{group size=1 by {n}, \
         vertical sep=8mm, x descriptions at=edge bottom}},\n  \
         width={w}cm, scale only axis, enlarge x limits=false,\n  \
         tick label style={{font=\\footnotesize}}, label style={{font=\\small}}]\n",
        w = fnum((width_cm - 3.0).max(2.0)),
    );

    let time_title = axis_title(&last_time_axis(fig)).unwrap_or_else(|| "Time (s)".to_owned());

    for (idx, panel) in fig.panels.iter().enumerate() {
        let ph = (usable_cm * panel.height_share.max(0.0) / share_sum).max(0.4);
        write_panel(
            &mut s,
            idx,
            panel,
            ph,
            &time_title,
            fig.theme,
            &mut sidecars,
        );
    }

    s.push_str("\\end{groupplot}\n\\end{tikzpicture}\n\\end{document}\n");

    TextExport {
        main_name: "figure.tex".to_owned(),
        main: s,
        sidecars,
    }
}

fn last_time_axis(fig: &Figure) -> Axis {
    fig.panels.last().map_or_else(
        || Axis::linear(0.0, 1.0, Some("Time"), Some("s")),
        |p| p.time_axis.clone(),
    )
}

fn write_panel(
    s: &mut String,
    idx: usize,
    panel: &Panel,
    height_cm: f64,
    time_title: &str,
    theme: phx_render::Theme,
    sidecars: &mut Vec<SidecarFile>,
) {
    let ta = &panel.time_axis;
    let va = &panel.value_axis;
    let is_tier = panel
        .layers
        .iter()
        .any(|l| matches!(l, Layer::Tiers { .. }));

    let _ = write!(
        s,
        "\\nextgroupplot[height={h}cm, xmin={xa}, xmax={xb}, ymin={ya}, ymax={yb},\n  \
         xlabel={{{xl}}}",
        h = fnum(height_cm),
        xa = fnum(ta.min),
        xb = fnum(ta.max),
        ya = fnum(va.min),
        yb = fnum(va.max),
        xl = latex_escape(time_title),
    );
    if is_tier {
        // A tier lane carries no meaningful value scale.
        s.push_str(", ytick=\\empty, yticklabels={}");
    } else if let Some(title) = axis_title(va) {
        let _ = write!(s, ", ylabel={{{}}}", latex_escape(&title));
    }
    s.push_str("]\n");

    for layer in &panel.layers {
        write_layer(s, idx, panel, layer, theme, sidecars);
    }
}

fn write_layer(
    s: &mut String,
    idx: usize,
    panel: &Panel,
    layer: &Layer,
    theme: phx_render::Theme,
    sidecars: &mut Vec<SidecarFile>,
) {
    match layer {
        Layer::Waveform {
            minmax,
            span,
            style,
        } => write_waveform(s, minmax, span.start, span.end, *style),
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
            let _ = writeln!(
                s,
                "\\addplot graphics[xmin={xa}, xmax={xb}, ymin={ya}, ymax={yb}]{{{name}}};",
                xa = fnum(t[0]),
                xb = fnum(t[1]),
                ya = fnum(f[0]),
                yb = fnum(f[1]),
            );
        }
        Layer::PitchLine { points, style, .. } | Layer::IntensityLine { points, style } => {
            write_line(s, points, *style);
        }
        Layer::SpectralSlice { bins, style } => write_line(s, bins, *style),
        Layer::FormantSpeckle { frames, style, .. } => {
            let _ = write!(
                s,
                "\\addplot[only marks, mark=*, mark size={r}pt, \
                 color={{rgb,255:red,{r8};green,{g8};blue,{b8}}}] coordinates {{",
                r = fnum(style.radius_pt),
                r8 = style.color.r,
                g8 = style.color.g,
                b8 = style.color.b,
            );
            for frame in frames {
                for p in &frame.points {
                    if p.frequency < panel.value_axis.min || p.frequency > panel.value_axis.max {
                        continue;
                    }
                    let _ = write!(s, "({},{}) ", fnum(frame.time), fnum(p.frequency));
                }
            }
            s.push_str("};\n");
        }
        Layer::Tiers { tiers, style } => {
            let bw = fnum(style.boundary.width_pt);
            for (j, tier) in tiers.iter().enumerate() {
                // Stack tiers as horizontal lanes within the 0..1 value range.
                let lanes = tiers.len().max(1) as f64;
                let ly0 = (lanes - 1.0 - j as f64) / lanes;
                let ly1 = (lanes - j as f64) / lanes;
                let mid = (ly0 + ly1) / 2.0;
                match &tier.content {
                    TierContent::Intervals(intervals) => {
                        for iv in intervals {
                            let _ = writeln!(
                                s,
                                "\\draw[line width={bw}pt, gray] (axis cs:{x},{a}) -- (axis cs:{x},{b});",
                                x = fnum(iv.xmin),
                                a = fnum(ly0),
                                b = fnum(ly1),
                            );
                            if !iv.label.is_empty() {
                                let _ = writeln!(
                                    s,
                                    "\\node[font=\\footnotesize] at (axis cs:{x},{y}) {{{l}}};",
                                    x = fnum((iv.xmin + iv.xmax) / 2.0),
                                    y = fnum(mid),
                                    l = latex_escape(&iv.label),
                                );
                            }
                        }
                        if let Some(last) = intervals.last() {
                            let _ = writeln!(
                                s,
                                "\\draw[line width={bw}pt, gray] (axis cs:{x},{a}) -- (axis cs:{x},{b});",
                                x = fnum(last.xmax),
                                a = fnum(ly0),
                                b = fnum(ly1),
                            );
                        }
                    }
                    TierContent::Points(points) => {
                        for pt in points {
                            let _ = writeln!(
                                s,
                                "\\draw[line width={bw}pt, gray] (axis cs:{x},{a}) -- (axis cs:{x},{b});",
                                x = fnum(pt.time),
                                a = fnum(ly0),
                                b = fnum(ly1),
                            );
                            if !pt.label.is_empty() {
                                let _ = writeln!(
                                    s,
                                    "\\node[font=\\footnotesize, anchor=south] at (axis cs:{x},{y}) {{{l}}};",
                                    x = fnum(pt.time),
                                    y = fnum(ly0),
                                    l = latex_escape(&pt.label),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn write_waveform(
    s: &mut String,
    minmax: &[crate::model::MinMax],
    t0: f64,
    t1: f64,
    style: LineStyle,
) {
    if minmax.is_empty() {
        return;
    }
    let merged = merge_envelope(minmax, MAX_WAVEFORM_COLUMNS);
    let n = merged.len();
    let x_at = |i: usize| {
        let frac = if n > 1 {
            i as f64 / (n - 1) as f64
        } else {
            0.0
        };
        t0 + frac * (t1 - t0)
    };
    let _ = write!(
        s,
        "\\addplot[draw=none, fill={{rgb,255:red,{r};green,{g};blue,{b}}}, \
         fill opacity=0.85] coordinates {{",
        r = style.color.r,
        g = style.color.g,
        b = style.color.b,
    );
    for (i, m) in merged.iter().enumerate() {
        let _ = write!(s, "({},{}) ", fnum(x_at(i)), fnum(f64::from(m.max)));
    }
    for (i, m) in merged.iter().enumerate().rev() {
        let _ = write!(s, "({},{}) ", fnum(x_at(i)), fnum(f64::from(m.min)));
    }
    s.push_str("} \\closedcycle;\n");
}

fn write_line(s: &mut String, points: &[(f64, f64)], style: LineStyle) {
    if points.is_empty() {
        return;
    }
    let dash = match style.dash {
        DashStyle::Solid => "",
        DashStyle::Dashed => ", dashed",
        DashStyle::Dotted => ", dotted",
    };
    for segment in gap_segments(points) {
        let _ = write!(
            s,
            "\\addplot[color={{rgb,255:red,{r};green,{g};blue,{b}}}, \
             line width={w}pt{dash}, mark=none] coordinates {{",
            r = style.color.r,
            g = style.color.g,
            b = style.color.b,
            w = fnum(style.width_pt),
        );
        for &(t, v) in segment {
            let _ = write!(s, "({},{}) ", fnum(t), fnum(v));
        }
        s.push_str("};\n");
    }
}

/// Split a point series where the time gap jumps well past the typical step, so
/// an unvoiced hole is not bridged, mirroring the SVG backend.
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

/// Merge a min/max envelope down to at most `target` columns, keeping the
/// minimum of the minima and maximum of the maxima over each merged span.
fn merge_envelope(minmax: &[crate::model::MinMax], target: usize) -> Vec<crate::model::MinMax> {
    if minmax.len() <= target || target == 0 {
        return minmax.to_vec();
    }
    let mut out = Vec::with_capacity(target);
    for i in 0..target {
        let lo = i * minmax.len() / target;
        let hi = ((i + 1) * minmax.len() / target).max(lo + 1);
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for m in &minmax[lo..hi] {
            min = min.min(m.min);
            max = max.max(m.max);
        }
        out.push(crate::model::MinMax { min, max });
    }
    out
}

/// Escape the LaTeX-special characters that appear in tier and axis labels.
fn latex_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            _ => out.push(c),
        }
    }
    out
}
