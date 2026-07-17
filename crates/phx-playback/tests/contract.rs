//! The [`PlaybackEngine`] contract, exercised headlessly through the mock
//! backend (the cpal backend shares the same clock operations).

use phx_audio::Audio;
use phx_playback::{MockPlayback, PlaybackEngine};

/// One second of mono audio at `rate`, so `load` hits the resampler's identity
/// fast path and stays cheap.
fn one_second(rate: u32) -> Audio {
    let frames = rate as usize;
    let ramp = (0..frames).map(|i| i as f32 / frames as f32).collect();
    Audio::new(vec![ramp], f64::from(rate)).unwrap()
}

#[test]
fn play_advances_and_pause_holds() {
    let rate = 8_000u32;
    let engine = MockPlayback::new(rate, 1);
    engine.load(&one_second(rate)).unwrap();

    assert_eq!(engine.position(), 0.0);
    assert!(!engine.is_playing());
    assert!((engine.duration() - 1.0).abs() < 1e-9);

    engine.play();
    engine.pump(rate as usize / 2); // half a second
    assert!(engine.is_playing());
    let mid = engine.position();
    assert!((mid - 0.5).abs() < 1e-6, "expected ~0.5s, got {mid}");

    engine.pause();
    let held = engine.position();
    engine.pump(1000); // no advance while paused
    assert_eq!(engine.position(), held);
    assert!(!engine.is_playing());
}

#[test]
fn seek_moves_the_cursor_while_paused() {
    let rate = 8_000u32;
    let engine = MockPlayback::new(rate, 1);
    engine.load(&one_second(rate)).unwrap();

    engine.seek(0.75);
    engine.pump(64); // a block lets the callback adopt the pending seek
    assert!((engine.position() - 0.75).abs() < 1e-6);
    assert!(!engine.is_playing());
}

#[test]
fn play_range_stops_at_the_span_end() {
    let rate = 8_000u32;
    let engine = MockPlayback::new(rate, 1);
    engine.load(&one_second(rate)).unwrap();

    engine.play_range(0.25, 0.5);
    // Pump well past the span; the callback must latch stopped at 0.5s.
    for _ in 0..40 {
        engine.pump(64);
    }
    assert!(!engine.is_playing(), "range playback should have stopped");
    let pos = engine.position();
    assert!(
        (pos - 0.5).abs() < 1e-3,
        "expected stop at ~0.5s, got {pos}"
    );
}

#[test]
fn stop_rewinds_to_the_start() {
    let rate = 8_000u32;
    let engine = MockPlayback::new(rate, 1);
    engine.load(&one_second(rate)).unwrap();

    engine.play();
    engine.pump(2000);
    assert!(engine.position() > 0.0);

    engine.stop();
    engine.pump(64);
    assert_eq!(engine.position(), 0.0);
    assert!(!engine.is_playing());
}

#[test]
fn play_reaching_the_end_latches_stopped() {
    let rate = 8_000u32;
    let engine = MockPlayback::new(rate, 1);
    engine.load(&one_second(rate)).unwrap();

    engine.play();
    // Pump more than the whole second; the cursor must stop at the end.
    for _ in 0..(rate as usize / 64 + 8) {
        engine.pump(64);
    }
    assert!(
        !engine.is_playing(),
        "playback should stop at the buffer end"
    );
    assert!((engine.position() - 1.0).abs() < 1e-3);
}
