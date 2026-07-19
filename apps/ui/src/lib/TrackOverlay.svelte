<script lang="ts">
  import { untrack } from 'svelte';
  import type {
    AudioInfo,
    CoreClientLike,
    FormantTrackData,
    IntensityTrackData,
    OverlayParams,
    OverlayStats,
    PitchTrackData,
    ViewportState
  } from './types';
  import { resizeCanvas } from './rendering';

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    viewport: ViewportState;
    theme: 'light' | 'dark';
    params: OverlayParams;
    onStats?: (stats: OverlayStats) => void;
  }

  let { client, audio, viewport, theme, params, onStats }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let renderToken = $state(0);

  let pitch = $state<PitchTrackData | null>(null);
  let formant = $state<FormantTrackData | null>(null);
  let intensity = $state<IntensityTrackData | null>(null);
  // Highest voiced value from the authoritative whole-signal track, not the
  // span preview, so the clipping badge never flickers on a partial window.
  let pitchMaxHz = $state(0);
  // Increments whenever fresh pitch data (preview or full) is applied; a test
  // hook for the visible-span re-render latency.
  let pitchDataToken = $state(0);

  // Track colours carry their own dark halo, so they read over any colormap
  // in either theme without being tuned per background.
  const PITCH_COLOR = '#9cc4ff';
  const FORMANT_COLOR = '#ff5a52';
  const INTENSITY_COLOR = '#ffcc33';
  const HALO = 'rgba(4, 8, 16, 0.7)';

  // Whole-signal analysis (pitch especially) is proportional to duration; past
  // this length the auto-run is paused so a long file does not tie up the
  // worker. Viewport-following analysis for long files is a later step.
  const MAX_OVERLAY_SECONDS = 120;
  let tooLong = $derived((audio?.duration ?? 0) > MAX_OVERLAY_SECONDS);

  function reportStats() {
    onStats?.({ pitchMaxHz, formantMaxHz: formant?.maxHz ?? 0 });
  }

  // Each analysis runs over the whole signal (its frame grid is a function of
  // the audio alone), so the fetched track is reused across zoom and scroll;
  // only a parameter edit or a new file refetches. The draw pass renders the
  // visible span from the cached track, which is what makes a ceiling change
  // repaint the viewport immediately.
  $effect(() => {
    const id = audio?.id;
    const show = params.pitch.show;
    const floorHz = params.pitch.floorHz;
    const ceilingHz = params.pitch.ceilingHz;
    if (!client || id === undefined || !show || tooLong) {
      pitch = null;
      pitchMaxHz = 0;
      reportStats();
      return;
    }
    // The viewport is read untracked: a parameter edit recomputes, but a plain
    // pan or zoom reuses the whole-signal track this effect settles on.
    const previewT0 = untrack(() => viewport.t0);
    const previewT1 = untrack(() => viewport.t1);
    let cancelled = false;
    let fullArrived = false;
    // Phase 1: the visible span, rendered first (pitch is the one contour whose
    // whole-signal cost grows with duration).
    client
      .pitchTrackSpan(id, floorHz, ceilingHz, previewT0, previewT1)
      .then((track) => {
        if (cancelled || fullArrived) return;
        pitch = track;
        pitchDataToken += 1;
      })
      .catch(() => {});
    // Phase 2: the whole-signal track, which replaces the preview and drives
    // the clipping badge.
    client
      .pitchTrack(id, floorHz, ceilingHz)
      .then((track) => {
        if (cancelled) return;
        fullArrived = true;
        pitch = track;
        pitchMaxHz = track.maxHz;
        pitchDataToken += 1;
        reportStats();
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const id = audio?.id;
    const show = params.formant.show;
    const ceilingHz = params.formant.ceilingHz;
    const maxFormants = params.formant.maxFormants;
    const smoothed = params.formant.smoothed;
    if (!client || id === undefined || !show || tooLong) {
      formant = null;
      reportStats();
      return;
    }
    let cancelled = false;
    client
      .formantTrack(id, ceilingHz, maxFormants, smoothed)
      .then((track) => {
        if (cancelled) return;
        formant = track;
        reportStats();
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const id = audio?.id;
    const show = params.intensity.show;
    const floorHz = params.intensity.floorHz;
    if (!client || id === undefined || !show || tooLong) {
      intensity = null;
      return;
    }
    let cancelled = false;
    client
      .intensityTrack(id, floorHz)
      .then((track) => {
        if (cancelled) return;
        intensity = track;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    if (!canvas) return;
    const observer = new ResizeObserver(() => scheduleDraw());
    observer.observe(canvas);
    scheduleDraw();
    return () => observer.disconnect();
  });

  $effect(() => {
    // Redraw when the viewport, theme, tracks, or scale bounds change.
    viewport.t0;
    viewport.t1;
    viewport.f0;
    viewport.f1;
    theme;
    pitch;
    formant;
    intensity;
    params.pitch.ceilingHz;
    params.formant.mark;
    scheduleDraw();
  });

  function scheduleDraw() {
    requestAnimationFrame(() => draw());
  }

  function timeToX(time: number, width: number) {
    return ((time - viewport.t0) / (viewport.t1 - viewport.t0)) * width;
  }

  function freqToY(freq: number, height: number) {
    const span = Math.max(1, viewport.f1 - viewport.f0);
    return height * (1 - (freq - viewport.f0) / span);
  }

  function draw() {
    if (!canvas) return;
    const { width, height } = resizeCanvas(canvas);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, width, height);
    if (!audio) {
      renderToken += 1;
      return;
    }
    if (tooLong) {
      drawNote(ctx, width, `Overlays paused above ${MAX_OVERLAY_SECONDS}s`);
      renderToken += 1;
      return;
    }

    if (params.intensity.show && intensity) drawIntensity(ctx, width, height);
    if (params.formant.show && formant) drawFormants(ctx, width, height);
    if (params.pitch.show && pitch) {
      drawPitch(ctx, width, height);
      drawPitchAxis(ctx, width, height);
    }
    renderToken += 1;
  }

  function drawNote(ctx: CanvasRenderingContext2D, width: number, text: string) {
    ctx.font = '12px ui-sans-serif, system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    ctx.lineWidth = 3;
    ctx.strokeStyle = HALO;
    ctx.strokeText(text, width / 2, 8);
    ctx.fillStyle = '#f1f5f9';
    ctx.fillText(text, width / 2, 8);
  }

  function drawFormants(ctx: CanvasRenderingContext2D, width: number, height: number) {
    if (params.formant.mark === 'track') {
      drawFormantTracks(ctx, width, height, groupFormantFrames(formant!.points));
      return;
    }
    const points = formant!.points;
    ctx.strokeStyle = HALO;
    ctx.lineWidth = 1;
    for (let i = 0; i < points.length; i += 3) {
      const time = points[i];
      if (time < viewport.t0 || time > viewport.t1) continue;
      const freq = points[i + 1];
      if (freq < viewport.f0 || freq > viewport.f1) continue;
      const bandwidth = points[i + 2];
      const x = timeToX(time, width);
      const y = freqToY(freq, height);
      // Wider bandwidth reads as a larger, fuzzier speckle.
      const r = Math.min(3.6, Math.max(1.1, 1.1 + bandwidth / 260));
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fillStyle = FORMANT_COLOR;
      ctx.globalAlpha = 0.72;
      ctx.fill();
      ctx.globalAlpha = 1;
      ctx.stroke();
    }
  }

  /** One analysis frame's formant candidates, ascending by frequency. */
  interface FormantFrameCandidates {
    time: number;
    freqs: Float64Array;
  }

  /**
   * Regroups the flat `[time, freq, bandwidth]` stream back into per-frame
   * candidate lists. Points from the same analysis frame share the same
   * `time` and arrive contiguously, so a run of equal `time` values is one
   * frame; candidates within it are already ascending by frequency (the
   * engine's own ordering), which is what a rank index below tracks.
   */
  function groupFormantFrames(points: Float64Array): FormantFrameCandidates[] {
    const frames: { time: number; freqs: number[] }[] = [];
    let current: { time: number; freqs: number[] } | null = null;
    for (let i = 0; i < points.length; i += 3) {
      const time = points[i];
      if (!current || current.time !== time) {
        current = { time, freqs: [] };
        frames.push(current);
      }
      current.freqs.push(points[i + 1]);
    }
    return frames.map((f) => ({ time: f.time, freqs: Float64Array.from(f.freqs) }));
  }

  /**
   * Connected per-formant tracks: one polyline per rank (lowest candidate in
   * a frame is rank 0, the next is rank 1, and so on). A frame that has no
   * candidate at a rank — an unvoiced stretch, or a higher formant the LPC
   * gate dropped that frame — breaks the path there instead of joining
   * across it, so the line never implies a measurement the analysis did not
   * produce.
   */
  function drawFormantTracks(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    frames: FormantFrameCandidates[]
  ) {
    let maxRank = 0;
    for (const frame of frames) maxRank = Math.max(maxRank, frame.freqs.length);

    const strokeRank = (rank: number, color: string, lineWidth: number) => {
      ctx.strokeStyle = color;
      ctx.lineWidth = lineWidth;
      ctx.lineJoin = 'round';
      ctx.lineCap = 'round';
      let drawing = false;
      ctx.beginPath();
      for (const frame of frames) {
        const freq = frame.freqs[rank];
        const inView =
          freq !== undefined &&
          frame.time >= viewport.t0 &&
          frame.time <= viewport.t1 &&
          freq >= viewport.f0 &&
          freq <= viewport.f1;
        if (!inView) {
          drawing = false;
          continue;
        }
        const x = timeToX(frame.time, width);
        const y = freqToY(freq, height);
        if (!drawing) {
          ctx.moveTo(x, y);
          drawing = true;
        } else {
          ctx.lineTo(x, y);
        }
      }
      ctx.stroke();
    };

    for (let rank = 0; rank < maxRank; rank += 1) {
      strokeRank(rank, HALO, 3.4);
      strokeRank(rank, FORMANT_COLOR, 1.5);
    }
  }

  function drawPitch(ctx: CanvasRenderingContext2D, width: number, height: number) {
    const times = pitch!.times;
    const f0 = pitch!.f0;
    const ceiling = Math.max(1, params.pitch.ceilingHz);
    const yFor = (hz: number) => height * (1 - hz / ceiling);

    const stroke = (color: string, lineWidth: number) => {
      ctx.strokeStyle = color;
      ctx.lineWidth = lineWidth;
      ctx.lineJoin = 'round';
      ctx.lineCap = 'round';
      let drawing = false;
      ctx.beginPath();
      for (let i = 0; i < times.length; i += 1) {
        const hz = f0[i];
        const time = times[i];
        if (!Number.isFinite(hz) || time < viewport.t0 || time > viewport.t1) {
          drawing = false;
          continue;
        }
        const x = timeToX(time, width);
        const y = yFor(hz);
        if (!drawing) {
          ctx.moveTo(x, y);
          drawing = true;
        } else {
          ctx.lineTo(x, y);
        }
      }
      ctx.stroke();
    };

    stroke(HALO, 5.5);
    stroke(PITCH_COLOR, 2.6);
  }

  // Frequency-ruler corner reserves the top band for its units chip; the pitch
  // ceiling label steps below it so the two right-edge scales never stack.
  const CORNER_CHIP_PX = 24;

  function drawPitchAxis(ctx: CanvasRenderingContext2D, width: number, height: number) {
    const ceiling = Math.max(1, params.pitch.ceilingHz);
    const ticks = [0, ceiling / 2, ceiling];
    ctx.font = '11px ui-sans-serif, system-ui, sans-serif';
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';
    // The unit is stated once by the frequency ruler's corner chip; the pitch
    // scale shows numbers only, so its ceiling label no longer collides with a
    // second "Hz" marker.
    for (const hz of ticks) {
      const y = Math.min(height - 7, Math.max(CORNER_CHIP_PX, height * (1 - hz / ceiling)));
      const label = `${Math.round(hz)}`;
      ctx.lineWidth = 3;
      ctx.strokeStyle = HALO;
      ctx.strokeText(label, width - 4, y);
      ctx.fillStyle = PITCH_COLOR;
      ctx.fillText(label, width - 4, y);
    }
  }

  function drawIntensity(ctx: CanvasRenderingContext2D, width: number, height: number) {
    const times = intensity!.times;
    const db = intensity!.db;
    let min = Infinity;
    let max = -Infinity;
    for (let i = 0; i < db.length; i += 1) {
      if (!Number.isFinite(db[i])) continue;
      if (db[i] < min) min = db[i];
      if (db[i] > max) max = db[i];
    }
    if (!Number.isFinite(min) || max - min < 1e-6) return;
    // Keep the contour inside a lower band so it does not fight the pitch line.
    const top = height * 0.12;
    const bottom = height * 0.94;
    const yFor = (value: number) => bottom - ((value - min) / (max - min)) * (bottom - top);

    const stroke = (color: string, lineWidth: number) => {
      ctx.strokeStyle = color;
      ctx.lineWidth = lineWidth;
      ctx.lineJoin = 'round';
      ctx.lineCap = 'round';
      let drawing = false;
      ctx.beginPath();
      for (let i = 0; i < times.length; i += 1) {
        const time = times[i];
        if (time < viewport.t0 || time > viewport.t1 || !Number.isFinite(db[i])) {
          drawing = false;
          continue;
        }
        const x = timeToX(time, width);
        const y = yFor(db[i]);
        if (!drawing) {
          ctx.moveTo(x, y);
          drawing = true;
        } else {
          ctx.lineTo(x, y);
        }
      }
      ctx.stroke();
    };

    stroke(HALO, 3.2);
    stroke(INTENSITY_COLOR, 1.4);
  }
</script>

<canvas
  bind:this={canvas}
  class="overlay"
  data-testid="track-overlay"
  data-overlay-token={renderToken}
  data-pitch-data-token={pitchDataToken}
  data-pitch-max={pitchMaxHz.toFixed(1)}
  data-formant-max={(formant?.maxHz ?? 0).toFixed(1)}
  aria-hidden="true"
></canvas>

<style>
  .overlay {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
  }
</style>
