//! Bounded min/max waveform pyramid over a streamed audio source.
//!
//! The eager [`crate::pyramid::Pyramid`] stores one [`MinMax`] per sample, which
//! is exact and fast but costs eight bytes a sample — about 2 GB for an hour of
//! 48 kHz mono, more than the decoded signal it summarizes. That is acceptable
//! for the short buffers the eager path holds, but not for the streamed
//! [`StreamingWav`] case this module serves.
//!
//! Here the base level instead holds one [`MinMax`] per fixed block of
//! [`BASE_BUCKET`] samples, built in a single streaming pass that never holds
//! the signal at once. An hour of mono collapses to a few megabytes of pyramid.
//! A query wider than a bucket reads whole buckets off the pyramid; the ragged
//! sample runs at each end of the query — always shorter than one bucket — read
//! raw samples back through the source's decoded-chunk cache. The two paths
//! combined give the exact minimum and maximum over the queried sample range,
//! bit-for-bit equal to a direct scan and to the eager pyramid.

use phx_audio::{AudioError, StreamingWav};
use phx_dsp::next_pow2;

use crate::pyramid::MinMax;

/// Samples summarized by one base-level pyramid entry.
///
/// A power of two so higher levels tile the base exactly, and an even divisor of
/// [`phx_audio::DECODED_CHUNK_FRAMES`] so a streamed decode chunk never splits a
/// bucket during the build pass.
pub(crate) const BASE_BUCKET: usize = 256;

/// Bounded min/max mip pyramid whose base entry covers [`BASE_BUCKET`] samples.
///
/// Level 0 is the base buckets, right-padded with [`MinMax::NEUTRAL`] to a power
/// of two so every higher level tiles exactly; each higher level pairwise
/// combines the one below. Sub-bucket precision at a query's edges comes from
/// raw ranged reads against the [`StreamingWav`] the pyramid was built over.
pub(crate) struct StreamPyramid {
    levels: Vec<Vec<MinMax>>,
    sample_count: usize,
    sample_rate: f64,
}

impl StreamPyramid {
    /// Builds the pyramid in one streaming pass over the source's mono mix.
    ///
    /// Memory stays bounded: the pass holds one decode chunk at a time (through
    /// the source cache) plus the base buckets it is filling.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] when the backing store fails mid-pass.
    pub(crate) fn build(source: &StreamingWav) -> Result<Self, AudioError> {
        let sample_count = source.frames();
        let sample_rate = source.sample_rate();
        let bucket_count = sample_count.div_ceil(BASE_BUCKET).max(1);

        let base = source.fold_mono_chunks(
            vec![MinMax::NEUTRAL; bucket_count],
            |buckets, start, mono| {
                for (i, &sample) in mono.iter().enumerate() {
                    let bucket = (start + i) / BASE_BUCKET;
                    buckets[bucket] = buckets[bucket].combine(MinMax::from_sample(sample));
                }
            },
        )?;

        let padded_len = next_pow2(base.len());
        let mut level0 = base;
        level0.resize(padded_len, MinMax::NEUTRAL);

        let mut levels = vec![level0];
        while levels.last().is_some_and(|level| level.len() > 1) {
            let next = levels
                .last()
                .expect("levels is non-empty")
                .chunks_exact(2)
                .map(|pair| pair[0].combine(pair[1]))
                .collect();
            levels.push(next);
        }

        Ok(Self {
            levels,
            sample_count,
            sample_rate,
        })
    }

    /// Computes `px` min/max buckets covering `[t0, t1)` seconds.
    ///
    /// Matches [`crate::pyramid::Pyramid::slice`] exactly: `t0`/`t1` are clamped
    /// to the signal duration and swapped if reversed, buckets split the clamped
    /// span evenly, and a bucket rounding to zero samples widens to one so
    /// silence never reads as an empty range. Each bucket's min/max is exact.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] when a sub-bucket edge read fails.
    pub(crate) fn slice(
        &self,
        source: &StreamingWav,
        t0: f64,
        t1: f64,
        px: u32,
    ) -> Result<Vec<MinMax>, AudioError> {
        let px = px as usize;
        if px == 0 {
            return Ok(Vec::new());
        }
        if self.sample_count == 0 {
            return Ok(vec![MinMax { min: 0.0, max: 0.0 }; px]);
        }

        let duration = self.sample_count as f64 / self.sample_rate;
        let (t0, t1) = (
            t0.min(t1).clamp(0.0, duration),
            t0.max(t1).clamp(0.0, duration),
        );

        let mut out = Vec::with_capacity(px);
        for i in 0..px {
            let frac0 = i as f64 / px as f64;
            let frac1 = (i + 1) as f64 / px as f64;
            let start = self.sample_index(t0 + frac0 * (t1 - t0));
            let mut end = self.sample_index(t0 + frac1 * (t1 - t0)).max(start);
            if end == start && start < self.sample_count {
                end = start + 1;
            }
            out.push(self.range(source, start, end)?);
        }
        Ok(out)
    }

    /// Exact min/max over the half-open real-sample range `[start, end)`.
    ///
    /// The aligned interior reads whole buckets off the pyramid; the shorter-
    /// than-a-bucket runs at each end read raw samples through the source cache.
    ///
    /// # Errors
    /// Returns [`AudioError::Io`] when an edge read fails.
    fn range(&self, source: &StreamingWav, start: usize, end: usize) -> Result<MinMax, AudioError> {
        debug_assert!(start <= end && end <= self.sample_count);
        if start >= end {
            return Ok(MinMax::NEUTRAL);
        }

        let aligned_start = start.div_ceil(BASE_BUCKET) * BASE_BUCKET;
        let aligned_end = (end / BASE_BUCKET) * BASE_BUCKET;

        if aligned_start >= aligned_end {
            // The range spans no whole bucket boundary; read it raw.
            return self.raw_range(source, start, end);
        }

        let mut acc = MinMax::NEUTRAL;
        if start < aligned_start {
            acc = acc.combine(self.raw_range(source, start, aligned_start)?);
        }
        acc =
            acc.combine(self.bucket_range(aligned_start / BASE_BUCKET, aligned_end / BASE_BUCKET));
        if aligned_end < end {
            acc = acc.combine(self.raw_range(source, aligned_end, end)?);
        }
        Ok(acc)
    }

