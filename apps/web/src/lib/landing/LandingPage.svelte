<script lang="ts">
  import { onMount } from 'svelte';
  import RollingWord from './RollingWord.svelte';
  import {
    HERO_KEYS,
    MINI_KEYS,
    WAV_KEYS,
    buildSpec,
    specToCanvas,
    strokeTrack,
    sampleKeys,
    type PlotBox
  } from './spectrogram';

  const PHRASES = [
    'in your browser',
    "in your phone's browser",
    'offline, as a desktop app',
    'without an account'
  ];

  interface Props {
    /** Called instead of navigating when this page is shown inline at the
     * app's own root (a first-time visit) — swaps straight to the app
     * without a reload. Omitted when this page is reached directly (the
     * `/landing` route, or the marketing subdomain), where a normal link
     * navigation is correct. */
    onEnterApp?: () => void;
  }

  let { onEnterApp }: Props = $props();

  // Marks the visit regardless of how "Open Phonia" is reached, so a visitor
  // who lands here directly (not through the app's own first-visit gate)
  // still skips the landing page next time.
  function handleOpenPhonia(event: MouseEvent) {
    try {
      localStorage.setItem('phonia:visited', '1');
    } catch {
      // Storage unavailable: the landing page simply shows again next visit.
    }
    const onAboutSubdomain =
      typeof window !== 'undefined' && window.location.hostname === 'about.phonia.app';
    if (onEnterApp && !onAboutSubdomain) {
      event.preventDefault();
      onEnterApp();
    }
  }

  let root = $state<HTMLElement | null>(null);
  let heroCanvas = $state<HTMLCanvasElement | null>(null);
  let miniCanvas = $state<HTMLCanvasElement | null>(null);
  let wavCanvas = $state<HTMLCanvasElement | null>(null);

  let showFormants = $state(true);
  let showPitch = $state(false);

  let frameReady = $state(false);
  let frameFailed = $state(false);
  let appFrame = $state<HTMLIFrameElement | null>(null);
  let frameSection = $state<HTMLElement | null>(null);

  let redrawHero: (() => void) | null = null;

  const rm =
    typeof window !== 'undefined' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // On the marketing subdomain, "/" is this same landing page, not the app —
  // send the CTA cross-origin to the app itself. `app=1` tells the app root
  // to skip its own first-visit landing check so the handoff is one click.
  // On the app's own domain (served at "/landing" or as a first-visit
  // redirect from "/"), a relative link is enough.
  const appHref =
    typeof window !== 'undefined' && window.location.hostname === 'about.phonia.app'
      ? 'https://phonia.app/?app=1'
      : '/';

  function handleFrameLoad() {
    // The embedded app is a client-rendered SPA: `load` fires once the
    // document and its script bundle have arrived, before Svelte has
    // mounted anything into the page, so the frame's body is briefly empty
    // on every successful load. That rules out an emptiness check here —
    // the 6 s timer in onMount is the sole failure signal (no `load` at
    // all within the window means the frame is genuinely stuck, blocked,
    // or offline).
    frameReady = true;
  }

  onMount(() => {
    // The iframe is `loading="lazy"`, so the browser only starts fetching it
    // once the frame nears the viewport — on a normal scroll pace that can
    // be well past 6 seconds after mount. Starting the failure timer at
    // mount would flag a slow scroller's frame as failed before it ever
    // began loading, so the timer starts only once the frame's section is
    // close enough to the viewport that the browser's own lazy-load has
    // plausibly kicked in.
    let timer: ReturnType<typeof setTimeout> | null = null;
    let frameObserver: IntersectionObserver | null = null;
    if (frameSection) {
      frameObserver = new IntersectionObserver(
        (entries) => {
          if (entries.some((e) => e.isIntersecting)) {
            timer = setTimeout(() => {
              if (!frameReady) frameFailed = true;
            }, 6000);
            frameObserver?.disconnect();
          }
        },
        { rootMargin: '800px 0px' }
      );
      frameObserver.observe(frameSection);
    }

    let raf = 0;

    const hero = heroCanvas;
    if (hero) {
      const spec = buildSpec({ cols: 560, rows: 230, maxF: 5500, dur: 4.2, keys: HERO_KEYS });
      const off = specToCanvas(spec);
      const ctx = hero.getContext('2d')!;
      const pad = { l: 48, r: 14, t: 12, b: 30 };
      let W = 0;
      let H = 0;
      const SWEEP = 6800;
      const HOLD = 1500;
      const FADE = 550;
      const CYCLE = SWEEP + HOLD + FADE;
      const start = performance.now();

      const plotBox = (): PlotBox => ({ l: pad.l, t: pad.t, w: W - pad.l - pad.r, h: H - pad.t - pad.b });

      const draw = (prog: number) => {
        ctx.clearRect(0, 0, W, H);
        const p = plotBox();
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.drawImage(off, p.l, p.t, p.w, p.h);
        ctx.strokeStyle = 'rgba(236,231,219,0.055)';
        ctx.lineWidth = 1;
        ctx.fillStyle = 'rgba(168,162,148,0.85)';
        ctx.font = '10px ui-monospace,Menlo,Consolas,monospace';
        ctx.textAlign = 'right';
        ctx.textBaseline = 'middle';
        for (let f = 1000; f < spec.maxF; f += 1000) {
          const y = p.t + (1 - f / spec.maxF) * p.h;
          ctx.beginPath();
          ctx.moveTo(p.l, y);
          ctx.lineTo(p.l + p.w, y);
          ctx.stroke();
          ctx.fillText(f / 1000 + 'k', p.l - 7, y);
        }
        ctx.fillText('0', p.l - 7, p.t + p.h);
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';
        for (let t = 0.5; t < spec.dur; t += 0.5) {
          const x = p.l + (t / spec.dur) * p.w;
          ctx.fillText(t.toFixed(1), x, p.t + p.h + 8);
        }
        ctx.textAlign = 'right';
        ctx.fillText('s', p.l + p.w, p.t + p.h + 20);
        const px = p.l + prog * p.w;
        if (prog < 1) {
          ctx.fillStyle = 'rgba(19,17,14,0.62)';
          ctx.fillRect(px, p.t, p.w - prog * p.w, p.h);
        }
        ctx.save();
        ctx.beginPath();
        ctx.rect(p.l, p.t, prog * p.w, p.h);
        ctx.clip();
        if (showPitch) strokeTrack(ctx, spec, p, spec.f0Arr, '#f5b04c', 2.2);
        if (showFormants) {
          for (let k = 0; k < 3; k++) {
            strokeTrack(ctx, spec, p, spec.form[k], '#5eead4', 2.2, (c) => spec.ampArr[c] > 0.05);
          }
        }
        ctx.restore();
        if (prog > 0 && prog < 1) {
          ctx.save();
          ctx.strokeStyle = 'rgba(245,176,76,0.95)';
          ctx.lineWidth = 2;
          ctx.shadowColor = 'rgba(245,176,76,0.8)';
          ctx.shadowBlur = 10;
          ctx.beginPath();
          ctx.moveTo(px, p.t);
          ctx.lineTo(px, p.t + p.h);
          ctx.stroke();
          ctx.restore();
        }
      };

      const progNow = () => {
        const t = (performance.now() - start) % CYCLE;
        return t < SWEEP ? t / SWEEP : 1;
      };
      const fadeNow = () => {
        const t = (performance.now() - start) % CYCLE;
        return t < SWEEP + HOLD ? 0 : (t - SWEEP - HOLD) / FADE;
      };
      const frame = () => {
        draw(progNow());
        const f = fadeNow();
        if (f > 0) {
          ctx.fillStyle = 'rgba(30,29,26,' + (f * 0.9).toFixed(3) + ')';
          ctx.fillRect(0, 0, W, H);
        }
        raf = requestAnimationFrame(frame);
      };
      const resize = () => {
        const r = hero.getBoundingClientRect();
        const dpr = Math.min(2, window.devicePixelRatio || 1);
        W = r.width;
        H = r.height;
        hero.width = Math.round(W * dpr);
        hero.height = Math.round(H * dpr);
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        draw(rm ? 1 : progNow());
      };
      const ro = new ResizeObserver(resize);
      ro.observe(hero);
      resize();
      redrawHero = () => draw(rm ? 1 : progNow());
      if (!rm) raf = requestAnimationFrame(frame);
    }

    const mini = miniCanvas;
    if (mini) {
      const spec = buildSpec({ cols: 300, rows: 130, maxF: 5000, dur: 2.1, keys: MINI_KEYS });
      const off = specToCanvas(spec);
      const ctx = mini.getContext('2d')!;
      const render = () => {
        const r = mini.getBoundingClientRect();
        const dpr = Math.min(2, window.devicePixelRatio || 1);
        const W = r.width;
        const H = r.height;
        mini.width = Math.round(W * dpr);
        mini.height = Math.round(H * dpr);
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        const p = { l: 8, t: 8, w: W - 16, h: H - 16 };
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.clearRect(0, 0, W, H);
        ctx.drawImage(off, p.l, p.t, p.w, p.h);
        for (let k = 0; k < 3; k++) {
          strokeTrack(ctx, spec, p, spec.form[k], '#5eead4', 1.6, (c) => spec.ampArr[c] > 0.06);
        }
      };
      new ResizeObserver(render).observe(mini);
      render();
    }

    const wav = wavCanvas;
    if (wav) {
      const sr = 6000;
      const dur = 1.3;
      const N = Math.floor(sr * dur);
      const data = new Float32Array(N);
      const f0Arr = new Float32Array(N);
      const B = [90, 110, 150, 210];
      let phase = 0;
      for (let i = 0; i < N; i++) {
        const t = i / sr;
        const p = sampleKeys(WAV_KEYS, t);
        const f0 = p.f0;
        f0Arr[i] = f0;
        let s = 0;
        if (f0 > 0 && p.amp > 0.003) {
          phase += (2 * Math.PI * f0) / sr;
          for (let h = 1; h <= 14; h++) {
            const f = h * f0;
            let w = 0;
            for (let k = 0; k < 4; k++) {
              const sig = B[k] / 2;
              const d = (f - p.F[k]) / sig;
              w += p.g[k] * Math.exp(-0.5 * d * d);
            }
            s += p.amp * Math.pow(h, -0.95) * (0.1 + w) * Math.sin(phase * h + h * 0.7);
          }
        }
        s += (p.ng || 0.002) * (Math.random() * 2 - 1) * 0.15;
        data[i] = s;
      }
      let peak = 1e-9;
      for (let i = 0; i < N; i++) {
        const a = Math.abs(data[i]);
        if (a > peak) peak = a;
      }
      for (let i = 0; i < N; i++) data[i] = (data[i] / peak) * 0.92;
      const ctx = wav.getContext('2d')!;
      const render = () => {
        const r = wav.getBoundingClientRect();
        const dpr = Math.min(2, window.devicePixelRatio || 1);
        const W = r.width;
        const H = r.height;
        wav.width = Math.round(W * dpr);
        wav.height = Math.round(H * dpr);
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, W, H);
        const padL = 10;
        const padR = 10;
        const padT = 14;
        const padB = 14;
        const pw = W - padL - padR;
        const ph = H - padT - padB;
        const mid = padT + ph / 2;
        ctx.fillStyle = 'rgba(217,210,194,0.5)';
        const step = N / pw;
        for (let x = 0; x < pw; x++) {
          const i0 = Math.floor(x * step);
          const i1 = Math.min(N - 1, Math.floor((x + 1) * step));
          let mn = 1;
          let mx = -1;
          for (let i = i0; i <= i1; i++) {
            const v = data[i];
            if (v < mn) mn = v;
            if (v > mx) mx = v;
          }
          const y0 = mid - (mx * ph) / 2;
          const y1 = mid - (mn * ph) / 2;
          ctx.fillRect(padL + x, y0, 1, Math.max(1, y1 - y0));
        }
        ctx.save();
        ctx.strokeStyle = '#f5b04c';
        ctx.lineWidth = 2;
        ctx.lineCap = 'round';
        ctx.lineJoin = 'round';
        ctx.shadowColor = 'rgba(245,176,76,0.7)';
        ctx.shadowBlur = 6;
        ctx.beginPath();
        let pen = false;
        const fMin = 70;
        const fMax = 150;
        for (let x = 0; x < pw; x++) {
          const i = Math.min(N - 1, Math.floor(x * step));
          const f = f0Arr[i];
          if (f <= 0) {
            pen = false;
            continue;
          }
          const y = padT + ph * (1 - (f - fMin) / (fMax - fMin));
          const px = padL + x;
          if (!pen) {
            ctx.moveTo(px, y);
            pen = true;
          } else ctx.lineTo(px, y);
        }
        ctx.stroke();
        ctx.restore();
        ctx.fillStyle = 'rgba(245,176,76,0.55)';
        ctx.font = '9px ui-monospace,Menlo,Consolas,monospace';
        ctx.textAlign = 'right';
        ctx.textBaseline = 'top';
        ctx.fillText('pitch, Hz', W - 10, 8);
      };
      new ResizeObserver(render).observe(wav);
      render();
    }

    let io: IntersectionObserver | null = null;
    if (!rm && root) {
      const els = [...root.querySelectorAll('[data-rv]')];
      els.forEach((el) => el.classList.add('rvl'));
      io = new IntersectionObserver(
        (entries) => {
          entries.forEach((en) => {
            if (en.isIntersecting) {
              en.target.classList.add('in');
              io?.unobserve(en.target);
            }
          });
        },
        { threshold: 0.12 }
      );
      els.forEach((el) => io!.observe(el));
    }

    return () => {
      if (timer) clearTimeout(timer);
      frameObserver?.disconnect();
      cancelAnimationFrame(raf);
      io?.disconnect();
    };
  });

  $effect(() => {
    showFormants;
    showPitch;
    redrawHero?.();
  });
