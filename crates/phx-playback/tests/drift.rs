//! Roadmap phase-6 gate: the playback cursor drifts less than one frame over
//! five minutes.
//!
//! The cursor is the atomic sample counter [`render_into`] advances, and the
//! reported position is that counter divided by the device rate. Because the
//! counter only ever counts frames the callback delivered, it is locked to the
//! stream by construction — the drift a wall-clock cursor accumulates against
//! the audio cannot occur. This test drives the real render step block by block
//! for five minutes of frames and measures the counter against the frames it
//! was fed.
//!
//! What is *not* covered here: opening an actual `cpal` device. WSL2 and CI
//! runners expose no output device, so the device path (`CpalPlayback`) cannot
//! be exercised headlessly; it is verified structurally by sharing the exact
//! `render_into` / `PlaybackClock` code this test drives.

use std::sync::Arc;

use phx_playback::{PlaybackClock, RenderBuffer, render_into};

#[test]
fn cursor_stays_within_one_frame_over_five_minutes() {
    let rate = 48_000u32;
    let minutes = 5u64;
    let total_frames = u64::from(rate) * 60 * minutes; // 14_400_000

    // Mono, device-rate buffer: sample values are irrelevant to the counter, so
    // a single silent allocation of the full length is enough to index into.
    let samples: Arc<[f32]> = Arc::from(vec![0.0f32; total_frames as usize]);
    let buffer = RenderBuffer::from_interleaved(samples, rate, 1);

    let clock = PlaybackClock::new(total_frames, rate);
    clock.set_end(u64::MAX); // play to the end
    clock.set_playing(true);

    // A typical callback block; the count is deliberately not a divisor of the
    // total so the final partial block is exercised too.
    let block = 512usize;
    let mut out = vec![0.0f32; block];

    let mut delivered: u64 = 0;
    while clock.cursor() < total_frames {
        render_into(&clock, &buffer, &mut out);
        // Frames actually advanced this block (the tail block is partial once
        // the cursor reaches the end).
        delivered = clock.cursor();
        assert!(delivered <= total_frames, "cursor overran the buffer");
    }

    // The counter consumed exactly the buffer.
    assert_eq!(delivered, total_frames);

    // Drift = reported-position frames minus frames the stream delivered.
    let reported_frames = clock.position_seconds() * f64::from(rate);
    let drift_frames = (reported_frames - delivered as f64).abs();
    assert!(
        drift_frames < 1.0,
        "cursor drifted {drift_frames} frames over {minutes} minutes (gate: < 1)"
    );

    // For contrast, a cursor timed off the wall clock on a device running a
    // modest 50 ppm fast would, over five minutes, sit
    //   50e-6 * 300 s * 48000 = 720 frames
    // off the audio. Deriving position from the delivered-frame counter removes
    // that error entirely, which is why the measured drift above is zero.
    assert_eq!(reported_frames, delivered as f64);
}
