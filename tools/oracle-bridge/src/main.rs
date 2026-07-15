//! Rust-side oracle bridge.
//!
//! Runs `phx-pitch`, `phx-formant`, and `phx-intensity` against the fixture
//! WAVs under `tests/fixtures/audio/` with the same parameters
//! `tools/oracle/src/oracle/cases.py` defines for `pitch-defaults`,
//! `formant-defaults`, and `intensity-defaults`, then writes measured JSON in
//! the shape `tools/oracle/src/oracle/diff.py` expects:
//! `{case, measure, audio, params, frames}`, same naming scheme as
//! `tools/oracle/references/` (`<case>__<audio-stem>.json`), so
//! `uv run oracle diff-all --measured-dir <out-dir>` can compare them
//! directly against the committed parselmouth references.
//!
//! Case -> phx crate mapping (`docs/plan/tasks/phase-2.md` T2.1-T2.3):
//!   - `pitch-defaults`     -> `phx_pitch::pitch_track`
//!   - `formant-defaults`   -> `phx_formant::formant_track`
//!   - `intensity-defaults` -> `phx_intensity::intensity_track`
//!
//! Frames are emitted exactly as each crate's own [`phx_dsp::FrameGrid`]
//! places them; this bridge performs no resampling, trimming, or
//! re-alignment of the Rust output to match Praat's frame times. A frame
//! count mismatch against a reference is a real disagreement (per
//! `diff.py`'s positional frame matching) and is reported as such, not
//! patched over here.
//!
//! `spectrogram-slice-defaults` is out of scope: `diff.py` refuses to diff
//! it (validation.md: spectrogram parity is checked against a scipy STFT
//! reference instead).

mod json;

use std::env::ArgsOs;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use phx_audio::{Audio, AudioView};
use phx_formant::{FormantParams, formant_track};
use phx_intensity::{IntensityParams, intensity_track};
use phx_pitch::{PitchParams, pitch_track};

use json::Json;

/// The oracle cases this bridge covers, in the order `cases.py` declares
/// them. `spectrogram-slice-defaults` is intentionally absent (see the
/// module doc comment).
const CASES: [&str; 3] = ["pitch-defaults", "formant-defaults", "intensity-defaults"];

/// `oracle.cases.SPEECH_AND_VOWEL_CORPUS`, copied verbatim.
const AUDIO_CORPUS: [&str; 4] = [
    "arctic_bdl_a0001.wav",
    "arctic_slt_a0001.wav",
    "librispeech_2277-149896-0005.wav",
    "synth_vowel_a.wav",
];

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args_os()) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!(
                "usage: oracle-bridge --out-dir <DIR> [--case <NAME>]... [--audio <FILENAME>]..."
            );
            return ExitCode::from(2);
        }
    };

    let repo_root = repo_root();
    let fixtures_dir = repo_root.join("tests/fixtures/audio");
    let references_dir = repo_root.join("tools/oracle/references");

    if let Err(err) = std::fs::create_dir_all(&args.out_dir) {
        eprintln!("error: creating {}: {err}", args.out_dir.display());
        return ExitCode::from(1);
    }

    let cases: Vec<&str> = if args.cases.is_empty() {
        CASES.to_vec()
    } else {
        args.cases.iter().map(String::as_str).collect()
    };
    let audio_files: Vec<&str> = if args.audio.is_empty() {
        AUDIO_CORPUS.to_vec()
    } else {
        args.audio.iter().map(String::as_str).collect()
    };

    let mut written = 0usize;
    let mut skipped: Vec<String> = Vec::new();

    for &audio_filename in &audio_files {
        let audio_path = fixtures_dir.join(audio_filename);
        let bytes = match std::fs::read(&audio_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("error: reading {}: {err}", audio_path.display());
                return ExitCode::from(1);
            }
        };
        let audio = match Audio::from_wav_bytes(&bytes) {
            Ok(audio) => audio,
            Err(err) => {
                eprintln!("error: decoding {}: {err}", audio_path.display());
                return ExitCode::from(1);
            }
        };
        let view = audio.slice_samples(0..audio.frames());

        for &case_name in &cases {
            let reference_name = measured_filename(case_name, audio_filename);
            if !references_dir.join(&reference_name).is_file() {
                skipped.push(reference_name);
                continue;
            }

            let payload = match case_name {
                "pitch-defaults" => pitch_payload(view.clone(), audio_filename),
                "formant-defaults" => formant_payload(view.clone(), audio_filename),
                "intensity-defaults" => intensity_payload(view.clone(), audio_filename),
                other => {
                    eprintln!("error: unknown case {other:?}");
                    return ExitCode::from(2);
                }
            };

            let out_path = args.out_dir.join(&reference_name);
            if let Err(err) = std::fs::write(&out_path, payload.to_pretty_string()) {
                eprintln!("error: writing {}: {err}", out_path.display());
                return ExitCode::from(1);
            }
            println!("wrote {}", out_path.display());
            written += 1;
        }
    }

    println!(
        "wrote {written} measured file(s) in {}",
        args.out_dir.display()
    );
    if !skipped.is_empty() {
        println!(
            "skipped {} case/audio pair(s) with no committed reference in {}:",
            skipped.len(),
            references_dir.display()
        );
        for name in &skipped {
            println!("  - {name}");
        }
    }

    ExitCode::SUCCESS
}

