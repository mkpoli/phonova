// Renders static SEO/install assets (OG image, app icons) from the Phonia
// mark using headless Chromium. Run with `bun run scripts/generate-assets.ts`
// from apps/web so `@playwright/test` resolves from the workspace install.
import { chromium } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const staticDir = join(here, '..', 'static');

// Mark geometry shared by every render — matches static/favicon.svg.
const MARK_STROKES = `
  <path d="M46.5 12.9 A 22 22 0 1 0 52.2 20.4" />
  <path d="M14 36 C20 24 25 24 31 32 C37 40 41 40 50 24" />
`;

// Standalone-rendering pins from docs/DESIGN.md: teal-600/amber-600 for the
// light scheme, teal-300/amber-300 for the dark scheme (same values as
// static/favicon.svg's two @media branches).
const LIGHT_PIN = { stroke: '#0d9488', dot: '#d97706' };
const DARK_PIN = { stroke: '#5eead4', dot: '#f5b04c' };

function markSvg(size: number, pin: { stroke: string; dot: string }): string {
  return `<svg width="${size}" height="${size}" viewBox="0 0 64 64" fill="none" stroke-linecap="round" stroke-width="7" xmlns="http://www.w3.org/2000/svg"><g stroke="${pin.stroke}">${MARK_STROKES}</g><circle cx="52" cy="20" r="5.5" fill="${pin.dot}" /></svg>`;
}

async function main() {
  await mkdir(staticDir, { recursive: true });
  const browser = await chromium.launch();

  try {
    // 1. Open Graph image: mark + wordmark + capability words on the dark
    //    warm-charcoal ground, from scripts/og-image.html.
    {
      const page = await browser.newPage({ viewport: { width: 1200, height: 630 } });
      await page.goto(`file://${join(here, 'og-image.html')}`);
      await page.waitForTimeout(50);
      await page.screenshot({ path: join(staticDir, 'og.png') });
      await page.close();
      console.log('wrote static/og.png');
    }

    // 2. App icons: mark alone, transparent background, light-scheme pin
    //    (matches the favicon's default, no-media-query rendering).
    for (const size of [192, 512]) {
      const page = await browser.newPage({ viewport: { width: size, height: size } });
      await page.setContent(
        `<!doctype html><html><head><style>html,body{margin:0;padding:0}</style></head><body>${markSvg(size, LIGHT_PIN)}</body></html>`
      );
      await page.screenshot({ path: join(staticDir, `icon-${size}.png`), omitBackground: true });
      await page.close();
      console.log(`wrote static/icon-${size}.png`);
    }

    // 3. Maskable 512 icon: solid warm-charcoal ground (matches manifest
    //    background_color), dark-scheme pin, mark held inside the maskable
    //    safe area (an 80%-diameter centered circle per the W3C spec — the
    //    mark here spans 60% of the canvas, well inside it).
    {
      const size = 512;
      const markSize = Math.round(size * 0.6);
      const page = await browser.newPage({ viewport: { width: size, height: size } });
      await page.setContent(`<!doctype html><html><head><style>
        html,body{margin:0;padding:0;width:${size}px;height:${size}px;background:#1e1d1a;
          display:flex;align-items:center;justify-content:center}
      </style></head><body>${markSvg(markSize, DARK_PIN)}</body></html>`);
      await page.screenshot({ path: join(staticDir, 'icon-512-maskable.png') });
      await page.close();
      console.log('wrote static/icon-512-maskable.png');
    }
  } finally {
    await browser.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
