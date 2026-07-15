//! Write the reference figure's text and code exports for inspection.
//!
//! Emits the TikZ, Typst, Vega-Lite, matplotlib, ggplot2, Makie, and GraphML
//! exports of [`phx_figure::reference_figure`] into a target directory (default
//! `target/figure-text`), each backend in its own subdirectory alongside its
//! sidecar or data files.
//!
//! ```text
//! cargo run -p phx-figure --example emit_text_exports
//! ```

use std::path::{Path, PathBuf};

use phx_figure::{
    CodeExport, CodeLang, SidecarFile, TextExport, figure_tiers, reference_figure, to_code,
    to_graphml, to_tikz, to_typst, to_vega,
};
use phx_render::Theme;

fn write_sidecars(dir: &Path, files: &[SidecarFile]) -> std::io::Result<()> {
    for f in files {
        std::fs::write(dir.join(&f.name), &f.bytes)?;
    }
    Ok(())
}

fn write_text(root: &Path, name: &str, export: &TextExport) -> std::io::Result<()> {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(&export.main_name), export.main.as_bytes())?;
    write_sidecars(&dir, &export.sidecars)?;
    println!(
        "{name}: {} + {} sidecar(s)",
        export.main_name,
        export.sidecars.len()
    );
    Ok(())
}

fn write_code(root: &Path, name: &str, export: &CodeExport) -> std::io::Result<()> {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(&export.script_name), export.script.as_bytes())?;
    write_sidecars(&dir, &export.data_files)?;
    println!(
        "{name}: {} + {} data file(s)",
        export.script_name,
        export.data_files.len()
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("target/figure-text"), PathBuf::from);
    let theme = match std::env::args().nth(2).as_deref() {
        Some("dark") => Theme::Dark,
        _ => Theme::Light,
    };
    std::fs::create_dir_all(&root)?;

    let mut fig = reference_figure();
    fig.theme = theme;

    write_text(&root, "tikz", &to_tikz(&fig))?;
    write_text(&root, "typst", &to_typst(&fig))?;

    std::fs::write(root.join("figure.vega.json"), to_vega(&fig))?;
    println!("vega: figure.vega.json");

    write_code(&root, "matplotlib", &to_code(&fig, CodeLang::Python))?;
    write_code(&root, "ggplot2", &to_code(&fig, CodeLang::R))?;
    write_code(&root, "makie", &to_code(&fig, CodeLang::Julia))?;

    let graphml = to_graphml(&figure_tiers(&fig));
    std::fs::write(root.join("annotation.graphml"), &graphml)?;
    println!("graphml: annotation.graphml");

    println!("wrote to {}", root.display());
    Ok(())
}
