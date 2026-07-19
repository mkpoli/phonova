<script lang="ts">
  import LandingPage from '$lib/landing/LandingPage.svelte';

  const url = 'https://phonia.app/landing';
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
      "url": "https://phonia.app/landing",
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