    /// Min/max over the raw samples in `[start, end)`, read through the source.
    ///
    /// Only ever called for runs shorter than [`BASE_BUCKET`], so the read stays
    /// small and cache-friendly.
    fn raw_range(
        &self,
        source: &StreamingWav,
        start: usize,
        end: usize,
    ) -> Result<MinMax, AudioError> {
        let mono = source.read_mono(start..end)?;
        let mut acc = MinMax::NEUTRAL;
        for &sample in &mono {
            acc = acc.combine(MinMax::from_sample(sample));
        }
        Ok(acc)
    }

    /// Combines whole base buckets `[bucket_start, bucket_end)` off the pyramid.
    ///
    /// The descent mirrors [`crate::pyramid::Pyramid::range`] but in bucket-index
    /// space: it climbs to the largest aligned level whose block stays inside the
    /// bucket range, so it visits `O(log n)` entries. Every bucket it touches is
    /// a real, fully-populated bucket, so the padding beyond the true bucket
    /// count is never read.
    fn bucket_range(&self, bucket_start: usize, bucket_end: usize) -> MinMax {
        let mut acc = MinMax::NEUTRAL;
        let mut pos = bucket_start;
        while pos < bucket_end {
            let mut level = 0usize;
            while level + 1 < self.levels.len() {
                let block = 1usize << (level + 1);
                if pos.is_multiple_of(block) && pos + block <= bucket_end {
                    level += 1;
                } else {
                    break;
                }
            }
            let block = 1usize << level;
            let idx = pos >> level;
            acc = acc.combine(self.levels[level][idx]);
            pos += block;
        }
        acc
    }

    fn sample_index(&self, time: f64) -> usize {
        (time * self.sample_rate)
            .round()
            .clamp(0.0, self.sample_count as f64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyramid::Pyramid;
    use phx_audio::{Audio, BitDepth, BytesReader};

    fn ramp_wav(sample_count: usize, sample_rate: f64) -> (Audio, Vec<u8>) {
        let samples: Vec<f32> = (0..sample_count)
            .map(|i| ((i as f32 * 0.013).sin() * 0.9).clamp(-1.0, 1.0))
            .collect();
        let audio = Audio::new(vec![samples], sample_rate).unwrap();
        let wav = audio.to_wav_bytes(BitDepth::Float32).unwrap();
        (audio, wav)
    }

    fn open(wav: Vec<u8>) -> StreamingWav {
        StreamingWav::open(BytesReader::new(wav)).unwrap()
    }

    #[test]
    fn stream_slice_equals_eager_pyramid_bit_for_bit() {
        // A non-bucket-multiple length exercises the ragged final bucket and the
        // sub-bucket edge reads at several zoom levels.
        let (audio, wav) = ramp_wav(BASE_BUCKET * 20 + 37, 8_000.0);
        let eager = Pyramid::build(&audio);
        let source = open(wav);
        let stream = StreamPyramid::build(&source).unwrap();

        let duration = audio.duration();
        for &px in &[1u32, 7, 64, 200, 1000] {
            for &(a, b) in &[
                (0.0, duration),
                (0.0, duration * 0.5),
                (duration * 0.2, duration * 0.9),
                (duration * 0.49, duration * 0.51),
                (0.0, 0.0005),
            ] {
                let expected = eager.slice(a, b, px);
                let actual = stream.slice(&source, a, b, px).unwrap();
                assert_eq!(actual.len(), expected.len(), "px {px} span {a}..{b}");
                for (i, (x, y)) in actual.iter().zip(&expected).enumerate() {
                    assert_eq!(
                        x.min.to_bits(),
                        y.min.to_bits(),
                        "px {px} span {a}..{b} bucket {i}"
                    );
                    assert_eq!(
                        x.max.to_bits(),
                        y.max.to_bits(),
                        "px {px} span {a}..{b} bucket {i}"
                    );
                }
            }
        }
    }

    #[test]
    fn stream_slice_matches_eager_on_a_single_bucket_length() {
        let (audio, wav) = ramp_wav(BASE_BUCKET, 8_000.0);
        let eager = Pyramid::build(&audio);
        let source = open(wav);
        let stream = StreamPyramid::build(&source).unwrap();
        let d = audio.duration();
        let expected = eager.slice(0.0, d, 300);
        let actual = stream.slice(&source, 0.0, d, 300).unwrap();
        for (x, y) in actual.iter().zip(&expected) {
            assert_eq!(x.min.to_bits(), y.min.to_bits());
            assert_eq!(x.max.to_bits(), y.max.to_bits());
        }
    }

    #[test]
    fn empty_source_yields_silent_buckets() {
        let audio = Audio::new(vec![Vec::<f32>::new()], 8_000.0).unwrap();
        let wav = audio.to_wav_bytes(BitDepth::Float32).unwrap();
        let source = open(wav);
        let stream = StreamPyramid::build(&source).unwrap();
        let slice = stream.slice(&source, 0.0, 1.0, 4).unwrap();
        assert_eq!(slice.len(), 4);
        assert!(slice.iter().all(|m| m.min == 0.0 && m.max == 0.0));
    }
}
