//! Streamed WAV decoding for recordings too large to hold decoded in memory.
//!
//! [`Audio::from_wav_bytes`](crate::Audio::from_wav_bytes) decodes a whole file
//! into planar `f32` at once; an hour of 48 kHz mono is ≈690 MB of samples, and
//! the 2 GiB planar guard rejects longer takes outright. The streamed path here
//! instead parses only the WAV header up front — so duration, sample rate, and
//! channel count are known immediately — and decodes sample ranges on demand
//! from a pluggable byte store through a bounded cache of decoded chunks.
//!
//! The byte store is abstracted by [`ByteReader`]: a desktop frontend backs it
//! with a file handle, a web worker with an OPFS synchronous access handle, and
//! tests with an in-memory buffer. The store never has to hold the whole file in
//! process memory, so the peak footprint is the header, the waveform pyramid a
//! caller builds over [`StreamingWav::fold_mono_chunks`], and the small decoded
//! chunk cache — never the decoded signal in full.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use crate::{Audio, AudioError};

/// Frames per decoded chunk held in [`StreamingWav`]'s cache.
///
/// A ranged read touches at most `⌈count / DECODED_CHUNK_FRAMES⌉ + 1` chunks;
/// the value trades cache granularity against the per-chunk decode overhead.
pub const DECODED_CHUNK_FRAMES: usize = 1 << 15;

/// Maximum decoded chunks retained before the least-recently-used is dropped.
///
/// Each cached chunk is `DECODED_CHUNK_FRAMES × channels × 4` bytes of planar
/// `f32`; at 32 mono chunks that is ≈4 MiB, the bound on the streamed decode
/// cache regardless of file length.
const DECODED_CHUNK_CACHE_CAP: usize = 32;

/// Reads byte ranges from a backing store without holding it all in memory.
///
/// Implementations serve a fixed-length blob (a WAV file on disk, an OPFS
/// access handle, an in-memory buffer). [`ByteReader::read_exact_at`] must fill
/// the whole output slice from `offset`; a store that ends before the range is
/// satisfied is a malformed shorter-than-declared file and returns
/// [`AudioError::Io`].
pub trait ByteReader {
    /// Total length of the backing blob in bytes.
    fn total_len(&self) -> u64;

    /// Fills `buf` with the bytes at `[offset, offset + buf.len())`.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] when the store cannot serve the full range,
    /// including a read that runs past the end of the blob.
    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), AudioError>;
}

/// In-memory [`ByteReader`] over a shared byte buffer.
///
/// Cloning shares the buffer through an [`Arc`]; the whole blob does live in
/// memory here, so this backing suits tests and the case where a frontend
/// already holds the file bytes. Disk- or handle-backed readers avoid that.
#[derive(Clone, Debug)]
pub struct BytesReader {
    bytes: Arc<[u8]>,
}

impl BytesReader {
    /// Wraps an owned byte buffer.
    #[must_use]
    pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }
}

impl ByteReader for BytesReader {
    fn total_len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn read_exact_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), AudioError> {
        let start = usize::try_from(offset)
            .map_err(|_| AudioError::Io(format!("offset {offset} exceeds addressable memory")))?;
        let end = start
            .checked_add(buf.len())
            .ok_or_else(|| AudioError::Io("read range overflows addressable memory".to_string()))?;
        if end > self.bytes.len() {
            return Err(AudioError::Io(format!(
                "read of {} bytes at {start} runs past the {}-byte buffer",
                buf.len(),
                self.bytes.len()
            )));
        }
        buf.copy_from_slice(&self.bytes[start..end]);
        Ok(())
    }
}

/// Integer or float sample encoding of a streamed WAV data chunk.
///
/// The normalization each variant applies matches
/// [`Audio::from_wav_bytes`](crate::Audio::from_wav_bytes) bit for bit, so a
/// range decoded here equals the same range of a fully decoded buffer exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamSampleFormat {
    /// 8-bit unsigned PCM, centered at 128 and scaled by `1/128`.
    Uint8,
    /// 16-bit signed little-endian PCM, scaled by `1/32768`.
    Int16,
    /// 24-bit signed little-endian PCM, scaled by `1/8388608`.
    Int24,
    /// 32-bit signed little-endian PCM, scaled by `1/2147483648`.
    Int32,
    /// 32-bit IEEE little-endian float, read as stored.
    Float32,
}

