//! Generated-script backends: matplotlib, ggplot2, and Makie.
//!
//! [`to_code`] emits a plotting script in one of three languages together with
//! the data files it reads. Bulk arrays live in the sidecars — one CSV per
//! track or tier layer, one decibel matrix per spectrogram — so the script
//! stays small: it carries the figure's structure, axes, and colors inline and
//! reads the numbers from disk. matplotlib uses no seaborn, R uses base
//! ggplot2, Julia uses CairoMakie.
//!
//! The spectrogram sidecar is a raw-decibel matrix, not a baked image, so the
//! script re-colorizes it through the target library's colormap.

use std::fmt::Write as _;

use crate::backends::bundle::{
    CodeExport, CodeLang, SidecarFile, axis_title, colormap_name, db_window,
};
use crate::model::{Axis, Figure, Layer, Panel, TierContent};

/// A resolved axis range with an optional title, ready to bake into a script.
struct AxisPlan {
    min: f64,
    max: f64,
    title: Option<String>,
}

impl AxisPlan {
    fn from(axis: &Axis) -> Self {
        Self {
            min: axis.min,
            max: axis.max,
            title: axis_title(axis),
        }
    }
}

/// One drawable item within a panel, pointing at the data file it reads.
enum Item {
    Waveform {
        file: String,
        color: String,
    },
    Spectrogram {
        file: String,
        t0: f64,
        t1: f64,
        f0: f64,
        f1: f64,
        cmap: &'static str,
        vmin: f64,
        vmax: f64,
    },
    Line {
        file: String,
        color: String,
        width: f64,
    },
    Formant {
        file: String,
        color: String,
        radius: f64,
    },
    Tier {
        lanes: Vec<TierLane>,
    },
}

/// One tier lane: its data file, whether it holds points, and its display name.
struct TierLane {
    file: String,
    name: String,
    points: bool,
}

struct PanelPlan {
    time: AxisPlan,
    value: AxisPlan,
    height_share: f64,
    is_tier: bool,
    items: Vec<Item>,
}

struct Plan {
    width_in: f64,
    height_in: f64,
    panels: Vec<PanelPlan>,
    caption: Vec<String>,
    data_files: Vec<SidecarFile>,
}

/// Renders `fig` to a plotting script in `lang` plus the data files it reads.
#[must_use]
pub fn to_code(fig: &Figure, lang: CodeLang) -> CodeExport {
    let plan = build_plan(fig);
    let (script_name, script) = match lang {
        CodeLang::Python => ("figure.py".to_owned(), render_python(&plan)),
        CodeLang::R => ("figure.R".to_owned(), render_r(&plan)),
        CodeLang::Julia => ("figure.jl".to_owned(), render_julia(&plan)),
    };
    CodeExport {
        script_name,
        script,
        data_files: plan.data_files,
    }
}

fn build_plan(fig: &Figure) -> Plan {
    let mut data_files = Vec::new();
    let mut panels = Vec::new();

    for (idx, panel) in fig.panels.iter().enumerate() {
        let is_tier = panel
            .layers
            .iter()
            .any(|l| matches!(l, Layer::Tiers { .. }));
        let mut items = Vec::new();
        for layer in &panel.layers {
            build_item(idx, panel, layer, fig, &mut items, &mut data_files);
        }
        panels.push(PanelPlan {
            time: AxisPlan::from(&panel.time_axis),
            value: AxisPlan::from(&panel.value_axis),
            height_share: panel.height_share.max(0.0001),
            is_tier,
            items,
        });
    }

    Plan {
        width_in: fig.size.width_in(),
        height_in: fig.size.height_in(),
        panels,
        caption: caption_lines(fig),
        data_files,
    }
}

