#!/usr/bin/env bun
// Assembles the bundled sample corpus the web app offers from the Home screen.
//
// The web project container references audio by path rather than embedding it,
// so the sample ships as its source files plus a manifest; the app fetches them
// and runs the same import path a folder drop uses. Running this script copies
// the fixture recordings into the web app's static tree and writes the manifest
// deterministically (fixed order, stable JSON), so the output is reproducible.
//
//   bun run tests/fixtures/scripts/build_sample_project.ts

import { copyFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../../..');
const audioDir = join(root, 'tests/fixtures/audio');
const textgridDir = join(root, 'tests/fixtures/textgrids');
const outDir = join(root, 'apps/web/static/sample');

// Two CMU ARCTIC sentences with their word tiers, and one perturbed synthetic
// vowel for the voice report. Each TextGrid takes the matching WAV's stem so
// stem attachment fires when the app imports the batch.
const entries: Array<{ from: string; name: string; mime: string }> = [
  { from: join(audioDir, 'arctic_bdl_a0001.wav'), name: 'arctic_bdl_a0001.wav', mime: 'audio/wav' },
  { from: join(audioDir, 'arctic_slt_a0001.wav'), name: 'arctic_slt_a0001.wav', mime: 'audio/wav' },
  { from: join(audioDir, 'synth_vowel_perturbed.wav'), name: 'synth_vowel_perturbed.wav', mime: 'audio/wav' },
  { from: join(textgridDir, 'arctic_bdl_a0001_long_utf8.TextGrid'), name: 'arctic_bdl_a0001.TextGrid', mime: 'text/plain' },
  { from: join(textgridDir, 'arctic_slt_a0001_short_utf8.TextGrid'), name: 'arctic_slt_a0001.TextGrid', mime: 'text/plain' }
];

mkdirSync(outDir, { recursive: true });

for (const entry of entries) {
  copyFileSync(entry.from, join(outDir, entry.name));
}

const manifest = {
  name: 'Sample: ARCTIC sentences',
  files: entries.map((entry) => ({ path: entry.name, name: entry.name, mime: entry.mime }))
};

writeFileSync(join(outDir, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');

console.log(`Wrote ${entries.length} files and manifest.json to ${outDir}`);
