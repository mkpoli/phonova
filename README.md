# Phonia

Phonia is an open-source toolkit for phonetic research: a Rust analysis core
surrounded by several user interfaces, meant to replace Praat for everyday
research tasks such as analyzing voice, managing audio, displaying
spectrograms, annotating recordings, and drawing publication figures. The
core is a Cargo workspace of small library crates with no UI dependencies,
compiled natively for a Tauri desktop app and to WebAssembly for a
browser-based app. The crate name `phonix` on crates.io belongs to an
unrelated project, so published crates use the `phx-` prefix.

Source: <https://github.com/mkpoli/phonia>. Web demo: <https://phonia.app>.
