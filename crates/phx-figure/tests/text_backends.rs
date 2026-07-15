//! Integration tests for the text-emission backends.
//!
//! Each backend exports [`phx_figure::reference_figure`] and is checked for the
//! structural markers its toolchain needs plus byte-for-byte determinism.
//! Toolchain compilation lives in `tools/figcheck`; these tests cover the
//! emitted text itself.

use phx_figure::{
    CodeLang, figure_tiers, reference_figure, to_code, to_graphml, to_tikz, to_typst, to_vega,
};

fn spectrogram_sidecars(names: &[String]) -> usize {
    names
        .iter()
        .filter(|n| n.starts_with("spectrogram-p"))
        .count()
}

#[test]
fn tikz_is_a_standalone_pgfplots_document() {
    let export = to_tikz(&reference_figure());
    assert_eq!(export.main_name, "figure.tex");
    assert!(export.main.contains("\\documentclass"));
    assert!(export.main.contains("\\usepackage{pgfplots}"));
    assert!(export.main.contains("\\begin{groupplot}"));
    assert!(export.main.contains("\\addplot graphics"));
    assert!(export.main.contains("\\end{document}"));
    // One spectrogram panel means one PNG sidecar, referenced by name.
    let names: Vec<String> = export.sidecars.iter().map(|f| f.name.clone()).collect();
    assert_eq!(spectrogram_sidecars(&names), 1);
    assert!(export.main.contains(&names[0]));
    // The PNG sidecar carries a real image.
    assert!(export.sidecars[0].bytes.starts_with(b"\x89PNG"));
}

#[test]
fn typst_is_a_cetz_canvas() {
    let export = to_typst(&reference_figure());
    assert_eq!(export.main_name, "figure.typ");
    assert!(export.main.contains("#import \"@preview/cetz"));
    assert!(export.main.contains("cetz.canvas"));
    assert!(export.main.contains("image(\"spectrogram-p1.png\""));
    let names: Vec<String> = export.sidecars.iter().map(|f| f.name.clone()).collect();
    assert_eq!(spectrogram_sidecars(&names), 1);
}

#[test]
fn vega_is_valid_json_with_one_view_per_panel() {
    let fig = reference_figure();
    let json = to_vega(&fig);
    let value: serde_json::Value = serde_json::from_str(&json).expect("vega output is valid JSON");
    assert_eq!(
        value["$schema"],
        "https://vega.github.io/schema/vega-lite/v5.json"
    );
    let views = value["vconcat"].as_array().expect("vconcat array");
    assert_eq!(views.len(), fig.panels.len());
    // The spectrogram travels as a data-URI image mark.
    assert!(json.contains("data:image/png;base64,"));
    assert!(json.contains("\"type\": \"image\""));
}

#[test]
fn code_backends_emit_a_script_and_one_data_file_per_layer() {
    let fig = reference_figure();
    let layer_count: usize = fig.panels.iter().map(|p| p.layers.len()).sum();
    for (lang, name, marker) in [
        (CodeLang::Python, "figure.py", "matplotlib"),
        (CodeLang::R, "figure.R", "ggplot2"),
        (CodeLang::Julia, "figure.jl", "CairoMakie"),
    ] {
        let export = to_code(&fig, lang);
        assert_eq!(export.script_name, name);
        assert!(export.script.contains(marker), "{name} names its library");
        // The reference has four single-layer panels: one data file each.
        assert_eq!(export.data_files.len(), layer_count);
        assert!(!export.script.contains("seaborn"));
        // Every data file the script names is present in the bundle.
        for f in &export.data_files {
            assert!(
                export.script.contains(&f.name),
                "{} references {}",
                name,
                f.name
            );
        }
    }
}

#[test]
fn graphml_has_one_node_per_annotation_item() {
    let fig = reference_figure();
    let tiers = figure_tiers(&fig);
    let item_count: usize = tiers
        .iter()
        .map(|t| match &t.content {
            phx_figure::TierContent::Intervals(v) => v.len(),
            phx_figure::TierContent::Points(v) => v.len(),
        })
        .sum();
    let graphml = to_graphml(&tiers);
    assert!(graphml.starts_with("<?xml"));
    assert!(graphml.contains("<graphml"));
    assert_eq!(graphml.matches("<node ").count(), item_count);
    // A single tier yields precedence edges only: item_count - 1 of them.
    assert_eq!(
        graphml.matches("<edge ").count(),
        item_count.saturating_sub(1)
    );
    assert!(graphml.contains("relation\" attr.type=\"string\""));
}

#[test]
fn every_backend_is_byte_deterministic() {
    let fig = reference_figure();
    assert_eq!(to_tikz(&fig).main, to_tikz(&fig).main);
    assert_eq!(to_typst(&fig).main, to_typst(&fig).main);
    assert_eq!(to_vega(&fig), to_vega(&fig));
    assert_eq!(
        to_graphml(&figure_tiers(&fig)),
        to_graphml(&figure_tiers(&fig))
    );
    for lang in [CodeLang::Python, CodeLang::R, CodeLang::Julia] {
        let a = to_code(&fig, lang);
        let b = to_code(&fig, lang);
        assert_eq!(a.script, b.script);
        assert_eq!(a.data_files, b.data_files);
    }
    // Sidecar bytes are deterministic too: the spectrogram PNG encodes the same.
    assert_eq!(to_tikz(&fig).sidecars, to_tikz(&fig).sidecars);
}
