//! dB→normalized display mapping.

/// Linear-in-dB display mapping applied before colormap lookup.
///
/// Tile values are clipped to `[max_db - dynamic_range_db, max_db]` and
/// mapped linearly onto `[0, 1]` before the [`crate::Colormap`] lookup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayMapping {
    /// Span, in dB, mapped onto the full colormap range.
    pub dynamic_range_db: f64,
    /// Fixed ceiling in dB. `None` autoscales to the maximum finite value
    /// present in the tile being colorized.
    pub max_db: Option<f64>,
}

impl Default for DisplayMapping {
    /// 50 dB dynamic range with an autoscaling ceiling.
    fn default() -> Self {
        Self {
            dynamic_range_db: 50.0,
            max_db: None,
        }
    }
}

impl DisplayMapping {
    /// Resolve the `(floor_db, ceiling_db)` clip window for a tile,
    /// autoscaling `max_db` from the tile's maximum finite value when
    /// unset. A tile with no finite values falls back to a `0.0` dB
    /// ceiling, so an all-silent tile still produces a well-defined
    /// (fully floor-colored) window instead of `NaN` propagating through
    /// the mapping.
    pub(crate) fn resolve(&self, tile_db: &[f32]) -> (f64, f64) {
        let max_db = self.max_db.unwrap_or_else(|| {
            tile_db
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .fold(f32::NEG_INFINITY, f32::max) as f64
        });
        let max_db = if max_db.is_finite() { max_db } else { 0.0 };
        (max_db - self.dynamic_range_db, max_db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_documented_defaults() {
        let m = DisplayMapping::default();
        assert_eq!(m.dynamic_range_db, 50.0);
        assert_eq!(m.max_db, None);
    }

    #[test]
    fn resolve_autoscales_to_finite_maximum() {
        let m = DisplayMapping::default();
        let (floor, ceiling) = m.resolve(&[-80.0, -10.0, f32::NEG_INFINITY]);
        assert_eq!(ceiling, -10.0);
        assert_eq!(floor, -60.0);
    }

    #[test]
    fn resolve_falls_back_to_zero_when_tile_has_no_finite_values() {
        let m = DisplayMapping::default();
        let (floor, ceiling) = m.resolve(&[f32::NEG_INFINITY, f32::NAN]);
        assert_eq!(ceiling, 0.0);
        assert_eq!(floor, -50.0);
    }

    #[test]
    fn resolve_honors_a_fixed_ceiling() {
        let m = DisplayMapping {
            dynamic_range_db: 30.0,
            max_db: Some(12.0),
        };
        let (floor, ceiling) = m.resolve(&[-1000.0]);
        assert_eq!(ceiling, 12.0);
        assert_eq!(floor, -18.0);
    }
}
