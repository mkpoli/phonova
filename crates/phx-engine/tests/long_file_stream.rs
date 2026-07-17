//! Long-file streamed-open measurement, kept out of the default run.
//!
//! Generates a ≥30-minute WAV on disk, opens it through a file-backed
//! [`ByteReader`] that never loads the whole file, and reports time-to-metadata,
//! pyramid-build time, resident memory, and waveform-scroll latency. Run with:
//!
//! ```text
//! cargo test -p phx-engine --release --test long_file_stream -- --ignored --nocapture
//! ```
//!
//! The disk-backed reader uses the Unix `FileExt::read_exact_at`, so the whole
//! measurement compiles only on Unix; other targets get an empty test binary.
#![cfg(unix)]

use std::fs::File;
use std::io::{BufWriter, Write};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::time::Instant;

use phx_audio::{AudioError, ByteReader, StreamingWav};
use phx_engine::Engine;

struct FileReader {
    file: File,
    len: u64,
}

impl FileReader {
    fn open(path: &PathBuf) -> Self {
        let file = File::open(path).expect("open temp wav");
        let len = file.metadata().expect("stat temp wav").len();
        Self { file, len }
    }
}

impl ByteReader for FileReader {
    fn total_len(&self) -> u64 {
        self.len
    }

    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), AudioError> {
        self.file
            .read_exact_at(buf, offset)
            .map_err(|e| AudioError::Io(e.to_string()))
    }
}

fn write_wav(path: &PathBuf, sample_rate: u32, seconds: u64) {
    let frames = sample_rate as u64 * seconds;
    let data_len = frames * 2; // 16-bit mono
    let mut w = BufWriter::new(File::create(path).expect("create temp wav"));
    // RIFF/WAVE header for 16-bit mono PCM.
    w.write_all(b"RIFF").unwrap();
    w.write_all(&((36 + data_len) as u32).to_le_bytes())
        .unwrap();
    w.write_all(b"WAVE").unwrap();
    w.write_all(b"fmt ").unwrap();
    w.write_all(&16u32.to_le_bytes()).unwrap();
    w.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    w.write_all(&1u16.to_le_bytes()).unwrap(); // mono
    w.write_all(&sample_rate.to_le_bytes()).unwrap();
    w.write_all(&(sample_rate * 2).to_le_bytes()).unwrap(); // byte rate
    w.write_all(&2u16.to_le_bytes()).unwrap(); // block align
    w.write_all(&16u16.to_le_bytes()).unwrap(); // bits
    w.write_all(b"data").unwrap();
    w.write_all(&(data_len as u32).to_le_bytes()).unwrap();

    // A slow 220 Hz sweep so the waveform has real min/max structure.
    let mut buf = Vec::with_capacity(1 << 16);
    for i in 0..frames {
        let t = i as f64 / sample_rate as f64;
        let s = (2.0 * std::f64::consts::PI * (110.0 + 5.0 * t) * t).sin() * 0.8;
        buf.extend_from_slice(&((s * 32_000.0) as i16).to_le_bytes());
        if buf.len() >= (1 << 16) {
            w.write_all(&buf).unwrap();
            buf.clear();
        }
    }
    w.write_all(&buf).unwrap();
    w.flush().unwrap();
}

fn rss_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

#[test]
#[ignore = "heavy: generates a ~170 MB WAV; run explicitly"]
fn long_file_opens_at_header_speed_with_bounded_memory() {
    let sample_rate = 48_000u32;
    let seconds = std::env::var("PHX_LONG_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_800u64); // 30 minutes
    let path = std::env::temp_dir().join(format!("phx_long_{}.wav", std::process::id()));

    let gen_start = Instant::now();
    write_wav(&path, sample_rate, seconds);
    let file_bytes = std::fs::metadata(&path).unwrap().len();
    eprintln!(
        "generated {:.1} min WAV = {:.1} MiB on disk in {:.2}s",
        seconds as f64 / 60.0,
        file_bytes as f64 / (1024.0 * 1024.0),
        gen_start.elapsed().as_secs_f64()
    );

    let rss_start = rss_kib();

    // Header-only metadata: must be independent of file length.
    let t = Instant::now();
    let source = StreamingWav::open(FileReader::open(&path)).expect("open header");
    let info = source.info();
    let metadata_ms = t.elapsed().as_secs_f64() * 1e3;
    drop(source);
    assert_eq!(info.channels, 1);
    assert_eq!(info.frames, (sample_rate as u64 * seconds) as usize);

    // Full streamed open: header + bounded pyramid pass.
    let mut engine = Engine::new();
    let t = Instant::now();
    let id = engine
        .open_streaming_wav(FileReader::open(&path), Some("long".into()))
        .expect("streamed open");
    let open_s = t.elapsed().as_secs_f64();
    let rss_after_open = rss_kib();

    let duration = engine.audio_info(id).unwrap().duration;

    // Waveform scroll latency: 200 full-width slices across the timeline.
    let t = Instant::now();
    let mut worst = 0.0f64;
    for k in 0..200 {
        let c = (k as f64 / 200.0) * duration;
        let half = duration * 0.02;
        let s = Instant::now();
        let slice = engine
            .waveform_slice(id, (c - half).max(0.0), (c + half).min(duration), 1_600)
            .unwrap();
        assert_eq!(slice.len(), 1_600);
        worst = worst.max(s.elapsed().as_secs_f64() * 1e3);
    }
    let scroll_total = t.elapsed().as_secs_f64() * 1e3;
    let rss_after_scroll = rss_kib();

    eprintln!("time-to-metadata (header only): {metadata_ms:.3} ms");
    eprintln!("streamed open (header + pyramid): {open_s:.3} s");
    eprintln!(
        "RSS start            : {:.1} MiB",
        rss_start as f64 / 1024.0
    );
    eprintln!(
        "RSS after open       : {:.1} MiB (Δ {:.1})",
        rss_after_open as f64 / 1024.0,
        (rss_after_open.saturating_sub(rss_start)) as f64 / 1024.0
    );
    eprintln!(
        "RSS after 200 slices : {:.1} MiB",
        rss_after_scroll as f64 / 1024.0
    );
    eprintln!("waveform: 200 slices in {scroll_total:.1} ms, worst single {worst:.3} ms");

    // Gates: open well under 5 s; memory nowhere near the decoded footprint;
    // scrolling smooth (worst slice far under a 32 ms frame budget).
    assert!(
        open_s < 5.0,
        "streamed open took {open_s:.3}s, over the 5 s gate"
    );
    let decoded_mib = (sample_rate as u64 * seconds * 4) as f64 / (1024.0 * 1024.0);
    let delta_mib = (rss_after_scroll.saturating_sub(rss_start)) as f64 / 1024.0;
    assert!(
        delta_mib < decoded_mib * 0.25,
        "RSS grew {delta_mib:.1} MiB, not bounded well under the {decoded_mib:.1} MiB decoded size"
    );
    assert!(
        worst < 32.0,
        "worst waveform slice {worst:.3} ms exceeds a 32 ms frame"
    );

    std::fs::remove_file(&path).ok();
}
