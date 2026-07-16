//! Bounded LRU cache of raw dB spectrogram column blocks.
//!
//! Spectrogram tiles split the object-level frame grid into fixed blocks of
//! [`TILE_COLS`] columns, aligned so a block's coordinates are a function of the
//! audio and analysis parameters alone. Each block holds the raw
//! `10·log10(Pa²/Hz)` values for every frequency row of its columns — the
//! expensive STFT output, before any display mapping. A colormap, theme, or
//! dynamic-range change re-colorizes the same cached dB and never recomputes the
//! transform.
//!
//! Memory: a block is `TILE_COLS × freq_rows × 4` bytes. With the 5 ms-window
//! Praat defaults the frequency grid is ≈115 rows, so a block is ≈230 KiB; the
//! [`CACHE_CAPACITY`]-entry bound holds ≈14 MiB, and a dense 5 kHz analysis with
//! ≈256 rows still stays near 32 MiB.

use std::collections::HashMap;

use phx_dsp::Window;
use phx_spectrogram::{ColumnBlock, SpectrogramParams};

/// Columns per cached block, aligned to multiples of this on the frame grid.
pub(crate) const TILE_COLS: usize = 512;

/// Maximum number of column blocks held before the least-recently-used is
/// evicted.
pub(crate) const CACHE_CAPACITY: usize = 64;

/// Cache key: audio buffer, analysis-parameter hash, and aligned block index.
///
/// The key excludes the colormap, theme, and dynamic range on purpose — those
/// are applied after the dB block is read, so changing them reuses the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BlockKey {
    pub audio: u64,
    pub params_hash: u64,
    pub block_index: usize,
}

/// Stable hash of the analysis parameters that determine a block's dB values.
#[must_use]
pub(crate) fn params_hash(params: &SpectrogramParams) -> u64 {
    // A small FNV-1a over the parameter bit patterns; order-fixed and free of
    // hash-map seeding so the same parameters always key the same block.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |bytes: &[u8]| {
        for &byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    mix(&params.window_length.to_bits().to_le_bytes());
    mix(&params.max_frequency.to_bits().to_le_bytes());
    mix(&params.time_step.to_bits().to_le_bytes());
    mix(&params.frequency_step.to_bits().to_le_bytes());
    let (tag, factor) = match params.window {
        Window::Gaussian {
            effective_len_factor,
        } => (0u8, effective_len_factor),
        Window::Hanning => (1u8, 0.0),
        Window::Kaiser { beta } => (2u8, beta),
    };
    mix(&[tag]);
    mix(&factor.to_bits().to_le_bytes());
    hash
}

/// Bounded LRU over raw dB column blocks.
///
/// Recency is tracked by a monotonically increasing tick stamped on every
/// access; eviction scans for the oldest stamp when the cache is full. The
/// capacity is small, so the linear scan on insert is cheaper than threading a
/// linked list through the map.
#[derive(Default)]
pub(crate) struct TileCache {
    entries: HashMap<BlockKey, Entry>,
    tick: u64,
}

struct Entry {
    block: ColumnBlock,
    last_used: u64,
}

impl TileCache {
    /// Returns a clone of the cached block for `key`, marking it as most
    /// recently used, or `None` on a miss.
    pub(crate) fn get(&mut self, key: BlockKey) -> Option<ColumnBlock> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.entries.get_mut(&key)?;
        entry.last_used = tick;
        Some(entry.block.clone())
    }

    /// Inserts a freshly computed block, evicting the least-recently-used entry
    /// when the cache is at capacity.
    pub(crate) fn insert(&mut self, key: BlockKey, block: ColumnBlock) {
        self.tick += 1;
        let last_used = self.tick;
        if !self.entries.contains_key(&key)
            && self.entries.len() >= CACHE_CAPACITY
            && let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(k, _)| *k)
        {
            self.entries.remove(&oldest);
        }
        self.entries.insert(key, Entry { block, last_used });
    }

    /// Number of blocks currently held. Used by tests and the perf probe.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(cols: usize) -> ColumnBlock {
        ColumnBlock {
            db: vec![0.0; cols],
            first_col: 0,
            col_count: cols,
            freq_len: 1,
        }
    }

    #[test]
    fn params_hash_ignores_nothing_that_changes_the_transform() {
        let base = SpectrogramParams::default();
        let mut other = base;
        other.window_length += 0.001;
        assert_ne!(params_hash(&base), params_hash(&other));
        assert_eq!(
            params_hash(&base),
            params_hash(&SpectrogramParams::default())
        );
    }

    #[test]
    fn evicts_least_recently_used_at_capacity() {
        let mut cache = TileCache::default();
        for i in 0..CACHE_CAPACITY {
            cache.insert(
                BlockKey {
                    audio: 0,
                    params_hash: 0,
                    block_index: i,
                },
                block(1),
            );
        }
        // Touch block 0 so it is no longer the oldest.
        assert!(
            cache
                .get(BlockKey {
                    audio: 0,
                    params_hash: 0,
                    block_index: 0
                })
                .is_some()
        );
        // Insert one more: block 1 (now the oldest) is evicted, block 0 stays.
        cache.insert(
            BlockKey {
                audio: 0,
                params_hash: 0,
                block_index: CACHE_CAPACITY,
            },
            block(1),
        );
        assert_eq!(cache.len(), CACHE_CAPACITY);
        assert!(
            cache
                .get(BlockKey {
                    audio: 0,
                    params_hash: 0,
                    block_index: 0
                })
                .is_some()
        );
        assert!(
            cache
                .get(BlockKey {
                    audio: 0,
                    params_hash: 0,
                    block_index: 1
                })
                .is_none()
        );
    }
}