/// Same naming scheme as `oracle.runner.reference_filename`:
/// `<case>__<audio-stem>.json`.
fn measured_filename(case_name: &str, audio_filename: &str) -> String {
    let stem = Path::new(audio_filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(audio_filename);
    format!("{case_name}__{stem}.json")
}

/// Locates the workspace root from this crate's own manifest directory
/// (`tools/oracle-bridge` -> `tools` -> repo root), independent of the
/// process's current working directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tools/oracle-bridge always lives two directories below the workspace root")
        .to_path_buf()
}

fn audio_field(audio_filename: &str) -> Json {
    Json::String(format!("tests/fixtures/audio/{audio_filename}"))
}

/// Runs `phx_pitch::pitch_track` with `PitchParams::default()` -- the same
/// defaults `oracle.params.PitchParams()` declares (both mirror Praat's
/// "Sound: To Pitch (raw autocorrelation)..." documented defaults) -- and
/// builds the `pitch-defaults` measured payload.
fn pitch_payload(view: AudioView<'_>, audio_filename: &str) -> Json {
    let params = PitchParams::default();
    let track = pitch_track(view, &params);

    let frames = track
        .frames()
        .iter()
        .map(|frame| {
            Json::object(vec![
                ("time", Json::number(frame.time)),
                ("voiced", Json::Bool(frame.f0.is_some())),
                ("f0_hz", frame.f0.map_or(Json::Null, Json::number)),
                ("strength", Json::number(frame.strength)),
            ])
        })
        .collect();

    Json::object(vec![
        ("case", Json::String("pitch-defaults".to_string())),
        ("measure", Json::String("pitch".to_string())),
        ("audio", audio_field(audio_filename)),
        ("params", pitch_params_json(&params)),
        ("frames", Json::Array(frames)),
    ])
}

/// Field-for-field mirror of `oracle.params.PitchParams.as_dict()`; field
/// names and defaults match `crates/phx-pitch/src/params.rs::PitchParams`.
fn pitch_params_json(params: &PitchParams) -> Json {
    Json::object(vec![
        (
            "time_step",
            params.time_step.map_or(Json::Null, Json::number),
        ),
        ("floor_hz", Json::number(params.floor_hz)),
        ("ceiling_hz", Json::number(params.ceiling_hz)),
        ("max_candidates", Json::number(params.max_candidates as f64)),
        ("very_accurate", Json::Bool(params.very_accurate)),
        ("silence_threshold", Json::number(params.silence_threshold)),
        ("voicing_threshold", Json::number(params.voicing_threshold)),
        ("octave_cost", Json::number(params.octave_cost)),
        ("octave_jump_cost", Json::number(params.octave_jump_cost)),
        (
            "voiced_unvoiced_cost",
            Json::number(params.voiced_unvoiced_cost),
        ),
    ])
}

/// Runs `phx_formant::formant_track` with `FormantParams::default()` -- the
/// same defaults `oracle.params.FormantParams()` declares (both mirror
/// Praat's "Sound: To Formant (burg)..." documented defaults) -- and builds
/// the `formant-defaults` measured payload from the raw per-frame Burg
/// candidates.
///
/// `oracle.measures.formant_frames` reads parselmouth's `to_formant_burg`
/// output (`Formant.get_value_at_time(slot, t)` for `slot` in
/// `1..=max_formants`), which is Praat's raw per-frame Burg result -- Praat
/// numbers the formants within each frame by ascending frequency and applies
/// trajectory tracking only through the separate `Formant: Track...` command,
/// which this case does not invoke. The Rust side matches it with the raw
/// `formant_track` output, whose `FormantFrame::formants` are likewise the
/// frequency-gated Burg roots sorted ascending. Numbering them positionally
/// (`1..=formants.len()`) therefore reproduces Praat's ascending 1-based slot
/// numbering exactly, with no tracking layer on either side.
fn formant_payload(view: AudioView<'_>, audio_filename: &str) -> Json {
    let params = FormantParams::default();
    let raw = formant_track(view, &params);

    let frames = raw
        .frames
        .iter()
        .map(|frame| {
            let formants = frame
                .formants
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    Json::object(vec![
                        ("formant", Json::number((index + 1) as f64)),
                        ("frequency_hz", Json::number(point.frequency)),
                        ("bandwidth_hz", Json::number(point.bandwidth)),
                    ])
                })
                .collect();
            Json::object(vec![
                ("time", Json::number(frame.time)),
                ("formants", Json::Array(formants)),
            ])
        })
        .collect();

    Json::object(vec![
        ("case", Json::String("formant-defaults".to_string())),
        ("measure", Json::String("formant".to_string())),
        ("audio", audio_field(audio_filename)),
        ("params", formant_params_json(&params)),
        ("frames", Json::Array(frames)),
    ])
}