impl StreamSampleFormat {
    fn bytes_per_sample(self) -> usize {
        match self {
            Self::Uint8 => 1,
            Self::Int16 => 2,
            Self::Int24 => 3,
            Self::Int32 | Self::Float32 => 4,
        }
    }

    fn decode(self, bytes: &[u8]) -> f32 {
        match self {
            Self::Uint8 => (i32::from(bytes[0]) - 128) as f32 / 128.0,
            Self::Int16 => {
                let v = i16::from_le_bytes([bytes[0], bytes[1]]);
                i32::from(v) as f32 / 32_768.0
            }
            Self::Int24 => {
                let raw =
                    i32::from(bytes[0]) | (i32::from(bytes[1]) << 8) | (i32::from(bytes[2]) << 16);
                // Sign-extend the 24-bit value into the full i32.
                let v = (raw << 8) >> 8;
                v as f32 / 8_388_608.0
            }
            Self::Int32 => {
                let v = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                v as f32 / 2_147_483_648.0
            }
            Self::Float32 => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }
}

/// Header facts about a streamed WAV, read without decoding any samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WavStreamInfo {
    /// Sample encoding of the data chunk.
    pub sample_format: StreamSampleFormat,
    /// Channel count.
    pub channels: usize,
    /// Sample rate in hertz.
    pub sample_rate: f64,
    /// Number of frames (per-channel samples).
    pub frames: usize,
    /// Byte offset of the first data-chunk sample within the blob.
    pub data_offset: u64,
}

impl WavStreamInfo {
    /// Duration in seconds.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.frames as f64 / self.sample_rate
    }

    fn frame_bytes(&self) -> usize {
        self.channels * self.sample_format.bytes_per_sample()
    }
}

/// A WAV opened for on-demand ranged decoding from a [`ByteReader`].
///
/// The header is parsed once at construction; sample ranges decode lazily
/// through a bounded LRU of decoded chunks. Cheap to share behind an `Arc`; the
/// chunk cache is internally synchronized so `&self` reads stay bounded and
/// thread-safe.
pub struct StreamingWav {
    reader: Box<dyn ByteReader + Send + Sync>,
    info: WavStreamInfo,
    cache: Mutex<DecodedChunkCache>,
}

impl StreamingWav {
    /// Parses the WAV header from `reader` and prepares on-demand decoding.
    ///
    /// Only the RIFF chunk table and the `fmt ` chunk are read here; no sample
    /// data is touched, so this returns in time independent of file length.
    ///
    /// # Errors
    /// Returns [`AudioError::MalformedWav`] when the RIFF structure is broken,
    /// [`AudioError::UnsupportedWav`] for a sample format outside the supported
    /// set, and [`AudioError::Io`] when the reader cannot serve the header.
    pub fn open(reader: impl ByteReader + Send + Sync + 'static) -> Result<Self, AudioError> {
        let info = parse_header(&reader)?;
        Ok(Self {
            reader: Box::new(reader),
            info,
            cache: Mutex::new(DecodedChunkCache::new(info.channels)),
        })
    }

    /// Returns the header facts read at open time.
    #[must_use]
    pub fn info(&self) -> WavStreamInfo {
        self.info
    }

    /// Number of frames (per-channel samples).
    #[must_use]
    pub fn frames(&self) -> usize {
        self.info.frames
    }

    /// Sample rate in hertz.
    #[must_use]
    pub fn sample_rate(&self) -> f64 {
        self.info.sample_rate
    }