fn build_item(
    idx: usize,
    panel: &Panel,
    layer: &Layer,
    fig: &Figure,
    items: &mut Vec<Item>,
    data_files: &mut Vec<SidecarFile>,
) {
    match layer {
        Layer::Waveform {
            minmax,
            span,
            style,
        } => {
            let n = minmax.len();
            let mut csv = String::from("t,min,max\n");
            for (i, m) in minmax.iter().enumerate() {
                let frac = if n > 1 {
                    i as f64 / (n - 1) as f64
                } else {
                    0.0
                };
                let t = span.start + frac * (span.end - span.start);
                let _ = writeln!(
                    csv,
                    "{},{},{}",
                    num(t),
                    num(f64::from(m.min)),
                    num(f64::from(m.max))
                );
            }
            let file = format!("panel{idx}_waveform.csv");
            data_files.push(SidecarFile::text(&file, csv));
            items.push(Item::Waveform {
                file,
                color: hex(style.color),
            });
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
            let (w, h) = (*width as usize, *height as usize);
            let mut csv = String::with_capacity(db.len() * 6);
            for row in 0..h {
                for col in 0..w {
                    if col > 0 {
                        csv.push(',');
                    }
                    csv.push_str(&num(f64::from(db[row * w + col])));
                }
                csv.push('\n');
            }
            let file = format!("panel{idx}_spectrogram.csv");
            data_files.push(SidecarFile::text(&file, csv));
            let (vmin, vmax) = db_window(db, display);
            items.push(Item::Spectrogram {
                file,
                t0: t[0],
                t1: t[1],
                f0: f[0],
                f1: f[1],
                cmap: colormap_name(*colormap),
                vmin,
                vmax,
            });
            let _ = fig;
        }
        Layer::PitchLine { points, style, .. } | Layer::IntensityLine { points, style } => {
            let file = format!("panel{idx}_{}.csv", layer_slug(layer));
            data_files.push(SidecarFile::text(&file, xy_csv(points)));
            items.push(Item::Line {
                file,
                color: hex(style.color),
                width: style.width_pt,
            });
        }
        Layer::SpectralSlice { bins, style } => {
            let file = format!("panel{idx}_slice.csv");
            data_files.push(SidecarFile::text(&file, xy_csv(bins)));
            items.push(Item::Line {
                file,
                color: hex(style.color),
                width: style.width_pt,
            });
        }
        Layer::FormantSpeckle { frames, style, .. } => {
            let mut csv = String::from("t,f\n");
            for frame in frames {
                for p in &frame.points {
                    if p.frequency < panel.value_axis.min || p.frequency > panel.value_axis.max {
                        continue;
                    }
                    let _ = writeln!(csv, "{},{}", num(frame.time), num(p.frequency));
                }
            }
            let file = format!("panel{idx}_formants.csv");
            data_files.push(SidecarFile::text(&file, csv));
            items.push(Item::Formant {
                file,
                color: hex(style.color),
                radius: style.radius_pt,
            });
        }
        Layer::Tiers { tiers, .. } => {
            let mut lanes = Vec::new();
            for (j, tier) in tiers.iter().enumerate() {
                let (csv, points) = match &tier.content {
                    TierContent::Intervals(intervals) => {
                        let mut c = String::from("xmin,xmax,label\n");
                        for iv in intervals {
                            let _ = writeln!(
                                c,
                                "{},{},{}",
                                num(iv.xmin),
                                num(iv.xmax),
                                csv_field(&iv.label)
                            );
                        }
                        (c, false)
                    }
                    TierContent::Points(pts) => {
                        let mut c = String::from("time,label\n");
                        for pt in pts {
                            let _ = writeln!(c, "{},{}", num(pt.time), csv_field(&pt.label));
                        }
                        (c, true)
                    }
                };
                let file = format!("panel{idx}_tier{j}.csv");
                data_files.push(SidecarFile::text(&file, csv));
                lanes.push(TierLane {
                    file,
                    name: tier.name.clone(),
                    points,
                });
            }
            items.push(Item::Tier { lanes });
        }
    }
}

fn layer_slug(layer: &Layer) -> &'static str {
    match layer {
        Layer::PitchLine { .. } => "pitch",
        Layer::IntensityLine { .. } => "intensity",
        _ => "line",
    }
}

fn xy_csv(points: &[(f64, f64)]) -> String {
    let mut csv = String::from("t,v\n");
    for &(t, v) in points {
        let _ = writeln!(csv, "{},{}", num(t), num(v));
    }
    csv
}

fn caption_lines(fig: &Figure) -> Vec<String> {
    fig.caption_meta
        .sources
        .iter()
        .map(|src| {
            let mut line = format!("{:?}", src.layer);
            if !src.params.is_empty() {
                let ps: Vec<String> = src.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
                line.push_str(" — ");
                line.push_str(&ps.join(", "));
            }
            if src.smoothed == Some(true) {
                line.push_str(" (smoothed track)");
            }
            line
        })
        .collect()
}

