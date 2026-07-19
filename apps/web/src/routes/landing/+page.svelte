<script lang="ts">
  import LandingPage from '$lib/landing/LandingPage.svelte';

  // Served at two addresses (about.phonia.app/ via the worker rewrite in
  // src/worker.ts, and phonia.app/landing directly) with byte-identical
  // markup. about.phonia.app is the dedicated marketing domain, so it is
  // canonical; phonia.app/landing's own tags point there too, rather than
  // at themselves, so the two don't read as duplicate content.
  const url = 'https://about.phonia.app/';
  const title = 'Phonia — phonetics software for the browser and desktop';
  const description =
    "Phonia records, annotates, and measures speech, in the browser or as a desktop app. Pitch, formant, intensity, and harmonicity analysis are checked against Praat under matched settings. Free and open source.";
</script>

<svelte:head>
  <title>{title}</title>
  <meta name="description" content={description} />
  <link rel="canonical" href={url} />

  <meta property="og:url" content={url} />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={description} />

  <meta name="twitter:title" content={title} />
  <meta name="twitter:description" content={description} />

  <script type="application/ld+json">
    {
      "@context": "https://schema.org",
      "@type": "AboutPage",
      "name": "Phonia — phonetics software for the browser and desktop",
      "description": "Phonia records, annotates, and measures speech, in the browser or as a desktop app. Pitch, formant, intensity, and harmonicity analysis are checked against Praat under matched settings. Free and open source.",
      "url": "https://about.phonia.app/",
      "mainEntity": {
        "@type": "SoftwareApplication",
        "name": "Phonia",
        "applicationCategory": "MultimediaApplication",
        "operatingSystem": "Web, Windows, macOS, Linux",
        "offers": {
          "@type": "Offer",
          "price": "0",
          "priceCurrency": "USD"
        },
        "url": "https://phonia.app/"
      }
    }
  </script>
</svelte:head>

<LandingPage />