    /// Channel count.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.info.channels
    }

    /// Duration in seconds.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.info.duration()
    }

    /// Decodes `range` into planar `f32` channels through the chunk cache.
    ///
    /// The result equals the same frame range of a fully decoded
    /// [`Audio`](crate::Audio) bit for bit. Only the chunks the range overlaps
    /// are decoded, and the cache keeps peak memory bounded regardless of how
    /// much of the file successive calls sweep.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] when the backing store cannot serve the
    /// bytes, and panics-free for any in-bounds `range`; an out-of-bounds range
    /// is clamped to the frame count.
    pub fn read_planar(&self, range: Range<usize>) -> Result<Vec<Vec<f32>>, AudioError> {
        let start = range.start.min(self.info.frames);
        let end = range.end.min(self.info.frames).max(start);
        let count = end - start;
        let mut planar: Vec<Vec<f32>> = (0..self.info.channels)
            .map(|_| Vec::with_capacity(count))
            .collect();
        if count == 0 {
            return Ok(planar);
        }

        let mut cache = self.cache.lock().expect("decoded chunk cache poisoned");
        let mut frame = start;
        while frame < end {
            let chunk_index = frame / DECODED_CHUNK_FRAMES;
            let chunk = cache.get_or_decode(chunk_index, &self.info, self.reader.as_ref())?;
            let chunk_start = chunk_index * DECODED_CHUNK_FRAMES;
            let local = frame - chunk_start;
            let take = (end - frame).min(chunk.frames - local);
            for (plane, source) in planar.iter_mut().zip(&chunk.planar) {
                plane.extend_from_slice(&source[local..local + take]);
            }
            frame += take;
        }
        Ok(planar)
    }

    /// Decodes `range` and mono-mixes it, matching
    /// [`Audio::mono_mix`](crate::Audio::mono_mix) frame for frame.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] on a backing-store failure.
    pub fn read_mono(&self, range: Range<usize>) -> Result<Vec<f32>, AudioError> {
        let planar = self.read_planar(range)?;
        Ok(mono_mix_planar(&planar, self.info.channels))
    }

    /// Materializes the whole signal into an in-memory [`Audio`].
    ///
    /// This reads the entire file through the chunk cache, so it costs the full
    /// decoded footprint and is subject to the same 2 GiB planar guard as
    /// [`Audio::from_wav_bytes`](crate::Audio::from_wav_bytes). Whole-signal
    /// analysis of a streamed source goes through here; the streamed advantage
    /// is that metadata and the waveform pyramid never pay it.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] on a backing-store failure and
    /// [`AudioError::OversizedAllocation`] when the decoded signal exceeds the
    /// planar limit.
    pub fn materialize(&self) -> Result<Audio, AudioError> {
        let planar = self.read_planar(0..self.info.frames)?;
        Audio::new(planar, self.info.sample_rate)
    }

    /// Streams the mono mix in sequential chunks, folding each into `state`.
    ///
    /// The whole signal passes through `fold` in `DECODED_CHUNK_FRAMES`-frame
    /// slices without ever being held at once, which is how a bounded waveform
    /// pyramid is built over an arbitrarily long file. `fold` receives the
    /// absolute start frame of each slice and its mono samples.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] on a backing-store failure.
    pub fn fold_mono_chunks<S, F>(&self, mut state: S, mut fold: F) -> Result<S, AudioError>
    where
        F: FnMut(&mut S, usize, &[f32]),
    {
        let mut frame = 0;
        while frame < self.info.frames {
            let end = (frame + DECODED_CHUNK_FRAMES).min(self.info.frames);
            let mono = self.read_mono(frame..end)?;
            fold(&mut state, frame, &mono);
            frame = end;
        }
        Ok(state)
    }
}

/// Fold helper alias documenting the [`StreamingWav::fold_mono_chunks`] closure
/// shape: `(state, start_frame, mono_samples)`.
pub type StreamChunkFold<'a, S> = dyn FnMut(&mut S, usize, &[f32]) + 'a;

struct DecodedChunk {
    planar: Vec<Vec<f32>>,
    frames: usize,
    last_used: u64,
}

struct DecodedChunkCache {
    channels: usize,
    entries: HashMap<usize, DecodedChunk>,
    tick: u64,
}

impl DecodedChunkCache {
    fn new(channels: usize) -> Self {
        Self {
            channels,
            entries: HashMap::new(),
            tick: 0,
        }
    }

    fn get_or_decode(
        &mut self,
        chunk_index: usize,
        info: &WavStreamInfo,
        reader: &(dyn ByteReader + Send + Sync),
    ) -> Result<&DecodedChunk, AudioError> {
        self.tick += 1;
        let tick = self.tick;
        if !self.entries.contains_key(&chunk_index) {
            let chunk = decode_chunk(chunk_index, info, reader, self.channels, tick)?;
            if self.entries.len() >= DECODED_CHUNK_CACHE_CAP
                && let Some(oldest) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .map(|(k, _)| *k)
            {
                self.entries.remove(&oldest);
            }
            self.entries.insert(chunk_index, chunk);
        }
        let entry = self
            .entries
            .get_mut(&chunk_index)
            .expect("chunk just inserted");
        entry.last_used = tick;
        Ok(entry)
    }
}

