//! Cached min/max mip pyramid backing [`crate::Engine::waveform_slice`].
//!
//! A waveform view at any zoom level needs, per output pixel, the exact
//! minimum and maximum sample value across the pixel's time span. Scanning
//! the raw signal for every request is the correctness baseline but is
//! `O(samples)` per call; a mip pyramid answers the same query in
//! `O(log samples)` by precomputing min/max pairs at every power-of-two
//! block size once, at import time.

use phx_audio::Audio;
use phx_dsp::next_pow2;

/// Minimum and maximum sample value observed over a span of audio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinMax {
    /// Lowest sample value in the span.
    pub min: f32,
    /// Highest sample value in the span.
    pub max: f32,
}

impl MinMax {
    /// Identity element for [`MinMax::combine`]: combining with `NEUTRAL`
    /// never changes the other operand. Used to pad the pyramid's base level
    /// to a power-of-two length without corrupting any query whose end index
    /// stays within the real sample count (see [`Pyramid::range`]).
    pub(crate) const NEUTRAL: Self = Self {
        min: f32::INFINITY,
        max: f32::NEG_INFINITY,
    };

    pub(crate) fn from_sample(sample: f32) -> Self {
        Self {
            min: sample,
            max: sample,
        }
    }

    pub(crate) fn combine(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

/// Cached min/max mip pyramid over an audio buffer's mono mix.
///
/// # Construction
///
/// Level 0 holds one [`MinMax`] per sample of the mono-mixed signal
/// (`min == max == sample`), right-padded with [`MinMax::NEUTRAL`] up to
/// `next_pow2(sample_count)` entries. Each subsequent level pairwise
/// combines the level below it (`level[k][i] = combine(level[k-1][2i],
/// level[k-1][2i+1])`), halving in length until a single entry remains.
/// Because the base level's length is a power of two, every level's length
/// is also exact — no level ever has a ragged final block — so entry `i` at
/// level `k` always covers precisely the padded-index range
/// `[i·2^k, (i+1)·2^k)`.
///
/// # Correctness
///
/// [`Pyramid::range`] only descends into a level-`k` block when the block's
/// end index is `<= end`, and callers only ever pass `end <= sample_count`
/// (the real, unpadded sample count). Every block a query touches therefore
/// lies entirely within `[0, sample_count)`, so the neutral padding beyond
/// `sample_count` — which does cover ranges `[i·2^k, (i+1)·2^k)` that may
/// extend past `sample_count` for the last real block at each level — is
/// never read by a valid query, and the combine is exact rather than
/// approximate: it agrees bit-for-bit with a direct min/max scan of the same
/// sample range.
pub struct Pyramid {
    levels: Vec<Vec<MinMax>>,
    sample_count: usize,
    sample_rate: f64,
}

impl Pyramid {
    /// Builds the pyramid from an audio buffer's mono mix.
    pub fn build(audio: &Audio) -> Self {
        let mono = audio.mono_mix();
        let sample_count = mono.len();
        let sample_rate = audio.sample_rate();
        let padded_len = next_pow2(sample_count);

        let mut base = Vec::with_capacity(padded_len);
        for i in 0..padded_len {
            base.push(match mono.get(i) {
                Some(&sample) => MinMax::from_sample(sample),
                None => MinMax::NEUTRAL,
            });
        }

        let mut levels = vec![base];
        while levels.last().is_some_and(|level| level.len() > 1) {
            let next = levels
                .last()
                .expect("levels is non-empty")
                .chunks_exact(2)
                .map(|pair| pair[0].combine(pair[1]))
                .collect();
            levels.push(next);
        }

        Self {
            levels,
            sample_count,
            sample_rate,
        }
    }

    /// Exact min/max over the half-open real-sample range `[start, end)`.
    ///
    /// Returns [`MinMax::NEUTRAL`] for an empty range.
    ///
    /// # Panics
    /// Panics if `start > end` or `end > self.sample_count()` — callers must
    /// clip to the real sample domain first.
    pub fn range(&self, start: usize, end: usize) -> MinMax {
        assert!(start <= end, "range start must not exceed end");
        assert!(
            end <= self.sample_count,
            "range end must not exceed the real sample count"
        );
        if start == end {
            return MinMax::NEUTRAL;
        }

        let mut acc = MinMax::NEUTRAL;
        let mut pos = start;
        while pos < end {
            let mut level = 0usize;
            while level + 1 < self.levels.len() {
                let block = 1usize << (level + 1);
                if pos.is_multiple_of(block) && pos + block <= end {
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

    /// Computes `px` [`MinMax`] buckets covering `[t0, t1)` in seconds.
    ///
    /// `t0`/`t1` are clamped to the signal's real duration and swapped if
    /// given in reverse order. Buckets split the clamped span evenly; a
    /// bucket that would otherwise round to zero real samples is widened to
    /// one sample so silence never masquerades as an out-of-range query.
    /// Returns `px` copies of a `{0.0, 0.0}` [`MinMax`] when the pyramid
    /// holds no samples.
    pub fn slice(&self, t0: f64, t1: f64, px: u32) -> Vec<MinMax> {
        let px = px as usize;
        if px == 0 {
            return Vec::new();
        }
        if self.sample_count == 0 {
            return vec![MinMax { min: 0.0, max: 0.0 }; px];
        }

        let duration = self.sample_count as f64 / self.sample_rate;
        let (t0, t1) = (
            t0.min(t1).clamp(0.0, duration),
            t0.max(t1).clamp(0.0, duration),
        );

        (0..px)
            .map(|i| {
                let frac0 = i as f64 / px as f64;
                let frac1 = (i + 1) as f64 / px as f64;
                let start = self.sample_index(t0 + frac0 * (t1 - t0));
                let mut end = self.sample_index(t0 + frac1 * (t1 - t0)).max(start);
                if end == start && start < self.sample_count {
                    end = start + 1;
                }
                self.range(start, end)
            })
            .collect()
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

    fn ramp_audio(sample_count: usize, sample_rate: f64) -> Audio {
        let samples = (0..sample_count)
            .map(|i| (i as f32 / sample_count.max(1) as f32) * 2.0 - 1.0)
            .collect();
        Audio::new(vec![samples], sample_rate).unwrap()
    }

    fn direct_min_max(samples: &[f32], start: usize, end: usize) -> MinMax {
        let slice = &samples[start..end];
        MinMax {
            min: slice.iter().copied().fold(f32::INFINITY, f32::min),
            max: slice.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        }
    }

    #[test]
    fn range_matches_direct_scan_at_every_alignment() {
        let audio = ramp_audio(1_237, 8_000.0);
        let mono = audio.mono_mix().into_owned();
        let pyramid = Pyramid::build(&audio);

        for &(start, end) in &[
            (0usize, 1usize),
            (0, 1_237),
            (5, 5),
            (1, 512),
            (513, 1_237),
            (100, 101),
            (0, 3),
            (1_236, 1_237),
        ] {
            let expected = if start == end {
                MinMax::NEUTRAL
            } else {
                direct_min_max(&mono, start, end)
            };
            let actual = pyramid.range(start, end);
            assert_eq!(
                actual.min.to_bits(),
                expected.min.to_bits(),
                "start={start} end={end}"
            );
            assert_eq!(
                actual.max.to_bits(),
                expected.max.to_bits(),
                "start={start} end={end}"
            );
        }
    }

    #[test]
    fn slice_buckets_agree_with_direct_min_max_over_the_same_bucket_bounds() {
        let sample_rate = 8_000.0;
        let audio = ramp_audio(4_001, sample_rate);
        let mono = audio.mono_mix().into_owned();
        let pyramid = Pyramid::build(&audio);

        let px = 37;
        let t0 = 0.05;
        let t1 = 0.45;
        let slice = pyramid.slice(t0, t1, px);
        assert_eq!(slice.len() as u32, px);

        let duration = mono.len() as f64 / sample_rate;
        let (ct0, ct1) = (t0.clamp(0.0, duration), t1.clamp(0.0, duration));
        for (i, bucket) in slice.iter().enumerate() {
            let frac0 = i as f64 / px as f64;
            let frac1 = (i + 1) as f64 / px as f64;
            let start = ((ct0 + frac0 * (ct1 - ct0)) * sample_rate)
                .round()
                .clamp(0.0, mono.len() as f64) as usize;
            let mut end = ((ct0 + frac1 * (ct1 - ct0)) * sample_rate)
                .round()
                .clamp(0.0, mono.len() as f64) as usize;
            end = end.max(start);
            if end == start && start < mono.len() {
                end = start + 1;
            }
            let expected = direct_min_max(&mono, start, end);
            assert_eq!(bucket.min.to_bits(), expected.min.to_bits(), "bucket {i}");
            assert_eq!(bucket.max.to_bits(), expected.max.to_bits(), "bucket {i}");
        }
    }

    #[test]
    fn slice_reversed_time_range_is_normalized() {
        let audio = ramp_audio(2_000, 8_000.0);
        let pyramid = Pyramid::build(&audio);
        let forward = pyramid.slice(0.05, 0.15, 8);
        let reversed = pyramid.slice(0.15, 0.05, 8);
        assert_eq!(
            forward.iter().map(|m| m.min.to_bits()).collect::<Vec<_>>(),
            reversed.iter().map(|m| m.min.to_bits()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn slice_zero_pixels_is_empty() {
        let audio = ramp_audio(100, 8_000.0);
        let pyramid = Pyramid::build(&audio);
        assert!(pyramid.slice(0.0, 0.01, 0).is_empty());
    }

    #[test]
    fn empty_audio_yields_silent_buckets() {
        let audio = Audio::new(vec![Vec::new()], 8_000.0).unwrap();
        let pyramid = Pyramid::build(&audio);
        let slice = pyramid.slice(0.0, 1.0, 4);
        assert_eq!(slice.len(), 4);
        assert!(slice.iter().all(|m| m.min == 0.0 && m.max == 0.0));
    }
}
