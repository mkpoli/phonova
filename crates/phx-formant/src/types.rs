use phx_dsp::FrameGrid;

use crate::FormantParams;

/// A single candidate formant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormantPoint {
    /// Formant frequency in hertz.
    pub frequency: f64,
    /// Formant bandwidth in hertz.
    pub bandwidth: f64,
}

/// Candidate formants at one analysis frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FormantFrame {
    /// Frame centre time in seconds.
    pub time: f64,
    /// Candidate formants sorted by ascending frequency.
    pub formants: Vec<FormantPoint>,
}

/// Burg formant candidates over an audio view.
///
/// Frames are in ascending time order. The frame grid stores the audio
/// duration, effective Gaussian window length, and hop used for analysis, so
/// callers can reconstruct the same absolute-time frame placement. The raw
/// frames retain the frequency-gated roots produced by LPC analysis, sorted by
/// frequency and capped at `params.max_formants`; [`crate::track_smoothed`]
/// reassigns those candidates to formant slots with Viterbi tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct FormantTrack {
    /// Candidate frames in ascending time order.
    pub frames: Vec<FormantFrame>,
    /// Parameters used for analysis.
    pub params: FormantParams,
    /// Analysed audio duration in seconds.
    pub duration: f64,
    /// Frame grid used to place frame centres.
    pub frame_grid: FrameGrid,
}
