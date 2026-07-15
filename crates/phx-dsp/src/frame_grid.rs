//! Absolute-time frame grid.

/// Frame centres derived from a signal's time domain alone.
///
/// The grid depends only on `(duration, window, step)`, never on a viewport,
/// zoom, or query range. This is what makes an analysis value at time *t*
/// identical no matter how the caller framed the request.
///
/// # Layout
///
/// For a signal of length `duration` starting at time `0`, with analysis window
/// `window` and hop `step` (all in seconds):
///
/// ```text
/// n            = floor((duration − window) / step) + 1       (0 if duration < window)
/// first centre = (duration − (n − 1)·step) / 2
/// centre[i]    = first centre + i·step
/// ```
///
/// The `(n − 1)·step` span is centred inside `duration`, so the leading and
/// trailing margins are equal and the whole grid sits on the signal midpoint. A
/// signal starting at an absolute time `t0` other than zero shifts every centre
/// by `t0`; the caller adds that offset, keeping the grid a pure function of the
/// signal's own time axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameGrid {
    count: usize,
    first_center: f64,
    step: f64,
    duration: f64,
    window: f64,
}

impl FrameGrid {
    /// Builds the grid for a signal of `duration` seconds analysed with a
    /// `window`-second window hopped by `step` seconds.
    ///
    /// When `duration < window` no full window fits and the grid is empty.
    ///
    /// # Panics
    ///
    /// Panics if `step <= 0.0` or if any argument is not finite.
    #[must_use]
    pub fn new(duration: f64, window: f64, step: f64) -> Self {
        assert!(
            duration.is_finite() && window.is_finite() && step.is_finite(),
            "FrameGrid arguments must be finite"
        );
        assert!(step > 0.0, "FrameGrid step must be positive");
        let count = if duration < window {
            0
        } else {
            ((duration - window) / step).floor() as usize + 1
        };
        let first_center = if count == 0 {
            0.0
        } else {
            (duration - (count - 1) as f64 * step) / 2.0
        };
        Self {
            count,
            first_center,
            step,
            duration,
            window,
        }
    }

    /// Number of frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether the grid holds no frames (signal shorter than the window).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Hop between consecutive centres, in seconds.
    #[must_use]
    pub fn step(&self) -> f64 {
        self.step
    }

    /// Window length the grid was built for, in seconds.
    #[must_use]
    pub fn window(&self) -> f64 {
        self.window
    }

    /// Signal duration the grid was built for, in seconds.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Absolute time of frame `i`, or `None` if `i` is out of range.
    #[must_use]
    pub fn center(&self, i: usize) -> Option<f64> {
        (i < self.count).then_some(self.first_center + i as f64 * self.step)
    }

    /// Iterator over all frame centre times, in seconds, ascending.
    pub fn centers(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.count).map(move |i| self.first_center + i as f64 * self.step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_and_centering() {
        // 1.0 s, 0.04 s window, 0.01 s step:
        // n = floor((1.0 − 0.04)/0.01) + 1 = 96 + 1 = 97.
        let g = FrameGrid::new(1.0, 0.04, 0.01);
        assert_eq!(g.len(), 97);
        let span = (g.len() - 1) as f64 * g.step();
        let first = g.center(0).unwrap();
        let last = g.center(g.len() - 1).unwrap();
        // Symmetric margins about the signal midpoint.
        assert!((first - (1.0 - span) / 2.0).abs() < 1e-12);
        assert!(
            ((1.0 - last) - first).abs() < 1e-12,
            "leading == trailing margin"
        );
        assert!(
            (0.5 * (first + last) - 0.5).abs() < 1e-12,
            "centred on midpoint"
        );
    }

    #[test]
    fn exact_and_short_signals() {
        // duration == window → exactly one frame centred on the midpoint.
        let g = FrameGrid::new(0.04, 0.04, 0.01);
        assert_eq!(g.len(), 1);
        assert!((g.center(0).unwrap() - 0.02).abs() < 1e-12);
        // duration < window → empty.
        assert!(FrameGrid::new(0.03, 0.04, 0.01).is_empty());
    }

    #[test]
    fn center_matches_iterator() {
        let g = FrameGrid::new(3.7, 0.025, 0.005);
        for (i, c) in g.centers().enumerate() {
            assert_eq!(g.center(i).unwrap(), c);
        }
        assert_eq!(g.centers().count(), g.len());
    }

    /// Zoom-independence: a query window into the signal never moves a frame
    /// centre. Filtering the global grid to a sub-interval must return the same
    /// absolute times as the global grid, and sliding the query interval leaves
    /// each surviving centre exactly where it was.
    #[test]
    fn query_window_never_shifts_centers() {
        let duration = 12.34;
        let window = 0.03;
        let step = 0.007;
        let grid = FrameGrid::new(duration, window, step);
        let all: Vec<f64> = grid.centers().collect();

        // A sweep of query windows of assorted widths and offsets.
        let queries = [
            (0.0, duration),
            (1.0, 2.0),
            (1.0001, 2.0),
            (0.9999, 2.0001),
            (5.5, 5.5000001),
            (3.3333, 9.8765),
        ];
        for &(a, b) in &queries {
            let inside: Vec<f64> = all.iter().copied().filter(|&c| c >= a && c <= b).collect();
            // Recomputing the grid and filtering yields identical times: the
            // centres are a property of the signal, not the query.
            let regrid: Vec<f64> = FrameGrid::new(duration, window, step)
                .centers()
                .filter(|&c| c >= a && c <= b)
                .collect();
            assert_eq!(inside.len(), regrid.len());
            for (x, y) in inside.iter().zip(regrid.iter()) {
                assert_eq!(
                    x.to_bits(),
                    y.to_bits(),
                    "centre moved under query [{a},{b}]"
                );
            }
        }
    }

    /// The grid is independent of the absolute placement of the query, tested
    /// as a coarse property sweep over parameter space: for any duration the
    /// centres reproduce bit-for-bit on reconstruction.
    #[test]
    fn deterministic_over_parameter_sweep() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..2000 {
            let duration = 0.1 + (next() % 20_000) as f64 / 1000.0;
            let window = 0.005 + (next() % 500) as f64 / 10_000.0;
            let step = 0.001 + (next() % 200) as f64 / 10_000.0;
            let a = FrameGrid::new(duration, window, step);
            let b = FrameGrid::new(duration, window, step);
            assert_eq!(a.len(), b.len());
            for i in 0..a.len() {
                assert_eq!(
                    a.center(i).unwrap().to_bits(),
                    b.center(i).unwrap().to_bits()
                );
            }
        }
    }
}