/// Field-for-field mirror of `oracle.params.FormantParams.as_dict()`; field
/// names and defaults match
/// `crates/phx-formant/src/params.rs::FormantParams`.
fn formant_params_json(params: &FormantParams) -> Json {
    Json::object(vec![
        ("ceiling_hz", Json::number(params.ceiling_hz)),
        ("max_formants", Json::number(params.max_formants as f64)),
        ("window_length", Json::number(params.window_length)),
        (
            "time_step",
            params.time_step.map_or(Json::Null, Json::number),
        ),
        (
            "preemphasis_from_hz",
            Json::number(params.preemphasis_from_hz),
        ),
    ])
}

/// Runs `phx_intensity::intensity_track` with `IntensityParams::default()`
/// -- the same defaults `oracle.params.IntensityParams()` declares (both
/// mirror Praat's "Sound: To Intensity..." documented defaults) -- and
/// builds the `intensity-defaults` measured payload.
fn intensity_payload(view: AudioView<'_>, audio_filename: &str) -> Json {
    let params = IntensityParams::default();
    let track = intensity_track(view, &params);

    let frames = track
        .iter()
        .map(|(time, db)| {
            Json::object(vec![("time", Json::number(time)), ("db", Json::number(db))])
        })
        .collect();

    Json::object(vec![
        ("case", Json::String("intensity-defaults".to_string())),
        ("measure", Json::String("intensity".to_string())),
        ("audio", audio_field(audio_filename)),
        ("params", intensity_params_json(&params)),
        ("frames", Json::Array(frames)),
    ])
}

/// Field-for-field mirror of `oracle.params.IntensityParams.as_dict()`;
/// field names and defaults match
/// `crates/phx-intensity/src/lib.rs::IntensityParams`.
fn intensity_params_json(params: &IntensityParams) -> Json {
    Json::object(vec![
        ("pitch_floor_hz", Json::number(params.pitch_floor_hz)),
        (
            "time_step",
            params.time_step.map_or(Json::Null, Json::number),
        ),
        ("subtract_mean", Json::Bool(params.subtract_mean)),
    ])
}

/// Parsed command-line arguments.
struct Args {
    /// Directory measured JSON files are written into.
    out_dir: PathBuf,
    /// Case names to run; empty means all of [`CASES`].
    cases: Vec<String>,
    /// Fixture WAV filenames to run; empty means all of [`AUDIO_CORPUS`].
    audio: Vec<String>,
}

impl Args {
    fn parse(mut args: ArgsOs) -> Result<Self, String> {
        args.next(); // argv[0]
        let mut out_dir = None;
        let mut cases = Vec::new();
        let mut audio = Vec::new();

        while let Some(arg) = args.next() {
            let arg = to_utf8(arg)?;
            match arg.as_str() {
                "--out-dir" => {
                    let value = args.next().ok_or("--out-dir needs a value")?;
                    out_dir = Some(PathBuf::from(value));
                }
                "--case" => {
                    let value = to_utf8(args.next().ok_or("--case needs a value")?)?;
                    if !CASES.contains(&value.as_str()) {
                        return Err(format!(
                            "unknown case {value:?}; known cases: {}",
                            CASES.join(", ")
                        ));
                    }
                    cases.push(value);
                }
                "--audio" => {
                    audio.push(to_utf8(args.next().ok_or("--audio needs a value")?)?);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self {
            out_dir: out_dir.ok_or("--out-dir is required")?,
            cases,
            audio,
        })
    }
}

fn to_utf8(value: OsString) -> Result<String, String> {
    value
        .into_string()
        .map_err(|raw| format!("argument is not valid UTF-8: {raw:?}"))
}