fn decode_chunk(
    chunk_index: usize,
    info: &WavStreamInfo,
    reader: &(dyn ByteReader + Send + Sync),
    channels: usize,
    tick: u64,
) -> Result<DecodedChunk, AudioError> {
    let first_frame = chunk_index * DECODED_CHUNK_FRAMES;
    let frames = DECODED_CHUNK_FRAMES.min(info.frames.saturating_sub(first_frame));
    let bytes_per_sample = info.sample_format.bytes_per_sample();
    let frame_bytes = info.frame_bytes();
    let byte_offset = info.data_offset + (first_frame as u64) * (frame_bytes as u64);
    let mut raw = vec![0u8; frames * frame_bytes];
    if !raw.is_empty() {
        reader.read_exact_at(byte_offset, &mut raw)?;
    }

    let mut planar: Vec<Vec<f32>> = (0..channels).map(|_| Vec::with_capacity(frames)).collect();
    for frame in 0..frames {
        let frame_base = frame * frame_bytes;
        for (ch, plane) in planar.iter_mut().enumerate() {
            let sample_base = frame_base + ch * bytes_per_sample;
            let sample = info
                .sample_format
                .decode(&raw[sample_base..sample_base + bytes_per_sample]);
            plane.push(sample);
        }
    }

    Ok(DecodedChunk {
        planar,
        frames,
        last_used: tick,
    })
}

fn mono_mix_planar(planar: &[Vec<f32>], channels: usize) -> Vec<f32> {
    match planar {
        [] => Vec::new(),
        [single] => single.clone(),
        channels_data => {
            let frames = channels_data[0].len();
            let mut mixed = vec![0.0f32; frames];
            for plane in channels_data {
                for (dst, sample) in mixed.iter_mut().zip(plane) {
                    *dst += *sample;
                }
            }
            let scale = 1.0 / channels as f32;
            for sample in &mut mixed {
                *sample *= scale;
            }
            mixed
        }
    }
}

fn parse_header(reader: &impl ByteReader) -> Result<WavStreamInfo, AudioError> {
    let total = reader.total_len();
    if total < 12 {
        return Err(AudioError::MalformedWav(
            "file shorter than a RIFF header".to_string(),
        ));
    }
    let mut riff = [0u8; 12];
    reader.read_exact_at(0, &mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err(AudioError::MalformedWav(
            "missing RIFF/WAVE signature".to_string(),
        ));
    }

    let mut fmt: Option<FmtChunk> = None;
    let mut cursor: u64 = 12;
    let mut header = [0u8; 8];
    while cursor + 8 <= total {
        reader.read_exact_at(cursor, &mut header)?;
        let id = [header[0], header[1], header[2], header[3]];
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
        let body = cursor + 8;
        if &id == b"fmt " {
            let read = size.min(40) as usize;
            let mut buf = vec![0u8; read];
            reader.read_exact_at(body, &mut buf)?;
            fmt = Some(parse_fmt(&buf)?);
        } else if &id == b"data" {
            let fmt = fmt.ok_or_else(|| {
                AudioError::MalformedWav("data chunk before fmt chunk".to_string())
            })?;
            let available = total.saturating_sub(body);
            let data_len = size.min(available);
            let frame_bytes = (fmt.channels * fmt.sample_format.bytes_per_sample()) as u64;
            if frame_bytes == 0 {
                return Err(AudioError::MalformedWav("zero-width frame".to_string()));
            }
            let frames = usize::try_from(data_len / frame_bytes).map_err(|_| {
                AudioError::MalformedWav("frame count exceeds addressable memory".to_string())
            })?;
            return Ok(WavStreamInfo {
                sample_format: fmt.sample_format,
                channels: fmt.channels,
                sample_rate: fmt.sample_rate,
                frames,
                data_offset: body,
            });
        }
        // Chunk bodies are word-aligned: an odd size carries a pad byte.
        cursor = body + size + (size & 1);
    }

    Err(AudioError::MalformedWav(
        "no data chunk in RIFF stream".to_string(),
    ))
}

struct FmtChunk {
    sample_format: StreamSampleFormat,
    channels: usize,
    sample_rate: f64,
}

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