// ---- Python / matplotlib ----------------------------------------------------

fn render_python(plan: &Plan) -> String {
    let mut s = String::with_capacity(8 * 1024);
    s.push_str(
        "# Phonetics figure: panels stacked over a shared time axis (seconds).\n\
         # Each panel's value axis carries its own unit; the spectrogram panel\n\
         # shows a decibel matrix re-colorized through the named colormap. Bulk\n\
         # data is read from the CSV and matrix files beside this script.\n\
         import csv\n\
         import os\n\n\
         import numpy as np\n\
         import matplotlib\n\
         matplotlib.use(\"Agg\")\n\
         import matplotlib.pyplot as plt\n\n\
         HERE = os.path.dirname(os.path.abspath(__file__))\n\n\n\
         def data(name):\n    return os.path.join(HERE, name)\n\n\n\
         def broken(t, v):\n    \
         \"\"\"Insert NaN gaps so unvoiced holes are not bridged by a line.\"\"\"\n    \
         dt = np.diff(t)\n    pos = dt[dt > 0]\n    \
         thresh = np.median(pos) * 2.5 if pos.size else np.inf\n    \
         out_t, out_v = [], []\n    prev = None\n    \
         for ti, vi in zip(t, v):\n        \
         if prev is not None and ti - prev > thresh:\n            \
         out_t.append((prev + ti) / 2.0)\n            out_v.append(np.nan)\n        \
         out_t.append(ti)\n        out_v.append(vi)\n        prev = ti\n    \
         return np.array(out_t), np.array(out_v)\n\n\n",
    );

    let ratios: Vec<String> = plan.panels.iter().map(|p| num(p.height_share)).collect();
    let _ = write!(
        s,
        "fig, axes = plt.subplots(\n    {n}, 1, figsize=({w}, {h}), sharex=True,\n    \
         gridspec_kw={{\"height_ratios\": [{ratios}]}},\n)\n\
         if {n} == 1:\n    axes = [axes]\n\n",
        n = plan.panels.len(),
        w = num(plan.width_in),
        h = num(plan.height_in),
        ratios = ratios.join(", "),
    );

    for (idx, panel) in plan.panels.iter().enumerate() {
        let _ = writeln!(s, "ax = axes[{idx}]");
        for item in &panel.items {
            render_python_item(&mut s, item, panel);
        }
        if panel.is_tier {
            s.push_str("ax.set_yticks([])\n");
        } else if let Some(title) = &panel.value.title {
            let _ = writeln!(s, "ax.set_ylabel({})", py_str(title));
        }
        let _ = write!(
            s,
            "ax.set_ylim({}, {})\n\n",
            num(panel.value.min),
            num(panel.value.max)
        );
    }

    if let Some(last) = plan.panels.last() {
        let _ = writeln!(
            s,
            "axes[-1].set_xlim({}, {})",
            num(last.time.min),
            num(last.time.max)
        );
        if let Some(title) = &last.time.title {
            let _ = writeln!(s, "axes[-1].set_xlabel({})", py_str(title));
        }
    }

    s.push_str("fig.tight_layout()\nfig.subplots_adjust(bottom=0.16)\n");
    for (i, line) in plan.caption.iter().enumerate() {
        let y = 0.012 + (plan.caption.len() - 1 - i) as f64 * 0.022;
        let _ = writeln!(
            s,
            "fig.text(0.01, {y}, {t}, fontsize=7, ha=\"left\", va=\"bottom\")",
            y = num(y),
            t = py_str(line),
        );
    }
    s.push_str("fig.savefig(\"figure.png\", dpi=150)\n");
    s
}

