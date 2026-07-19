#!/usr/bin/env bun
// Assembles the bundled sample corpus the web app offers from the Home screen.
//
// The web project container references audio by path rather than embedding it,
// so the sample ships as its source files plus a manifest; the app fetches them
// and runs the same import path a folder drop uses. Running this script copies
// the fixture recordings into the web app's static tree, converts each
// recording's genuine CMU ARCTIC forced-alignment label file into a TextGrid
// via `phx-textgrid`'s own writer (`cargo run -p phx-textgrid --example
// lab_to_textgrid`), and writes the manifest deterministically (fixed order,
// stable JSON), so the output is reproducible.
//
//   bun run tests/fixtures/scripts/build_sample_project.ts

import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../../..');
const audioDir = join(root, 'tests/fixtures/audio');
const alignmentsDir = join(root, 'tests/fixtures/alignments');
const outDir = join(root, 'apps/web/static/sample');

/** Reads a 16-bit PCM WAV's duration in seconds from its RIFF header. */
function wavDurationSeconds(path: string): number {
  const buf = readFileSync(path);
  if (buf.toString('ascii', 0, 4) !== 'RIFF' || buf.toString('ascii', 8, 12) !== 'WAVE') {
    throw new Error(`${path} is not a RIFF/WAVE file`);
  }
  let offset = 12;
  let sampleRate = 0;
  let blockAlign = 0;
  let dataSize = 0;
  while (offset + 8 <= buf.length) {
    const id = buf.toString('ascii', offset, offset + 4);
    const size = buf.readUInt32LE(offset + 4);
    const body = offset + 8;
    if (id === 'fmt ') {
      sampleRate = buf.readUInt32LE(body + 4);
      blockAlign = buf.readUInt16LE(body + 12);
    } else if (id === 'data') {
      dataSize = size;
    }
    offset = body + size + (size % 2);
  }
  if (sampleRate === 0 || blockAlign === 0 || dataSize === 0) {
    throw new Error(`${path}: could not find fmt/data chunks`);
  }
  return dataSize / blockAlign / sampleRate;
}

/**
 * Converts a CMU ARCTIC `.lab` forced-alignment file into a TextGrid with a
 * single `phones` interval tier, boundaries taken verbatim from the
 * alignment. See `tests/fixtures/alignments/` in `MANIFEST.md` for
 * provenance and license.
 */
function labToTextGrid(labPath: string, wavPath: string, outPath: string): void {
  const duration = wavDurationSeconds(wavPath);
  const result = spawnSync(
    'cargo',
    ['run', '-p', 'phx-textgrid', '--example', 'lab_to_textgrid', '--', labPath, String(duration), 'phones', outPath],
    { cwd: root, stdio: 'inherit' }
  );
  if (result.status !== 0) {
    throw new Error(`lab_to_textgrid failed for ${labPath}`);
  }
}

mkdirSync(outDir, { recursive: true });

const audioEntries: Array<{ from: string; name: string; mime: string }> = [
  { from: join(audioDir, 'arctic_bdl_a0001.wav'), name: 'arctic_bdl_a0001.wav', mime: 'audio/wav' },
  { from: join(audioDir, 'arctic_slt_a0001.wav'), name: 'arctic_slt_a0001.wav', mime: 'audio/wav' },
  { from: join(audioDir, 'synth_vowel_perturbed.wav'), name: 'synth_vowel_perturbed.wav', mime: 'audio/wav' }
];
for (const entry of audioEntries) {
  copyFileSync(entry.from, join(outDir, entry.name));
}

// synth_vowel_perturbed is a synthesized sustained vowel with no forced
// alignment, so it ships with no TextGrid.
const textgridEntries: Array<{ lab: string; wav: string; name: string; mime: string }> = [
  { lab: join(alignmentsDir, 'arctic_bdl_a0001.lab'), wav: join(audioDir, 'arctic_bdl_a0001.wav'), name: 'arctic_bdl_a0001.TextGrid', mime: 'text/plain' },
  { lab: join(alignmentsDir, 'arctic_slt_a0001.lab'), wav: join(audioDir, 'arctic_slt_a0001.wav'), name: 'arctic_slt_a0001.TextGrid', mime: 'text/plain' }
];
for (const entry of textgridEntries) {
  labToTextGrid(entry.lab, entry.wav, join(outDir, entry.name));
}

const manifest = {
  name: 'Sample: ARCTIC sentences',
  files: [
    ...audioEntries.map((entry) => ({ path: entry.name, name: entry.name, mime: entry.mime })),
    ...textgridEntries.map((entry) => ({ path: entry.name, name: entry.name, mime: entry.mime }))
  ]
};

writeFileSync(join(outDir, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');

console.log(`Wrote ${audioEntries.length + textgridEntries.length} files and manifest.json to ${outDir}`);
