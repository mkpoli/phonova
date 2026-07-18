# phx-textgrid

A reader and writer for Praat TextGrid files, built on the [`phx-annot`]
annotation model.

The reader accepts every format Praat writes: the long (tagged) and short
(bare values) text formats, decoded from UTF-8, UTF-16 (via byte-order mark),
or Latin-1, and Praat's binary format (detected by its `ooBinaryFile` magic).
Files predating Praat's Unicode support decode as Latin-1; a file from that era
saved under Mac OS Classic may instead carry MacRoman-encoded bytes, which are
also valid Latin-1 byte-for-byte and are not distinguished from it, matching
the ambiguity Praat's own manual describes for encoding-less files. The
binary format is undocumented by Praat; this crate derives it clean-room from
oracle-generated sample pairs rather than from Praat's source — see
`docs/binary-format.md`. The reader reports the detected source provenance
alongside the document and never panics on malformed input. The writer emits
one canonical shape: long text format, UTF-8, `LF` line endings, no
byte-order mark; writing the binary format is out of scope.

TextGrid carries no cross-tier relation data, so every imported tier is
independent.

## Example

```rust
use phx_textgrid::{SourceInfo, read, write};

let text = "\
File type = \"ooTextFile\"
Object class = \"TextGrid\"

xmin = 0
xmax = 1
tiers? <absent>
";
let (doc, source) = read(text.as_bytes())?;
match source {
    SourceInfo::Text { variant, encoding } => {
        println!("read {variant:?} text, {encoding:?} encoding");
    }
    SourceInfo::Binary => println!("read binary format"),
}

// Re-emit as canonical long-format UTF-8.
let canonical = write(&doc)?;
assert!(std::str::from_utf8(&canonical).is_ok());
# Ok::<(), Box<dyn std::error::Error>>(())
```

`read` rejects a structurally invalid document — a reversed domain, a
gapped or overlapping interval, a duplicate id — with
[`TextGridError::Invalid`]; [`read_lenient`] returns the same issues
alongside the document instead, for recovery or import tooling that wants
to show what is wrong with a file rather than refuse to open it.

## Compatibility

Requires Rust 1.88 or newer (edition 2024).

## License

Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE) at
your option.

[`phx-annot`]: https://crates.io/crates/phx-annot