fn render_python_item(s: &mut String, item: &Item, panel: &PanelPlan) {
    match item {
        Item::Waveform { file, color } => {
            let _ = write!(
                s,
                "wav = np.genfromtxt(data({f}), delimiter=\",\", names=True)\n\
                 ax.fill_between(wav[\"t\"], wav[\"min\"], wav[\"max\"], \
                 color={c}, alpha=0.85, linewidth=0)\n",
                f = py_str(file),
                c = py_str(color),
            );
        }
        Item::Spectrogram {
            file,
            t0,
            t1,
            f0,
            f1,
            cmap,
            vmin,
            vmax,
        } => {
            let _ = write!(
                s,
                "spec = np.loadtxt(data({f}), delimiter=\",\")\n\
                 ax.imshow(spec, origin=\"lower\", aspect=\"auto\", cmap={cmap},\n    \
                 extent=[{t0}, {t1}, {f0}, {f1}], vmin={vmin}, vmax={vmax})\n",
                f = py_str(file),
                cmap = py_str(cmap),
                t0 = num(*t0),
                t1 = num(*t1),
                f0 = num(*f0),
                f1 = num(*f1),
                vmin = num(*vmin),
                vmax = num(*vmax),
            );
        }
        Item::Line { file, color, width } => {
            let _ = write!(
                s,
                "ln = np.genfromtxt(data({f}), delimiter=\",\", names=True)\n\
                 lt, lv = broken(np.atleast_1d(ln[\"t\"]), np.atleast_1d(ln[\"v\"]))\n\
                 ax.plot(lt, lv, color={c}, linewidth={w})\n",
                f = py_str(file),
                c = py_str(color),
                w = num(*width),
            );
        }
        Item::Formant {
            file,
            color,
            radius,
        } => {
            let _ = write!(
                s,
                "fm = np.genfromtxt(data({f}), delimiter=\",\", names=True)\n\
                 ax.scatter(np.atleast_1d(fm[\"t\"]), np.atleast_1d(fm[\"f\"]), \
                 s={size}, c={c})\n",
                f = py_str(file),
                c = py_str(color),
                size = num(radius * radius * 6.0),
            );
        }
        Item::Tier { lanes } => {
            let n = lanes.len().max(1);
            for (j, lane) in lanes.iter().enumerate() {
                let ly0 = (n - 1 - j) as f64 / n as f64;
                let ly1 = (n - j) as f64 / n as f64;
                let mid = (ly0 + ly1) / 2.0;
                let _ = write!(
                    s,
                    "with open(data({f}), newline=\"\", encoding=\"utf-8\") as fh:\n    \
                     rows = list(csv.reader(fh))[1:]\n",
                    f = py_str(&lane.file),
                );
                if lane.points {
                    let _ = write!(
                        s,
                        "for time, label in rows:\n    \
                         ax.axvline(float(time), color=\"#888888\", linewidth=0.5)\n    \
                         if label:\n        ax.text(float(time), {y}, label, ha=\"center\", \
                         va=\"bottom\", fontsize=8)\n",
                        y = num(ly0),
                    );
                } else {
                    let _ = write!(
                        s,
                        "for xmin, xmax, label in rows:\n    \
                         ax.axvline(float(xmin), color=\"#888888\", linewidth=0.5)\n    \
                         if label:\n        ax.text((float(xmin) + float(xmax)) / 2.0, {y}, \
                         label, ha=\"center\", va=\"center\", fontsize=8)\n\
                         if rows:\n    ax.axvline(float(rows[-1][1]), color=\"#888888\", linewidth=0.5)\n",
                        y = num(mid),
                    );
                }
                let _ = writeln!(
                    s,
                    "ax.text(-0.005, {y}, {name}, transform=ax.get_yaxis_transform(), ha=\"right\", va=\"center\", fontsize=7)",
                    y = num(mid),
                    name = py_str(&lane.name)
                );
            }
        }
    }
    let _ = panel;
}

// ---- R / ggplot2 ------------------------------------------------------------

