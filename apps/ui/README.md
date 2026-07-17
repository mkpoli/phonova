# @phonia/ui

The shared Svelte package owns editor layout, viewport state, canvas rendering, cache keys, and the WebGL2 to Canvas2D fallback. `apps/web` supplies a `CoreClientLike` transport and playback clock.

Waveform and spectrogram caches are keyed by audio id, tile coordinates, viewport pixel size, theme, and analysis parameters. The renderer records the last 24 draw durations. A pane switches to Canvas2D when the rolling average exceeds 32 ms and shows a non-blocking notice.