</script>

<div class="landing" bind:this={root}>
  <a class="skip" href="#main">Skip to content</a>

  <header class="site-header">
    <div class="wrap header-in">
      <a class="brand" href="/landing" aria-label="Phonia home">
        <svg width="38" height="38" viewBox="0 0 64 64" fill="none" aria-hidden="true">
          <path
            class="mark-ring"
            pathLength="100"
            d="M46.5 12.9 A 22 22 0 1 0 52.2 20.4"
            stroke="#5eead4"
            stroke-width="7"
            stroke-linecap="round"
          />
          <path
            class="mark-line"
            pathLength="100"
            d="M14 36 C20 24 25 24 31 32 C37 40 41 40 50 24"
            stroke="#5eead4"
            stroke-width="7"
            stroke-linecap="round"
          />
          <circle class="mark-dot" cx="52" cy="20" r="5.5" fill="#f5b04c" stroke="none" />
        </svg>
        <span class="wordmark">Phonia</span>
      </a>
      <nav class="nav" aria-label="Primary">
        <a class="nav-link" href="#capabilities">Capabilities</a>
        <a class="nav-link" href="#validation">Validation</a>
        <a class="nav-link" href="#app">The app</a>
        <a class="btn btn-primary btn-sm" href={appHref} onclick={handleOpenPhonia}>Open Phonia</a>
      </nav>
    </div>
  </header>

  <main id="main">
    <section class="hero">
      <div class="hero-glow" aria-hidden="true"></div>
      <div class="wrap">
        <p class="eyebrow">Free and open-source · Browser and desktop</p>
        <h1>Phonetics software that runs <RollingWord phrases={PHRASES} /></h1>
        <p class="lede">
          Phonia is a phonetics workstation for recording, annotating, and measuring speech,
          built to replace Praat for everyday research tasks. The analysis engine is written in
          Rust, compiled to WebAssembly for the browser build; a desktop build runs the same
          engine offline. Pitch, formant, and intensity measurements are checked against Praat
          under matched settings.
        </p>
        <div class="cta-row">
          <a class="btn btn-primary" href={appHref} onclick={handleOpenPhonia}>Open Phonia</a>
          <a class="btn btn-ghost" href="#capabilities">Browse capabilities</a>
          <span class="cta-note">Runs locally. Recordings are not uploaded.</span>
        </div>

        <div class="spec-frame" data-rv>
          <div class="panel-bar">
            <span class="cap">Wide-band spectrogram · 0–5500 Hz · synthesized vowel sequence</span>
            <div class="toggles" role="group" aria-label="Track overlays">
              <label class="pill">
                <input type="checkbox" bind:checked={showFormants} /><span class="dot"></span>Formants
              </label>
              <label class="pill amber">
                <input type="checkbox" bind:checked={showPitch} /><span class="dot"></span>Pitch
              </label>
            </div>
          </div>
          <div class="spec-canvas">
            <canvas bind:this={heroCanvas}
              >A spectrogram rendered from synthesized speech, with teal formant tracks.</canvas
            >
          </div>
        </div>
      </div>
    </section>

    <section id="capabilities">
      <div class="wrap">
        <div class="sec-head" data-rv>
          <p class="eyebrow">Capabilities</p>
          <h2>Record, measure, annotate, and plot.</h2>
        </div>
        <div class="cap-grid">
          <article class="card" data-rv>
            <div class="card-top"><span class="card-num">01</span><h3>Analyze Voice</h3></div>
            <p>
              Pitch, formants, intensity, and harmonicity measured from the signal and drawn over
              it. Values are validated against Praat's own routines.
            </p>
            <div class="viz">
              <canvas bind:this={wavCanvas}>A waveform with an amber pitch contour.</canvas>
            </div>
          </article>
          <article class="card" data-rv>
            <div class="card-top"><span class="card-num">02</span><h3>Manage Audio</h3></div>
            <p>
              Import audio files or record from the microphone, and organize a corpus of
              recordings into projects with groups, tags, and search.
            </p>
            <div class="viz viz-pad">
              <div class="file-row">
                <div>
                  <div class="file-name">rec_2024-05-12.wav</div>
                  <div class="file-meta">12:47 · 44.1 kHz · 16-bit</div>
                </div>
                <span class="wave" aria-hidden="true"
                  ><i style="height:30%"></i><i style="height:55%"></i><i style="height:82%"></i><i
                    style="height:44%"
                  ></i><i style="height:90%"></i><i style="height:60%"></i><i style="height:34%"
                  ></i><i style="height:70%"></i><i style="height:95%"></i><i style="height:50%"
                  ></i><i style="height:66%"></i><i style="height:40%"></i></span
                >
              </div>
              <div class="file-row">
                <div>
                  <div class="file-name">interview_a3.wav</div>
                  <div class="file-meta">08:03 · 48 kHz · 24-bit</div>
                </div>
                <span class="wave" aria-hidden="true"
                  ><i style="height:44%"></i><i style="height:72%"></i><i style="height:38%"></i><i
                    style="height:86%"
                  ></i><i style="height:58%"></i><i style="height:92%"></i><i style="height:47%"
                  ></i><i style="height:64%"></i><i style="height:80%"></i><i style="height:36%"
                  ></i><i style="height:56%"></i><i style="height:70%"></i></span
                >
              </div>
              <div class="file-row">
                <div>
                  <div class="file-name">wordlist_deny.wav</div>
                  <div class="file-meta">02:19 · 44.1 kHz · 16-bit</div>
                </div>
                <span class="wave" aria-hidden="true"
                  ><i style="height:62%"></i><i style="height:34%"></i><i style="height:78%"></i><i
                    style="height:52%"
                  ></i><i style="height:88%"></i><i style="height:42%"></i><i style="height:68%"
                  ></i><i style="height:94%"></i><i style="height:48%"></i><i style="height:74%"
                  ></i><i style="height:38%"></i><i style="height:58%"></i></span
                >
              </div>
            </div>
          </article>
          <article class="card" data-rv>
            <div class="card-top"><span class="card-num">03</span><h3>Show Spectrograms</h3></div>
            <p>
              Formant and pitch tracks drawn over the signal, with a choice of built-in color
              scales or a custom gradient.
            </p>
            <div class="viz">
              <canvas bind:this={miniCanvas}
                >A small spectrogram of the vowels i, a, u with formant tracks.</canvas
              >
            </div>
          </article>
          <article class="card" data-rv>
            <div class="card-top"><span class="card-num">04</span><h3>Annotate Recordings</h3></div>
            <p>
              Tiered intervals over the signal. Create boundaries by listening, nudge them by the
              sample, and export TextGrids.
            </p>
            <div class="viz">
              <svg viewBox="0 0 360 128" role="img" aria-label="Two annotation tiers: words and segments">
                <text x="24" y="18" font-family="ui-monospace,Menlo,monospace" font-size="9" fill="#7d786a" letter-spacing="1.5">WORDS</text>
                <rect x="63" y="26" width="234" height="34" fill="rgba(236,231,219,.035)" />
                <line x1="63" y1="26" x2="63" y2="60" stroke="#3a372e" />
                <line x1="297" y1="26" x2="297" y2="60" stroke="#3a372e" />
                <text x="180" y="47" text-anchor="middle" font-family="Georgia,serif" font-size="13" fill="#ece7db">Phonia</text>
                <text x="24" y="74" font-family="ui-monospace,Menlo,monospace" font-size="9" fill="#7d786a" letter-spacing="1.5">SEGMENTS</text>
                <rect x="110" y="80" width="47" height="34" fill="rgba(94,234,212,.12)" stroke="#5eead4" stroke-opacity=".55" />
                <g stroke="#3a372e">
                  <line x1="63" y1="80" x2="63" y2="114" /><line x1="110" y1="80" x2="110" y2="114" />
                  <line x1="157" y1="80" x2="157" y2="114" /><line x1="203" y1="80" x2="203" y2="114" />
                  <line x1="250" y1="80" x2="250" y2="114" /><line x1="297" y1="80" x2="297" y2="114" />
                </g>
                <g font-family="Georgia,serif" font-size="12" fill="#a8a294" text-anchor="middle">
                  <text x="86" y="101">f</text><text x="133" y="101" fill="#5eead4">o</text><text x="180" y="101">n</text>
                  <text x="226" y="101">i</text><text x="273" y="101">a</text>
                </g>
                <line x1="133" y1="10" x2="133" y2="120" stroke="#f5b04c" stroke-width="1.5" stroke-opacity=".85" />
              </svg>
            </div>
          </article>
          <article class="card" data-rv>
            <div class="card-top"><span class="card-num">05</span><h3>Draw Plots</h3></div>
            <p>
              Pitch contours and formant tracks exported as SVG, PDF, TikZ, Typst, or Vega-Lite,
              or as plotting code for Python, R, and Julia.
            </p>
            <div class="viz">
              <svg viewBox="0 0 360 226" role="img" aria-label="Vowel plot with F1 on the vertical axis and F2 on the horizontal axis">
                <line x1="40" y1="20" x2="40" y2="190" stroke="#3a372e" />
                <line x1="40" y1="190" x2="330" y2="190" stroke="#3a372e" />
                <g font-family="ui-monospace,Menlo,monospace" font-size="8.5" fill="#7d786a">
                  <text x="34" y="39" text-anchor="end">300</text>
                  <text x="34" y="100" text-anchor="end">500</text>
                  <text x="34" y="162" text-anchor="end">700</text>
                  <text x="108" y="203" text-anchor="middle">2000</text>
                  <text x="193" y="203" text-anchor="middle">1500</text>
                  <text x="278" y="203" text-anchor="middle">1000</text>
                  <text x="14" y="105" transform="rotate(-90 14 105)" text-anchor="middle">F1 (Hz)</text>
                  <text x="330" y="220" text-anchor="end">F2 (Hz)</text>
                </g>
                <polygon points="65.6,29.3 111.6,75.6 265.2,162.2 309.5,84.9 319.8,41.6" fill="rgba(94,234,212,.06)" stroke="rgba(94,234,212,.35)" stroke-dasharray="4 4" />
                <g fill="#5eead4">
                  <circle cx="65.6" cy="29.3" r="4.5" /><circle cx="111.6" cy="75.6" r="4.5" />
                  <circle cx="265.2" cy="162.2" r="4.5" /><circle cx="309.5" cy="84.9" r="4.5" />
                  <circle cx="319.8" cy="41.6" r="4.5" />
                </g>
                <g font-family="Georgia,serif" font-style="italic" font-size="13" fill="#ece7db">
                  <text x="56" y="22">i</text><text x="102" y="68">e</text>
                  <text x="273" y="180">a</text><text x="297" y="72">o</text>
                  <text x="312" y="30" text-anchor="end">u</text>
                </g>
              </svg>
            </div>
          </article>
        </div>
      </div>
    </section>

    <section id="learning">
      <div class="wrap">
        <div class="sec-head" data-rv>
          <p class="eyebrow">Learning</p>
          <h2>Interface conventions</h2>
        </div>
        <ul class="learn-list" data-rv>
          <li>
            <strong>Command palette (Ctrl/Cmd-K).</strong> Every action is listed with its shortcut,
            so nothing is buried two menus deep.
          </li>
          <li>
            <strong>One undo stack (Ctrl/Cmd-Z).</strong> Annotation edits and library changes —
            a renamed recording, a deleted tier — share the same history, so one shortcut reaches
            back through all of it.
          </li>
          <li>
            <strong>One selection.</strong> A range selected on the waveform is the same range on
            the spectrogram and in the tiers — playback, readouts, and export all use it.
          </li>
        </ul>
      </div>
    </section>

    <section id="validation">
      <div class="wrap">
        <div class="sec-head" data-rv>
          <p class="eyebrow">Validation</p>
          <h2>Every measurement is checked against Praat.</h2>
        </div>
        <div class="val-grid">
          <div class="val-text" data-rv>
            <p>
              <a href="https://www.fon.hum.uva.nl/praat/" rel="external">Praat</a> is the standard
              tool in phonetics research and teaching. Phonia's pitch, intensity, and formant
              measurements are compared against a Praat oracle under matched settings, each with a
              documented tolerance band. A continuous-integration job re-runs the comparison on
              every change; a measurement drifting past its tolerance fails the build.
            </p>
            <p>
              Most formant checks agree exactly; the remaining residual traces to Praat's own
              unpublished sinc-interpolation window, and is recorded in the repository's
              validation notes. Voice-report measures — jitter, shimmer, and harmonicity — are
              checked on sustained-vowel cases.
            </p>
          </div>
          <div data-rv>
            <table>
              <thead>
                <tr><th>Measure</th><th>Praat routine</th><th>Status</th></tr>
              </thead>
              <tbody>
                <tr><td>Pitch (F0)</td><td>Sound: To Pitch (ac)</td><td><span class="ok">Validated</span></td></tr>
                <tr><td>Formants F1–F3</td><td>Sound: To Formant (burg)</td><td><span class="ok">Validated</span></td></tr>
                <tr><td>Intensity</td><td>Sound: To Intensity</td><td><span class="ok">Validated</span></td></tr>
                <tr><td>Harmonicity</td><td>Sound: To Harmonicity (ac)</td><td><span class="ok">Validated</span></td></tr>
              </tbody>
            </table>
            <p class="tbl-note">Routine names as they appear in Praat.</p>
          </div>
        </div>
      </div>
    </section>

    <section id="app">
      <div class="wrap">
        <div class="sec-head" data-rv>
          <p class="eyebrow">The application</p>
          <h2>This frame runs the live application.</h2>
          <p class="sec-sub">
            The frame below loads the same build served at this site's root.
          </p>
        </div>
        <figure data-rv>
          <div class="shot-frame" bind:this={frameSection}>
            <div class="shot-chrome">
              <i></i><i></i><i></i>
              <span class="shot-title">phonia — live</span>
            </div>
            <div class="shot-body">
              <div class="app-fallback" class:hidden={frameReady && !frameFailed}>
                <svg width="44" height="44" viewBox="0 0 64 64" fill="none" aria-hidden="true">
                  <path d="M46.5 12.9 A 22 22 0 1 0 52.2 20.4" stroke="#5eead4" stroke-width="7" stroke-linecap="round" />
                  <path d="M14 36 C20 24 25 24 31 32 C37 40 41 40 50 24" stroke="#5eead4" stroke-width="7" stroke-linecap="round" />
                  <circle cx="52" cy="20" r="5.5" fill="#f5b04c" stroke="none" />
                </svg>
                <p>Open Phonia to see it live.</p>
                <a class="btn btn-ghost btn-sm" href={appHref} onclick={handleOpenPhonia}>Open Phonia</a>
              </div>
              {#if !frameFailed}
                <iframe
                  bind:this={appFrame}
                  src="/"
                  title="Phonia running live"
                  loading="lazy"
                  class:ready={frameReady}
                  onload={handleFrameLoad}
                ></iframe>
              {/if}
            </div>
          </div>
          <figcaption>The application in an embedded frame. Open Phonia in its own window to use it.</figcaption>
        </figure>
      </div>
    </section>

    <section class="final">
      <div class="wrap" data-rv>
        <h2>Open a sound and start measuring.</h2>
        <a class="btn btn-primary" href={appHref} onclick={handleOpenPhonia}>Open Phonia</a>
        <p class="fine">Free and open-source · Validated against Praat</p>
      </div>
    </section>
  </main>

  <footer>
    <div class="wrap foot">
      <div class="foot-brand">
        <svg width="26" height="26" viewBox="0 0 64 64" fill="none" aria-hidden="true">
          <path d="M46.5 12.9 A 22 22 0 1 0 52.2 20.4" stroke="#5eead4" stroke-width="7" stroke-linecap="round" />
          <path d="M14 36 C20 24 25 24 31 32 C37 40 41 40 50 24" stroke="#5eead4" stroke-width="7" stroke-linecap="round" />
          <circle cx="52" cy="20" r="5.5" fill="#f5b04c" stroke="none" />
        </svg>
        <span class="wordmark">Phonia</span>
      </div>
      <p>Free and open-source software · MIT / Apache-2.0 · <a href={appHref} onclick={handleOpenPhonia}>Open Phonia</a></p>
    </div>
  </footer>
</div>

<style>
  .landing {
    --bg: #1e1d1a;
    --bg-deep: #191813;
    --panel: #262420;
    --panel-2: #2b2923;
    --line: #3a372e;
    --line-soft: #2f2d26;
    --l-text: #ece7db;
    --l-muted: #a8a294;
    --l-faint: #7d786a;
    --l-teal: #5eead4;
    --l-teal-hi: #99f6e4;
    --l-amber: #f5b04c;
    --l-ink: #16211d;
    --l-serif: 'Iowan Old Style', 'Palatino Linotype', Palatino, Charter, Georgia, 'Times New Roman', serif;
    --l-sans: system-ui, -apple-system, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
    --l-mono: ui-monospace, 'SF Mono', 'Cascadia Mono', Menlo, Consolas, monospace;
    --wrap: min(1120px, 92vw);

    background: var(--bg);
    color: var(--l-text);
    color-scheme: dark;
    font-family: var(--l-sans);
    font-size: 16px;
    line-height: 1.6;
    -webkit-font-smoothing: antialiased;
    overflow-x: hidden;
    min-height: 100vh;
  }

  @media (prefers-reduced-motion: no-preference) {
    :global(html) {
      scroll-behavior: smooth;
    }
  }

  .landing ::selection {
    background: var(--l-amber);
    color: var(--bg);
  }

  .landing a {
    color: var(--l-teal);
    text-decoration: none;
    text-underline-offset: 3px;
  }

  .landing a:hover {
    color: var(--l-teal-hi);
  }

  .landing :focus-visible {
    outline: 2px solid var(--l-amber);
    outline-offset: 3px;
    border-radius: 6px;
  }

  .skip {
    position: absolute;
    left: -9999px;
    top: 0;
    background: var(--l-teal);
    color: var(--l-ink);
    padding: 0.6rem 1rem;
    z-index: 100;
    border-radius: 0 0 10px 0;
    font-family: var(--l-mono);
    font-size: 13px;
  }

  .skip:focus {
    left: 0;
  }

  .wrap {
    width: var(--wrap);
    margin: 0 auto;
  }

  .site-header {
    position: sticky;
    top: 0;
    z-index: 50;
    background: rgba(30, 29, 26, 0.82);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid rgba(58, 55, 46, 0.55);
  }

  .header-in {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 64px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    color: var(--l-text);
  }

  .brand svg {
    display: block;
  }

  .wordmark {
    font-family: var(--l-serif);
    font-size: 1.4rem;
    font-weight: 500;
    letter-spacing: 0.01em;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 1.8rem;
  }

  .nav a {
    color: var(--l-muted);
    font-size: 0.95rem;
  }

  .nav a:hover {
    color: var(--l-text);
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-family: var(--l-sans);
    font-weight: 600;
    font-size: 0.95rem;
    border-radius: 999px;
    padding: 0.7rem 1.5rem;
    cursor: pointer;
    border: 1px solid transparent;
    transition:
      transform 0.18s ease,
      background 0.18s ease,
      border-color 0.18s ease,
      color 0.18s ease;
  }

  .btn.btn-primary {
    background: var(--l-teal);
    color: var(--l-ink);
  }

  .btn.btn-primary:hover {
    background: var(--l-teal-hi);
    color: var(--l-ink);
    transform: translateY(-1px);
  }

  .btn-sm {
    padding: 0.45rem 1.1rem;
    font-size: 0.88rem;
  }

  .btn-ghost {
    border-color: var(--line);
    color: var(--l-text);
    background: transparent;
  }

  .btn-ghost:hover {
    border-color: var(--l-teal);
    color: var(--l-teal);
    transform: translateY(-1px);
  }

  .hero {
    position: relative;
    padding: 7rem 0 5rem;
  }

  .hero-glow {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      radial-gradient(46rem 22rem at 18% -6%, rgba(94, 234, 212, 0.055), transparent 60%),
      radial-gradient(40rem 20rem at 86% 8%, rgba(245, 176, 76, 0.05), transparent 60%);
  }

  .eyebrow {
    font-family: var(--l-mono);
    font-size: 0.75rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--l-amber);
    margin: 0 0 1.4rem;
  }

  h1 {
    font-family: var(--l-serif);
    font-weight: 500;
    font-size: clamp(1.6rem, 7.5vw, 4.2rem);
    line-height: 1.08;
    letter-spacing: -0.01em;
    max-width: 16ch;
    margin: 0;
  }

  .lede {
    max-width: 44rem;
    margin: 1.5rem 0 0;
    color: var(--l-muted);
    font-size: 1.08rem;
  }

  .cta-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 2.2rem;
    flex-wrap: wrap;
  }

  .cta-note {
    font-family: var(--l-mono);
    font-size: 0.8rem;
    color: var(--l-faint);
  }

  .spec-frame {
    margin-top: 3.5rem;
    background: var(--bg-deep);
    border: 1px solid var(--line);
    border-radius: 20px;
    box-shadow: 0 40px 90px -40px rgba(0, 0, 0, 0.65);
    overflow: hidden;
    position: relative;
  }

  .panel-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    padding: 0.85rem 1.2rem;
    border-bottom: 1px solid var(--line-soft);
  }

  .panel-bar .cap {
    font-family: var(--l-mono);
    font-size: 0.78rem;
    color: var(--l-muted);
  }

  .toggles {
    display: flex;
    gap: 0.6rem;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.32rem 0.85rem;
    border: 1px solid var(--line);
    border-radius: 999px;
    font-family: var(--l-mono);
    font-size: 0.75rem;
    color: var(--l-muted);
    cursor: pointer;
    user-select: none;
    transition:
      border-color 0.15s ease,
      color 0.15s ease,
      background 0.15s ease;
  }

  .pill input {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
  }

  .pill .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    opacity: 0.3;
  }

  .pill:has(input:checked) {
    color: var(--l-teal);
    border-color: rgba(94, 234, 212, 0.5);
    background: rgba(94, 234, 212, 0.06);
  }

  .pill:has(input:checked) .dot {
    opacity: 1;
  }

  .pill.amber:has(input:checked) {
    color: var(--l-amber);
    border-color: rgba(245, 176, 76, 0.5);
    background: rgba(245, 176, 76, 0.06);
  }

  .spec-canvas {
    height: clamp(300px, 44vw, 440px);
    position: relative;
  }

  .spec-canvas canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
  }

  section {
    padding: 5.5rem 0;
  }

  .sec-head {
    margin-bottom: 3rem;
    max-width: 46rem;
  }

  h2 {
    font-family: var(--l-serif);
    font-weight: 500;
    font-size: clamp(1.8rem, 3.4vw, 2.5rem);
    line-height: 1.15;
    letter-spacing: -0.01em;
    margin: 0;
  }

  .sec-sub {
    margin: 1rem 0 0;
    color: var(--l-muted);
  }

  .cap-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 1.2rem;
  }

  .card {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 18px;
    padding: 1.3rem 1.3rem 1.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    transition:
      transform 0.2s ease,
      border-color 0.2s ease,
      background 0.2s ease;
  }

  .card:hover {
    transform: translateY(-3px);
    border-color: rgba(94, 234, 212, 0.35);
    background: var(--panel-2);
  }

  .card-top {
    display: flex;
    align-items: baseline;
    gap: 0.7rem;
  }

  .card-num {
    font-family: var(--l-mono);
    font-size: 0.75rem;
    color: var(--l-amber);
  }

  .card h3 {
    font-size: 1.05rem;
    font-weight: 600;
    letter-spacing: 0.005em;
    margin: 0;
  }

  .card p {
    font-size: 0.92rem;
    color: var(--l-muted);
    margin: 0;
  }

  .viz {
    margin-top: auto;
    height: 150px;
    border-radius: 12px;
    background: var(--bg-deep);
    border: 1px solid var(--line-soft);
    position: relative;
    overflow: hidden;
  }

  .viz canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
  }

  .viz svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
  }

  .viz-pad {
    padding: 0.6rem 0.8rem;
    height: auto;
    min-height: 150px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .file-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.55rem 0.15rem;
    border-bottom: 1px dashed var(--line-soft);
  }

  .file-row:last-child {
    border-bottom: none;
  }

  .file-name {
    font-family: var(--l-mono);
    font-size: 0.78rem;
    color: var(--l-text);
  }

  .file-meta {
    font-family: var(--l-mono);
    font-size: 0.7rem;
    color: var(--l-faint);
    margin-top: 0.1rem;
  }

  .wave {
    display: inline-flex;
    align-items: center;
    height: 26px;
  }

  .wave i {
    display: inline-block;
    width: 3px;
    border-radius: 2px;
    background: #57534a;
    margin-right: 2px;
    transition: background 0.15s ease;
  }

  .file-row:hover .wave i {
    background: rgba(94, 234, 212, 0.55);
  }

  .learn-list {
    max-width: 46rem;
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .learn-list li {
    color: var(--l-muted);
    padding-left: 1.1rem;
    position: relative;
  }

  .learn-list li::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0.55em;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--l-teal);
  }

  .learn-list strong {
    color: var(--l-text);
    font-weight: 600;
  }

  .val-grid {
    display: grid;
    grid-template-columns: 1fr 1.15fr;
    gap: 3rem;
    align-items: start;
  }

  .val-text p {
    color: var(--l-muted);
    margin: 0 0 1rem;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
  }

  th {
    font-family: var(--l-mono);
    font-size: 0.7rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--l-faint);
    text-align: left;
    padding: 0.6rem 0.4rem;
    border-bottom: 1px solid var(--line);
  }

  td {
    padding: 0.75rem 0.4rem;
    border-bottom: 1px solid var(--line-soft);
    color: var(--l-muted);
  }

  td:first-child {
    color: var(--l-text);
  }

  td .ok {
    color: var(--l-teal);
    font-family: var(--l-mono);
    font-size: 0.8rem;
  }

  .tbl-note {
    margin-top: 0.9rem;
    font-family: var(--l-mono);
    font-size: 0.72rem;
    color: var(--l-faint);
  }

  figure {
    margin: 0;
  }

  .shot-frame {
    border: 1px solid var(--line);
    border-radius: 16px;
    overflow: hidden;
    background: var(--bg-deep);
  }

  .shot-chrome {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    height: 34px;
    padding: 0 0.9rem;
    border-bottom: 1px solid var(--line-soft);
    background: var(--panel);
  }

  .shot-chrome i {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #4a463c;
  }

  .shot-title {
    margin-left: auto;
    margin-right: auto;
    font-family: var(--l-mono);
    font-size: 0.68rem;
    color: var(--l-faint);
    transform: translateX(-1.2rem);
  }

  .shot-body {
    aspect-ratio: 16 / 10;
    position: relative;
  }

  .shot-body iframe {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    border: 0;
    display: block;
    background: var(--bg-deep);
    opacity: 0;
    transition: opacity 0.6s ease;
  }

  .shot-body iframe.ready {
    opacity: 1;
  }

  .app-fallback {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.9rem;
    text-align: center;
    padding: 1.5rem;
  }

  .app-fallback p {
    margin: 0;
    color: var(--l-muted);
    font-size: 0.95rem;
  }

  .app-fallback.hidden {
    visibility: hidden;
  }

  figcaption {
    margin-top: 0.7rem;
    font-family: var(--l-mono);
    font-size: 0.75rem;
    color: var(--l-muted);
  }

  .final {
    padding: 6rem 0 7rem;
    text-align: center;
  }

  .final h2 {
    margin: 0 auto;
    max-width: 20ch;
  }

  .final .btn {
    margin-top: 2.2rem;
  }

  .final .fine {
    margin-top: 1.1rem;
    font-family: var(--l-mono);
    font-size: 0.75rem;
    color: var(--l-faint);
  }

  footer {
    border-top: 1px solid var(--line-soft);
    padding: 2rem 0 2.5rem;
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1.5rem;
    flex-wrap: wrap;
  }

  .foot-brand {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }

  .foot-brand .wordmark {
    font-size: 1.1rem;
  }

  .foot p {
    font-family: var(--l-mono);
    font-size: 0.75rem;
    color: var(--l-faint);
    margin: 0;
  }

  .foot a {
    color: var(--l-muted);
  }

  .foot a:hover {
    color: var(--l-teal);
  }

  /* Content is visible by default — a crawler, a share-preview generator, or
     a full-page capture taken without scrolling must see every section, not
     a hero followed by blank space. `.rvl` on its own has no visual effect;
     scrolling a section into view only adds a one-shot rise-and-fade played
     via the `.in` class's animation, matching where the content already
     sits. An element that never earns `.in` — no JS, or never scrolled to —
     simply never animates and stays at its normal, fully visible resting
     state. */
  .landing :global(.rvl.in) {
    animation: rv-reveal 0.7s cubic-bezier(0.2, 0.6, 0.2, 1);
  }

  @keyframes rv-reveal {
    from {
      opacity: 0;
      transform: translateY(16px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: no-preference) {
    .mark-ring {
      stroke-dasharray: 100;
      stroke-dashoffset: 100;
      animation: draw-in 1.3s cubic-bezier(0.4, 0, 0.2, 1) 0.15s forwards;
    }

    .mark-line {
      stroke-dasharray: 100;
      stroke-dashoffset: 100;
      animation: draw-in 1s cubic-bezier(0.4, 0, 0.2, 1) 0.55s forwards;
    }

    .mark-dot {
      opacity: 0;
      animation: fade-in 0.4s ease 1.25s forwards;
    }

    @keyframes draw-in {
      to {
        stroke-dashoffset: 0;
      }
    }

    @keyframes fade-in {
      to {
        opacity: 1;
      }
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .landing *,
    .landing *::before,
    .landing *::after {
      animation: none !important;
      transition: none !important;
    }

    .shot-body iframe {
      transition: none;
    }
  }

  @media (max-width: 860px) {
    .val-grid {
      grid-template-columns: 1fr;
    }

    .nav .nav-link {
      display: none;
    }

    .hero {
      padding-top: 5rem;
    }
  }
</style>