fn render_r(plan: &Plan) -> String {
    let mut s = String::with_capacity(4 * 1024);
    s.push_str(
        "# Phonetics figure: panels stacked over a shared time axis (seconds).\n\
         # Each panel's value axis carries its own unit; the spectrogram panel\n\
         # shows a decibel matrix re-colorized through a continuous fill scale.\n\
         # Bulk data is read from the CSV and matrix files beside this script.\n\
         library(ggplot2)\n\
         library(grid)\n\n\
         args <- commandArgs(trailingOnly = FALSE)\n\
         file_arg <- sub(\"^--file=\", \"\", args[grep(\"^--file=\", args)])\n\
         here <- if (length(file_arg)) dirname(normalizePath(file_arg)) else getwd()\n\
         path <- function(name) file.path(here, name)\n\n\
         panels <- list()\n\n",
    );

    for (idx, panel) in plan.panels.iter().enumerate() {
        let _ = write!(s, "# panel {idx}\np <- ggplot()\n");
        for item in &panel.items {
            render_r_item(&mut s, item);
        }
        let xlab = panel
            .time
            .title
            .clone()
            .unwrap_or_else(|| "Time (s)".to_owned());
        let ylab = panel.value.title.clone().unwrap_or_default();
        let _ = write!(
            s,
            "p <- p + coord_cartesian(xlim = c({x0}, {x1}), ylim = c({y0}, {y1}), expand = FALSE) +\n  \
             labs(x = {xl}, y = {yl}) + theme_minimal()\n",
            x0 = num(panel.time.min),
            x1 = num(panel.time.max),
            y0 = num(panel.value.min),
            y1 = num(panel.value.max),
            xl = r_str(&xlab),
            yl = r_str(&ylab),
        );
        if panel.is_tier {
            s.push_str(
                "p <- p + theme(axis.text.y = element_blank(), axis.ticks.y = element_blank())\n",
            );
        }
        let _ = write!(s, "panels[[{}]] <- p\n\n", idx + 1);
    }

    let heights: Vec<String> = plan.panels.iter().map(|p| num(p.height_share)).collect();
    let _ = write!(
        s,
        "png(\"figure.png\", width = {w}, height = {h}, units = \"in\", res = 150)\n\
         grid.newpage()\n\
         pushViewport(viewport(layout = grid.layout({n}, 1, heights = unit(c({heights}), \"null\"))))\n\
         for (i in seq_along(panels)) {{\n  \
         pushViewport(viewport(layout.pos.row = i, layout.pos.col = 1))\n  \
         print(panels[[i]], newpage = FALSE)\n  popViewport()\n}}\n\
         dev.off()\n",
        w = num(plan.width_in),
        h = num(plan.height_in),
        n = plan.panels.len(),
        heights = heights.join(", "),
    );
    s
}

fn render_r_item(s: &mut String, item: &Item) {
    match item {
        Item::Waveform { file, color } => {
            let _ = write!(
                s,
                "wav <- read.csv(path({f}))\n\
                 p <- p + geom_ribbon(data = wav, aes(x = t, ymin = min, ymax = max), \
                 fill = {c}, alpha = 0.85)\n",
                f = r_str(file),
                c = r_str(color),
            );
        }
        Item::Spectrogram {
            file,
            t0,
            t1,
            f0,
            f1,
            cmap,
            vmin,
            vmax,
        } => {
            let _ = write!(
                s,
                "m <- as.matrix(read.csv(path({f}), header = FALSE))\n\
                 spec <- expand.grid(row = seq_len(nrow(m)), col = seq_len(ncol(m)))\n\
                 spec$db <- as.vector(m)\n\
                 spec$x <- {t0} + (spec$col - 0.5) / ncol(m) * ({t1} - {t0})\n\
                 spec$y <- {f0} + (spec$row - 0.5) / nrow(m) * ({f1} - {f0})\n\
                 p <- p + geom_raster(data = spec, aes(x = x, y = y, fill = db)) +\n  \
                 scale_fill_gradientn(colours = grDevices::hcl.colors(64, {cmap}), \
                 limits = c({vmin}, {vmax}), oob = scales::squish)\n",
                f = r_str(file),
                cmap = r_str(&r_palette(cmap)),
                t0 = num(*t0),
                t1 = num(*t1),
                f0 = num(*f0),
                f1 = num(*f1),
                vmin = num(*vmin),
                vmax = num(*vmax),
            );
        }
        Item::Line { file, color, width } => {
            let _ = write!(
                s,
                "ln <- read.csv(path({f}))\n\
                 p <- p + geom_line(data = ln, aes(x = t, y = v), colour = {c}, linewidth = {w})\n",
                f = r_str(file),
                c = r_str(color),
                w = num(width / 2.0),
            );
        }
        Item::Formant {
            file,
            color,
            radius,
        } => {
            let _ = write!(
                s,
                "fm <- read.csv(path({f}))\n\
                 p <- p + geom_point(data = fm, aes(x = t, y = f), colour = {c}, size = {sz})\n",
                f = r_str(file),
                c = r_str(color),
                sz = num(radius * 0.6),
            );
        }
        Item::Tier { lanes } => {
            let n = lanes.len().max(1);
            for (j, lane) in lanes.iter().enumerate() {
                let ly0 = (n - 1 - j) as f64 / n as f64;
                let ly1 = (n - j) as f64 / n as f64;
                let mid = (ly0 + ly1) / 2.0;
                if lane.points {
                    let _ = write!(
                        s,
                        "tier <- read.csv(path({f}))\n\
                         p <- p + geom_vline(data = tier, aes(xintercept = time), colour = \"#888888\", linewidth = 0.2) +\n  \
                         geom_text(data = tier, aes(x = time, y = {y}, label = label), size = 3)\n",
                        f = r_str(&lane.file),
                        y = num(ly0),
                    );
                } else {
                    let _ = write!(
                        s,
                        "tier <- read.csv(path({f}))\n\
                         p <- p + geom_vline(data = tier, aes(xintercept = xmin), colour = \"#888888\", linewidth = 0.2) +\n  \
                         geom_text(data = tier, aes(x = (xmin + xmax) / 2, y = {y}, label = label), size = 3)\n",
                        f = r_str(&lane.file),
                        y = num(mid),
                    );
                }
            }
        }
    }
}