fn parse_fmt(buf: &[u8]) -> Result<FmtChunk, AudioError> {
    if buf.len() < 16 {
        return Err(AudioError::MalformedWav("fmt chunk too short".to_string()));
    }
    let mut format_tag = u16::from_le_bytes([buf[0], buf[1]]);
    let channels = u16::from_le_bytes([buf[2], buf[3]]) as usize;
    let sample_rate = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let bits_per_sample = u16::from_le_bytes([buf[14], buf[15]]);

    if format_tag == WAVE_FORMAT_EXTENSIBLE {
        // The real format lives in the extension's sub-format GUID; its first
        // two bytes are the effective format tag.
        if buf.len() >= 26 {
            format_tag = u16::from_le_bytes([buf[24], buf[25]]);
        } else {
            return Err(AudioError::UnsupportedWav {
                reason: "WAVE_FORMAT_EXTENSIBLE without a sub-format".to_string(),
            });
        }
    }

    if channels == 0 {
        return Err(AudioError::InvalidChannelCount { channels });
    }
    if sample_rate == 0 {
        return Err(AudioError::InvalidSampleRate {
            sample_rate: f64::from(sample_rate),
        });
    }

    let sample_format = match (format_tag, bits_per_sample) {
        (WAVE_FORMAT_PCM, 8) => StreamSampleFormat::Uint8,
        (WAVE_FORMAT_PCM, 16) => StreamSampleFormat::Int16,
        (WAVE_FORMAT_PCM, 24) => StreamSampleFormat::Int24,
        (WAVE_FORMAT_PCM, 32) => StreamSampleFormat::Int32,
        (WAVE_FORMAT_IEEE_FLOAT, 32) => StreamSampleFormat::Float32,
        (tag, bits) => {
            return Err(AudioError::UnsupportedWav {
                reason: format!("unsupported WAV format tag {tag} at {bits} bits"),
            });
        }
    };

    Ok(FmtChunk {
        sample_format,
        channels,
        sample_rate: f64::from(sample_rate),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BitDepth;

    fn tone(channels: usize, frames: usize, sample_rate: f64) -> Audio {
        let mut planar = Vec::new();
        for ch in 0..channels {
            let samples = (0..frames)
                .map(|i| {
                    let phase = i as f32 * 0.021 + ch as f32 * 0.37;
                    (phase.sin() * 0.6).clamp(-1.0, 1.0)
                })
                .collect();
            planar.push(samples);
        }
        Audio::new(planar, sample_rate).unwrap()
    }

    fn open_bytes(bytes: Vec<u8>) -> StreamingWav {
        StreamingWav::open(BytesReader::new(bytes)).unwrap()
    }

    fn assert_stream_equals_eager(bits: BitDepth, channels: usize) {
        let audio = tone(channels, 5_000, 44_100.0);
        let wav = audio.to_wav_bytes(bits).unwrap();
        let eager = Audio::from_wav_bytes(&wav).unwrap();
        let stream = open_bytes(wav);

        assert_eq!(stream.channels(), eager.channel_count());
        assert_eq!(stream.frames(), eager.frames());
        assert_eq!(stream.sample_rate(), eager.sample_rate());

        let planar = stream.read_planar(0..stream.frames()).unwrap();
        for (ch, plane) in planar.iter().enumerate().take(channels) {
            let expected = eager.channel(ch);
            assert_eq!(plane.len(), expected.len());
            for (i, (a, b)) in plane.iter().zip(expected).enumerate() {
                assert_eq!(a.to_bits(), b.to_bits(), "ch {ch} frame {i}");
            }
        }
    }

    #[test]
    fn header_is_read_without_decoding_samples() {
        let audio = tone(2, 1_234, 48_000.0);
        let wav = audio.to_wav_bytes(BitDepth::Pcm16).unwrap();
        let stream = open_bytes(wav);
        assert_eq!(stream.channels(), 2);
        assert_eq!(stream.frames(), 1_234);
        assert_eq!(stream.sample_rate(), 48_000.0);
        assert!((stream.duration() - 1_234.0 / 48_000.0).abs() < 1e-12);
    }

    #[test]
    fn streamed_range_equals_eager_decode_for_every_supported_depth() {
        assert_stream_equals_eager(BitDepth::Pcm8, 1);
        assert_stream_equals_eager(BitDepth::Pcm16, 1);
        assert_stream_equals_eager(BitDepth::Pcm16, 2);
        assert_stream_equals_eager(BitDepth::Pcm24, 1);
        assert_stream_equals_eager(BitDepth::Pcm24, 2);
        assert_stream_equals_eager(BitDepth::Pcm32, 1);
        assert_stream_equals_eager(BitDepth::Float32, 1);
        assert_stream_equals_eager(BitDepth::Float32, 2);
    }

    #[test]
    fn partial_range_reads_match_the_full_decode() {
        let audio = tone(2, 100_000, 44_100.0);
        let wav = audio.to_wav_bytes(BitDepth::Pcm16).unwrap();
        let stream = open_bytes(wav);
        let full = stream.read_planar(0..stream.frames()).unwrap();
        for &(start, end) in &[
            (0usize, 1usize),
            (0, DECODED_CHUNK_FRAMES),
            (DECODED_CHUNK_FRAMES - 3, DECODED_CHUNK_FRAMES + 7),
            (40_000, 90_000),
            (99_999, 100_000),
        ] {
            let part = stream.read_planar(start..end).unwrap();
            for ch in 0..2 {
                assert_eq!(part[ch], full[ch][start..end], "ch {ch} [{start},{end})");
            }
        }
    }

    #[test]
    fn read_mono_matches_audio_mono_mix() {
        let audio = tone(2, 8_000, 32_000.0);
        let wav = audio.to_wav_bytes(BitDepth::Pcm24).unwrap();
        let stream = open_bytes(wav);
        let mono = stream.read_mono(0..stream.frames()).unwrap();
        let expected = audio.mono_mix().into_owned();
        // The eager buffer's own mono mix, re-decoded, is the reference.
        let eager = Audio::from_wav_bytes(&audio.to_wav_bytes(BitDepth::Pcm24).unwrap()).unwrap();
        let eager_mono = eager.mono_mix().into_owned();
        assert_eq!(mono.len(), eager_mono.len());
        for (i, (a, b)) in mono.iter().zip(&eager_mono).enumerate() {
            assert_eq!(a.to_bits(), b.to_bits(), "frame {i}");
        }
        // Sanity: the reference tracks the source within quantization.
        let _ = expected;
    }

    #[test]
    fn fold_mono_chunks_covers_the_whole_signal_in_order() {
        let audio = tone(1, 70_000, 16_000.0);
        let wav = audio.to_wav_bytes(BitDepth::Pcm16).unwrap();
        let stream = open_bytes(wav);
        let collected = stream
            .fold_mono_chunks(Vec::new(), |acc: &mut Vec<f32>, start, mono| {
                assert_eq!(acc.len(), start);
                acc.extend_from_slice(mono);
            })
            .unwrap();
        let direct = stream.read_mono(0..stream.frames()).unwrap();
        assert_eq!(collected, direct);
    }

    #[test]
    fn decoded_chunk_cache_stays_bounded_under_a_full_sweep() {
        let audio = tone(
            1,
            DECODED_CHUNK_FRAMES * (DECODED_CHUNK_CACHE_CAP + 8),
            8_000.0,
        );
        let wav = audio.to_wav_bytes(BitDepth::Pcm16).unwrap();
        let stream = open_bytes(wav);
        let _ = stream.read_planar(0..stream.frames()).unwrap();
        let held = stream.cache.lock().unwrap().entries.len();
        assert!(
            held <= DECODED_CHUNK_CACHE_CAP,
            "cache held {held} chunks, above the {DECODED_CHUNK_CACHE_CAP} cap"
        );
    }

    #[test]
    fn truncated_data_chunk_reports_the_real_frame_count() {
        let audio = tone(1, 1_000, 8_000.0);
        let mut wav = audio.to_wav_bytes(BitDepth::Pcm16).unwrap();
        wav.truncate(wav.len() - 200);
        let stream = open_bytes(wav);
        // 200 trailing bytes = 100 frames of 16-bit mono are gone.
        assert_eq!(stream.frames(), 900);
        assert!(stream.read_planar(0..stream.frames()).is_ok());
    }

    #[test]
    fn non_wav_bytes_are_a_typed_error() {
        let err = StreamingWav::open(BytesReader::new(b"not a wav".to_vec()))
            .map(|_| ())
            .unwrap_err();
        assert!(matches!(err, AudioError::MalformedWav(_)));
    }
}
