//! GraphML backend for annotation tiers.
//!
//! [`to_graphml`] turns a set of annotation tiers into a GraphML graph: one
//! node per interval or point (carrying its tier, label, and span), and edges
//! for precedence within a tier and for alignment and containment across tiers.
//!
//! The output is deterministic and self-contained GraphML that a graph library
//! loads directly.

use std::fmt::Write as _;

use crate::model::{Figure, Layer, TierContent, TierData};

/// Coincidence tolerance, in seconds, for two boundaries counting as aligned.
const ALIGN_EPS: f64 = 1e-6;

/// One graph node: an interval or a point with its span.
struct Node {
    id: String,
    tier: String,
    label: String,
    kind: &'static str,
    xmin: f64,
    xmax: f64,
}

impl Node {
    fn boundaries(&self) -> Vec<f64> {
        if self.kind == "point" {
            vec![self.xmin]
        } else {
            vec![self.xmin, self.xmax]
        }
    }
}

/// Collects the annotation tiers embedded in a figure's [`Layer::Tiers`] layers.
#[must_use]
pub fn figure_tiers(fig: &Figure) -> Vec<TierData> {
    let mut out = Vec::new();
    for panel in &fig.panels {
        for layer in &panel.layers {
            if let Layer::Tiers { tiers, .. } = layer {
                out.extend(tiers.iter().cloned());
            }
        }
    }
    out
}

/// Renders `tiers` to a GraphML document.
///
/// Nodes carry `tier`, `label`, `kind` (`interval` or `point`), and `xmin`/
/// `xmax` span attributes. Edges carry a `relation`: `precedence` between
/// consecutive items of one tier, `alignment` between items of different tiers
/// that share a boundary time, and `containment` from an interval to an item of
/// another tier whose span falls inside it.
#[must_use]
pub fn to_graphml(tiers: &[TierData]) -> String {
    let mut nodes: Vec<Node> = Vec::new();
    // Track where each tier's nodes start and end, for precedence within tier.
    let mut tier_ranges: Vec<(usize, usize)> = Vec::new();
    for (ti, tier) in tiers.iter().enumerate() {
        let start = nodes.len();
        match &tier.content {
            TierContent::Intervals(intervals) => {
                for (ii, iv) in intervals.iter().enumerate() {
                    nodes.push(Node {
                        id: format!("t{ti}_i{ii}"),
                        tier: tier.name.clone(),
                        label: iv.label.clone(),
                        kind: "interval",
                        xmin: iv.xmin,
                        xmax: iv.xmax,
                    });
                }
            }
            TierContent::Points(points) => {
                for (pi, pt) in points.iter().enumerate() {
                    nodes.push(Node {
                        id: format!("t{ti}_p{pi}"),
                        tier: tier.name.clone(),
                        label: pt.label.clone(),
                        kind: "point",
                        xmin: pt.time,
                        xmax: pt.time,
                    });
                }
            }
        }
        tier_ranges.push((start, nodes.len()));
    }

    let mut edges: Vec<(String, String, &'static str)> = Vec::new();

    // Precedence: consecutive items within one tier, in time order.
    for &(start, end) in &tier_ranges {
        for i in start + 1..end {
            edges.push((nodes[i - 1].id.clone(), nodes[i].id.clone(), "precedence"));
        }
    }

    // Cross-tier relations, comparing each earlier tier against each later one.
    for a in 0..tier_ranges.len() {
        for b in a + 1..tier_ranges.len() {
            let (as_, ae) = tier_ranges[a];
            let (bs, be) = tier_ranges[b];
            for i in as_..ae {
                for j in bs..be {
                    if shares_boundary(&nodes[i], &nodes[j]) {
                        edges.push((nodes[i].id.clone(), nodes[j].id.clone(), "alignment"));
                    }
                    if contains(&nodes[i], &nodes[j]) {
                        edges.push((nodes[i].id.clone(), nodes[j].id.clone(), "containment"));
                    } else if contains(&nodes[j], &nodes[i]) {
                        edges.push((nodes[j].id.clone(), nodes[i].id.clone(), "containment"));
                    }
                }
            }
        }
    }

    render(&nodes, &edges)
}

fn shares_boundary(a: &Node, b: &Node) -> bool {
    for ba in a.boundaries() {
        for bb in b.boundaries() {
            if (ba - bb).abs() <= ALIGN_EPS {
                return true;
            }
        }
    }
    false
}

/// Whether `a` (an interval) strictly contains `b`'s span.
fn contains(a: &Node, b: &Node) -> bool {
    a.kind == "interval"
        && a.xmin <= b.xmin + ALIGN_EPS
        && a.xmax >= b.xmax - ALIGN_EPS
        && (a.xmax - a.xmin) > (b.xmax - b.xmin) + ALIGN_EPS
}

fn render(nodes: &[Node], edges: &[(String, String, &'static str)]) -> String {
    let mut s = String::with_capacity(1024 + nodes.len() * 160 + edges.len() * 80);
    s.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n\
         <key id=\"d_tier\" for=\"node\" attr.name=\"tier\" attr.type=\"string\"/>\n\
         <key id=\"d_label\" for=\"node\" attr.name=\"label\" attr.type=\"string\"/>\n\
         <key id=\"d_kind\" for=\"node\" attr.name=\"kind\" attr.type=\"string\"/>\n\
         <key id=\"d_xmin\" for=\"node\" attr.name=\"xmin\" attr.type=\"double\"/>\n\
         <key id=\"d_xmax\" for=\"node\" attr.name=\"xmax\" attr.type=\"double\"/>\n\
         <key id=\"e_rel\" for=\"edge\" attr.name=\"relation\" attr.type=\"string\"/>\n\
         <graph id=\"annotation\" edgedefault=\"directed\">\n",
    );
    for node in nodes {
        let _ = writeln!(
            s,
            "  <node id=\"{id}\">\
             <data key=\"d_tier\">{tier}</data>\
             <data key=\"d_label\">{label}</data>\
             <data key=\"d_kind\">{kind}</data>\
             <data key=\"d_xmin\">{xmin}</data>\
             <data key=\"d_xmax\">{xmax}</data></node>",
            id = xml_attr(&node.id),
            tier = xml_text(&node.tier),
            label = xml_text(&node.label),
            kind = node.kind,
            xmin = num(node.xmin),
            xmax = num(node.xmax),
        );
    }
    for (i, (src, dst, rel)) in edges.iter().enumerate() {
        let _ = writeln!(
            s,
            "  <edge id=\"e{i}\" source=\"{src}\" target=\"{dst}\">\
             <data key=\"e_rel\">{rel}</data></edge>",
            src = xml_attr(src),
            dst = xml_attr(dst),
        );
    }
    s.push_str("</graph>\n</graphml>\n");
    s
}

fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_owned();
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

fn xml_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn xml_attr(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}