fn r_palette(cmap: &str) -> String {
    match cmap {
        "viridis" => "Viridis".to_owned(),
        "magma" => "Magma".to_owned(),
        _ => "Grays".to_owned(),
    }
}

// ---- Julia / Makie ----------------------------------------------------------

fn render_julia(plan: &Plan) -> String {
    let mut s = String::with_capacity(4 * 1024);
    s.push_str(
        "# Phonetics figure: panels stacked over a shared time axis (seconds).\n\
         # Each panel's value axis carries its own unit; the spectrogram panel\n\
         # shows a decibel matrix re-colorized through the named colormap. Bulk\n\
         # data is read from the CSV and matrix files beside this script.\n\
         using CairoMakie\n\
         using DelimitedFiles\n\n\
         here = @__DIR__\n\
         path(name) = joinpath(here, name)\n\n\
         function readxy(name)\n    \
         raw = readdlm(path(name), ',', skipstart = 1)\n    \
         return raw[:, 1], raw[:, 2]\n\
         end\n\n",
    );
    let _ = write!(
        s,
        "fig = Figure(size = ({w}, {h}))\n\n",
        w = num(plan.width_in * 96.0),
        h = num(plan.height_in * 96.0),
    );

    let total: f64 = plan.panels.iter().map(|p| p.height_share).sum();
    let total = if total > 0.0 { total } else { 1.0 };
    for (idx, panel) in plan.panels.iter().enumerate() {
        let row = idx + 1;
        let xlab = if idx + 1 == plan.panels.len() {
            panel
                .time
                .title
                .clone()
                .unwrap_or_else(|| "Time (s)".to_owned())
        } else {
            String::new()
        };
        let ylab = if panel.is_tier {
            String::new()
        } else {
            panel.value.title.clone().unwrap_or_default()
        };
        let _ = write!(
            s,
            "ax{row} = Axis(fig[{row}, 1], xlabel = {xl}, ylabel = {yl}, height = Relative({share}))\n\
             limits!(ax{row}, {x0}, {x1}, {y0}, {y1})\n",
            xl = jl_str(&xlab),
            yl = jl_str(&ylab),
            share = num(panel.height_share / total),
            x0 = num(panel.time.min),
            x1 = num(panel.time.max),
            y0 = num(panel.value.min),
            y1 = num(panel.value.max),
        );
        for item in &panel.items {
            render_julia_item(&mut s, item, row);
        }
        if panel.is_tier {
            let _ = writeln!(s, "hideydecorations!(ax{row})");
        }
        s.push('\n');
    }
    s.push_str("save(\"figure.png\", fig)\n");
    s
}

