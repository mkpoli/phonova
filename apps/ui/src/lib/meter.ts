/** Shared conversions for the level meters (the microphone recording strip and
 *  the editor's playback meter), so both read the same decibel scale. */

/** Converts a linear amplitude in [0, 1] to dBFS, floored at -60 dB. */
export function amplitudeToDb(value: number): number {
  if (value <= 0) return -60;
  return Math.max(-60, 20 * Math.log10(value));
}

/** Maps a linear amplitude in [0, 1] onto a meter fill fraction in [0, 1] over
 *  the top 60 dB, so quiet speech still moves the bar. */
export function amplitudeToMeterFill(value: number): number {
  return Math.max(0, Math.min(1, (amplitudeToDb(value) + 60) / 60));
}
