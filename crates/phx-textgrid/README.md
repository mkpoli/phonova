# phx-textgrid

A reader and writer for Praat TextGrid files, built on the [`phx-annot`]
annotation model.

The reader accepts both text formats Praat writes — long (tagged) and short
(bare values) — and decodes UTF-8, UTF-16 (via byte-order mark), and Latin-1.
Files predating Praat's Unicode support decode as Latin-1; a file from that era
saved under Mac OS Classic may instead carry MacRoman-encoded bytes, which are
also valid Latin-1 byte-for-byte and are not distinguished from it, matching
the ambiguity Praat's own manual describes for encoding-less files. The reader
reports the detected format variant and encoding alongside the document and
never panics on malformed input. The writer emits one canonical shape: long
format, UTF-8, `LF` line endings, no byte-order mark.

The binary TextGrid variant is not supported and is reported as a typed error.
TextGrid carries no cross-tier relation data, so every imported tier is
independent.

## Example

```rust
use phx_textgrid::{read, write};

let bytes = std::fs::read("example.TextGrid")?;
let (doc, source) = read(&bytes)?;
println!("read {:?} format, {:?} encoding", source.variant, source.encoding);

// Re-emit as canonical long-format UTF-8.
std::fs::write("canonical.TextGrid", write(&doc))?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Compatibility

Requires Rust 1.88 or newer (edition 2024).

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE) at
your option.

[`phx-annot`]: https://crates.io/crates/phx-annot