fn render_julia_item(s: &mut String, item: &Item, row: usize) {
    match item {
        Item::Waveform { file, color } => {
            let _ = write!(
                s,
                "wav = readdlm(path({f}), ',', skipstart = 1)\n\
                 band!(ax{row}, wav[:, 1], wav[:, 2], wav[:, 3], color = ({c}, 0.85))\n",
                f = jl_str(file),
                c = jl_str(color),
            );
        }
        Item::Spectrogram {
            file,
            t0,
            t1,
            f0,
            f1,
            cmap,
            vmin,
            vmax,
        } => {
            let _ = write!(
                s,
                "spec = readdlm(path({f}), ',')\n\
                 heatmap!(ax{row}, range({t0}, {t1}, length = size(spec, 2)), \
                 range({f0}, {f1}, length = size(spec, 1)), permutedims(spec), \
                 colormap = {cmap}, colorrange = ({vmin}, {vmax}))\n",
                f = jl_str(file),
                cmap = jl_str(cmap),
                t0 = num(*t0),
                t1 = num(*t1),
                f0 = num(*f0),
                f1 = num(*f1),
                vmin = num(*vmin),
                vmax = num(*vmax),
            );
        }
        Item::Line { file, color, width } => {
            let _ = write!(
                s,
                "lt, lv = readxy({f})\n\
                 lines!(ax{row}, lt, lv, color = {c}, linewidth = {w})\n",
                f = jl_str(file),
                c = jl_str(color),
                w = num(*width),
            );
        }
        Item::Formant {
            file,
            color,
            radius,
        } => {
            let _ = write!(
                s,
                "ft, ff = readxy({f})\n\
                 scatter!(ax{row}, ft, ff, color = {c}, markersize = {sz})\n",
                f = jl_str(file),
                c = jl_str(color),
                sz = num(radius * 3.0),
            );
        }
        Item::Tier { lanes } => {
            let n = lanes.len().max(1);
            for (j, lane) in lanes.iter().enumerate() {
                let ly0 = (n - 1 - j) as f64 / n as f64;
                let ly1 = (n - j) as f64 / n as f64;
                let mid = (ly0 + ly1) / 2.0;
                let _ = writeln!(
                    s,
                    "tier = readdlm(path({f}), ',', skipstart = 1)",
                    f = jl_str(&lane.file),
                );
                if lane.points {
                    let _ = write!(
                        s,
                        "for r in 1:size(tier, 1)\n    \
                         vlines!(ax{row}, Float64(tier[r, 1]), color = (:gray, 0.7))\n    \
                         text!(ax{row}, Float64(tier[r, 1]), {y}, text = string(tier[r, 2]), align = (:center, :bottom), fontsize = 10)\nend\n",
                        y = num(ly0),
                    );
                } else {
                    let _ = write!(
                        s,
                        "for r in 1:size(tier, 1)\n    \
                         vlines!(ax{row}, Float64(tier[r, 1]), color = (:gray, 0.7))\n    \
                         text!(ax{row}, (Float64(tier[r, 1]) + Float64(tier[r, 2])) / 2, {y}, text = string(tier[r, 3]), align = (:center, :center), fontsize = 10)\nend\n\
                         vlines!(ax{row}, Float64(tier[end, 2]), color = (:gray, 0.7))\n",
                        y = num(mid),
                    );
                }
            }
        }
    }
}

// ---- shared formatting ------------------------------------------------------

/// Format a number for a data file or a script literal: finite values only,
/// compact, negative zero collapsed. Non-finite values become an empty CSV
/// cell / a NaN-safe blank.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "nan".to_owned();
    }
    let v = if v == 0.0 { 0.0 } else { v };
    let mut out = format!("{v:.6}");
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

fn hex(color: crate::style::RgbaColor) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// Escape a value for a CSV field, quoting when it holds a comma, quote, or
/// newline.
fn csv_field(input: &str) -> String {
    if input.contains([',', '"', '\n', '\r']) {
        let escaped = input.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        input.to_owned()
    }
}

/// A Python single-quoted string literal.
fn py_str(input: &str) -> String {
    quote(input, '\'')
}

/// An R double-quoted string literal.
fn r_str(input: &str) -> String {
    quote(input, '"')
}

/// A Julia double-quoted string literal.
fn jl_str(input: &str) -> String {
    quote(input, '"')
}

fn quote(input: &str, q: char) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push(q);
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push(q);
    out
}
