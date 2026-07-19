//! Re-serializes any TextGrid (long, short, binary, or a legacy encoding)
//! into `phx-textgrid`'s canonical long-format UTF-8 output, unchanged in
//! content. Useful for normalizing a third-party TextGrid (e.g. one
//! produced by another aligner) to this repo's canonical form without
//! hand-editing it.
//!
//! ```text
//! cargo run -p phx-textgrid --example reformat_textgrid -- input.TextGrid output.TextGrid
//! ```

use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let [_, in_path, out_path] = args.as_slice() else {
        eprintln!("usage: reformat_textgrid <input.TextGrid> <output.TextGrid>");
        return ExitCode::FAILURE;
    };
    let bytes = fs::read(in_path).expect("read input file");
    let (doc, _source_info) = phx_textgrid::read(&bytes).expect("parse textgrid");
    let out = phx_textgrid::write(&doc).expect("write textgrid");
    fs::write(out_path, out).expect("write output file");
    println!("wrote {out_path}");
    ExitCode::SUCCESS
}
