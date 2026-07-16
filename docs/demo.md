# Phonix v0.1 demo walkthrough

A click-by-click run of the five demo moments against a local build. Each moment
starts from the Home screen and takes under a minute. The steps use the bundled
sample project, so no external files are needed except where a moment imports one
from the repository's fixture folder.

## Build and open

From the repository root:

```
cd apps/web
bun run build
bun run preview
```

The preview serves the production build at <http://127.0.0.1:4173>. Open that URL
in Chromium or another current browser. The Home screen shows the empty state
with an **Open sample project** button and a first-run hint for the command
palette (`Ctrl-K`, `⌘K` on macOS).

The command palette is the fastest route to every action. Press `Ctrl-K` at any
point, type a few letters of an action's name, and press Enter. Each moment below
gives the palette route alongside the direct one.

## Moment 1 — a dropped folder becomes a browsable corpus

1. On the Home screen, click **Open sample project**. (To use your own material
   instead, drop a folder of WAV files onto the window; a TextGrid beside a WAV
   of the same name attaches as its annotation.)
2. The corpus view opens with one row per recording: two CMU ARCTIC sentences and
   a synthesized vowel. Each row draws a waveform thumbnail and reads out
   duration, sample rate, and channel count. The two ARCTIC rows show a **tiers**
   badge from their attached word annotation.
3. Click the `arctic_bdl_a0001` row to open it in the editor.
4. In the annotation toolbar, type a word from the sentence (for example `the`)
   into the search field. The counter shows how many labels match, and the `‹`
   and `›` buttons step through them, seeking the timeline to each hit. Clearing
   the field returns the full view.

## Moment 2 — measure a time–frequency box

1. With a recording open, click-drag a box on the spectrogram pane: press inside
   the pane, drag across a vowel and up through its formants, and release.
2. The readout bar above the timeline fills in for the box: duration, mean F0,
   and band energy. The bar's **Play**, **Zoom** (`F`), and **Voice report**
   buttons all act on the box.
3. Click **Play** to hear the boxed region. Click **Zoom** (or press `F`) to
   frame the box; press `0` to fit the whole file again.

## Moment 3 — one undo stack across different edits

1. Open `arctic_bdl_a0001`. Click the `words` tier chip to make it active.
2. Move a boundary: press `Tab` to step to an interval, then `→` to nudge its
   active boundary one pixel (hold `Alt` to step one sample frame).
3. Edit a label: press `Enter`, type a new label, and press `Enter` again.
4. Split an interval: seek the cursor into an interval and press `S`.
5. Press `Ctrl-Z` three times. The split, the label, and the boundary each
   reverse in turn from one stack, in order. `Ctrl-Shift-Z` replays them.

Importing a recording or a TextGrid records on the same journal, so undo reaches
back through an import as well.

## Moment 4 — the palette measurement equals the scripted one

1. Draw a selection box over a voiced stretch, as in moment 2.
2. Press `Ctrl-K`, type `voice`, and run **Voice report over selection**. The row
   shows the engine method it calls, `voiceReport`. The report card reads out F0,
   jitter, shimmer, and HNR, with the pitch floor and ceiling it used (75 Hz and
   600 Hz by default).
3. Open the browser's developer console and call the same method against the
   engine directly, at the selection's own coordinates:

   ```js
   const { client, audioId } = window.__phonix;
   const bar = document.querySelector('[data-testid="readout-bar"]');
   const t0 = Number(bar.getAttribute('data-t0'));
   const t1 = Number(bar.getAttribute('data-t1'));
   const report = await client.voiceReport(audioId, t0, t1, 75, 600);
   console.log(report);
   ```

4. The scripted jitter, shimmer, and HNR match the card digit for digit. The
   palette action and the console call reach the one engine; the palette names
   the method so a script can find it.

## Moment 5 — a grayscale figure for print

1. Press `Ctrl-K`, type `gray`, and run **Spectrogram palette: Grayscale**. The
   spectrogram redraws in grayscale, tuned for the current theme rather than
   inverted from the color map.
2. Press `E` (or click **Export figure** in the status bar) to open the figure
   dialog. The preview renders from the current view through the same SVG backend
   the export writes, so the preview matches what the export saves.
3. Set **Palette** to *Grayscale (print)*. Toggle **Preview theme** between Light
   and Dark to confirm the figure reads against a white page as well as a dark
   one.
4. Choose a **Format** and click **Download**. SVG and PNG save from the browser;
   the preview and the saved SVG are byte-for-byte identical. PDF export runs in
   the desktop build (the dialog notes this on the web).

## Notes

- Nothing here is destructive. Imports never change the source WAV, edits become
  journal entries, and the project autosaves in the background with crash
  recovery from the Home screen.
- Every action shown has a palette entry under the same name, so the palette
  doubles as a searchable index of what the build can do.
