/// A pitch candidate for one analysis frame.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchCandidate {
    /// Candidate frequency in hertz; `0.0` marks the explicit unvoiced candidate.
    pub frequency: f64,
    /// Candidate strength `R`: Boersma eq. 24 for voiced candidates and eq. 23
    /// for the unvoiced candidate.
    pub strength: f64,
}

/// One frame of a pitch track.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchFrame {
    /// Frame centre time in seconds.
    pub time: f64,
    /// Selected fundamental frequency in hertz, or `None` when the path is unvoiced.
    pub f0: Option<f64>,
    /// Strength stored on the selected candidate.
    pub strength: f64,
    /// All candidates generated for the frame, including the unvoiced candidate.
    pub candidates: Vec<PitchCandidate>,
}

/// A closed time interval `[start, end]` in seconds.
///
/// This crate-local type scopes pitch statistics until the workspace gains a
/// shared time-domain interval type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSpan {
    /// Inclusive start time in seconds.
    pub start: f64,
    /// Inclusive end time in seconds.
    pub end: f64,
}

impl TimeSpan {
    /// Creates a closed interval `[start, end]` in seconds.
    ///
    /// # Panics
    ///
    /// Panics if either bound is non-finite or if `start > end`.
    #[must_use]
    pub fn new(start: f64, end: f64) -> Self {
        assert!(
            start.is_finite() && end.is_finite(),
            "TimeSpan bounds must be finite"
        );
        assert!(start <= end, "TimeSpan start must be <= end");
        Self { start, end }
    }

    /// Returns whether `t` lies inside the closed interval.
    #[must_use]
    pub fn contains(&self, t: f64) -> bool {
        t >= self.start && t <= self.end
    }
}

/// Pitch-analysis result across a frame grid.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchTrack {
    frames: Vec<PitchFrame>,
}

impl PitchTrack {
    pub(crate) fn new(frames: Vec<PitchFrame>) -> Self {
        Self { frames }
    }

    /// Returns all frames on the analysis grid.
    #[must_use]
    pub fn frames(&self) -> &[PitchFrame] {
        &self.frames
    }

    /// Mean selected voiced frequency in hertz over `span`.
    #[must_use]
    pub fn mean_hz(&self, span: TimeSpan) -> Option<f64> {
        let values = self.voiced_hz(span);
        mean(&values)
    }

    /// Median selected voiced frequency in hertz over `span`.
    #[must_use]
    pub fn median_hz(&self, span: TimeSpan) -> Option<f64> {
        let mut values = self.voiced_hz(span);
        median(&mut values)
    }

    /// Minimum selected voiced frequency in hertz over `span`.
    #[must_use]
    pub fn min_hz(&self, span: TimeSpan) -> Option<f64> {
        let values = self.voiced_hz(span);
        min(&values)
    }

    /// Maximum selected voiced frequency in hertz over `span`.
    #[must_use]
    pub fn max_hz(&self, span: TimeSpan) -> Option<f64> {
        let values = self.voiced_hz(span);
        max(&values)
    }

    /// Mean selected voiced frequency in semitones re 1 Hz over `span`.
    #[must_use]
    pub fn mean_semitones(&self, span: TimeSpan) -> Option<f64> {
        let values = self.voiced_semitones(span);
        mean(&values)
    }

    /// Median selected voiced frequency in semitones re 1 Hz over `span`.
    #[must_use]
    pub fn median_semitones(&self, span: TimeSpan) -> Option<f64> {
        let mut values = self.voiced_semitones(span);
        median(&mut values)
    }

    /// Minimum selected voiced frequency in semitones re 1 Hz over `span`.
    #[must_use]
    pub fn min_semitones(&self, span: TimeSpan) -> Option<f64> {
        let values = self.voiced_semitones(span);
        min(&values)
    }

    /// Maximum selected voiced frequency in semitones re 1 Hz over `span`.
    #[must_use]
    pub fn max_semitones(&self, span: TimeSpan) -> Option<f64> {
        let values = self.voiced_semitones(span);
        max(&values)
    }

    fn voiced_hz(&self, span: TimeSpan) -> Vec<f64> {
        self.frames
            .iter()
            .filter(|frame| span.contains(frame.time))
            .filter_map(|frame| frame.f0)
            .collect()
    }

    fn voiced_semitones(&self, span: TimeSpan) -> Vec<f64> {
        self.voiced_hz(span)
            .into_iter()
            .map(hz_to_semitones)
            .collect()
    }
}

/// Converts hertz to semitones re 1 Hz, the primitive Praat Hertz-to-semitone
/// reference used by this crate.
#[must_use]
pub fn hz_to_semitones(f0_hz: f64) -> f64 {
    12.0 * f0_hz.log2()
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        Some(0.5 * (values[mid - 1] + values[mid]))
    } else {
        Some(values[mid])
    }
}

fn min(values: &[f64]) -> Option<f64> {
    values.iter().copied().reduce(f64::min)
}

fn max(values: &[f64]) -> Option<f64> {
    values.iter().copied().reduce(f64::max)
}
