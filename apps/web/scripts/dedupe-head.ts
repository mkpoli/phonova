// app.html carries the site-wide default <title>/<meta>/<link>/JSON-LD
// block as plain static text, outside %sveltekit.head% — every prerendered
// route's output starts with those defaults baked in. A route that declares
// its own <svelte:head> (see routes/landing/+page.svelte) gets its tags
// appended after that static block, not substituted for it, so the built
// HTML ends up with two <title> elements, two canonical links, and so on.
//
// This pass runs after `vite build` and, for every prerendered *.html file,
// keeps only the last occurrence of each singleton head tag (title, meta
// description, canonical link, each distinct og:*/twitter:* property, and
// the JSON-LD block) — the later one is always the route's own override,
// since %sveltekit.head% sits after the static defaults in app.html. Pages
// with no route-level override are untouched: there is nothing to drop.

import { readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const BUILD_DIR = join(import.meta.dir, '..', 'build');

const TAG_RE = /<title>[\s\S]*?<\/title>|<meta\b[^>]*>|<link\b[^>]*>|<script\b[^>]*>[\s\S]*?<\/script>/gi;

function attr(tag: string, name: string): string | null {
  const m = tag.match(new RegExp(`\\b${name}\\s*=\\s*"([^"]*)"`, 'i'));
  return m ? m[1] : null;
}

function tagName(tag: string): string {
  return (tag.match(/^<(\w+)/)?.[1] ?? '').toLowerCase();
}

/** Returns a dedup key for tags that must appear at most once per page, or
 * null for tags that are always kept (icons, manifest, charset, viewport,
 * theme-color — the last two carry a `media` attribute that already makes
 * the light/dark pair distinct). */
function dedupeKey(tag: string): string | null {
  const name = tagName(tag);
  if (name === 'title') return 'title';
  if (name === 'meta') {
    const property = attr(tag, 'property');
    if (property) return `meta:property:${property}`;
    const metaName = attr(tag, 'name');
    if (metaName === 'description') return 'meta:name:description';
    if (metaName?.startsWith('twitter:')) return `meta:name:${metaName}`;
    return null;
  }
  if (name === 'link') {
    return attr(tag, 'rel') === 'canonical' ? 'link:canonical' : null;
  }
  if (name === 'script') {
    return attr(tag, 'type') === 'application/ld+json' ? 'ldjson' : null;
  }
  return null;
}

function dedupeHead(html: string): string {
  const headMatch = html.match(/<head>([\s\S]*?)<\/head>/i);
  if (!headMatch) return html;
  const head = headMatch[1];

  const tags = [...head.matchAll(TAG_RE)].map((m) => ({ full: m[0], key: dedupeKey(m[0]) }));
  const lastIndex = new Map<string, number>();
  tags.forEach((t, i) => {
    if (t.key) lastIndex.set(t.key, i);
  });
  const kept = tags.filter((t, i) => !t.key || lastIndex.get(t.key) === i);
  if (kept.length === tags.length) return html; // nothing to drop

  // Reconstruct the head by walking matches again, keeping surrounding
  // whitespace and only dropping the source span of tags not in `kept`.
  const matches = [...head.matchAll(TAG_RE)];
  let newHead = '';
  let pos = 0;
  matches.forEach((m, i) => {
    const start = m.index ?? 0;
    newHead += head.slice(pos, start);
    if (kept.includes(tags[i])) newHead += m[0];
    pos = start + m[0].length;
  });
  newHead += head.slice(pos);

  return html.slice(0, headMatch.index!) + `<head>${newHead}</head>` + html.slice(headMatch.index! + headMatch[0].length);
}

async function main() {
  let entries: string[];
  try {
    entries = (await readdir(BUILD_DIR)).filter((f) => f.endsWith('.html'));
  } catch {
    console.log('dedupe-head: no build/ directory, skipping');
    return;
  }

  for (const entry of entries) {
    const path = join(BUILD_DIR, entry);
    const html = await readFile(path, 'utf8');
    const deduped = dedupeHead(html);
    if (deduped !== html) {
      await writeFile(path, deduped);
      console.log(`dedupe-head: cleaned duplicate head tags in ${entry}`);
    }
  }
}

await main();
