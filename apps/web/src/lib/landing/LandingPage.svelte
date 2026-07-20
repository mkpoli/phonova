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

  // The rotating half of "phonetics ___" under the hero title. Each phrase
  // names a real way to run Phonia; the rotation itself is the point — see
  // the reveal/verify notes in the project's landing-page work log.
  const PLATFORM_PHRASES = ['in your browser', 'on your phone', 'on the desktop', 'offline'];

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

  // The embed's maximize dot has no href to navigate through on its own, so
  // it drives the same handoff as every other "Open Phonia" control and then
  // — only when that handoff didn't already swap the view in place — sends
  // the browser to appHref itself. `appHref` is already origin-aware (it
  // points at phonia.app from the about subdomain), so this is the same
  // mechanism the anchors use, not a second one.
  function handleMaximize(event: MouseEvent) {
    handleOpenPhonia(event);
    if (!(onEnterApp && appHref === '/')) {
      window.location.href = appHref;
    }
  }

  let root = $state<HTMLElement | null>(null);
  let heroCanvas = $state<HTMLCanvasElement | null>(null);
  let sideCanvas = $state<HTMLCanvasElement | null>(null);
  let miniCanvas = $state<HTMLCanvasElement | null>(null);
  let wavCanvas = $state<HTMLCanvasElement | null>(null);

  let showFormants = $state(true);
  let showPitch = $state(false);

  let frameReady = $state(false);
  let frameFailed = $state(false);
  let appFrame = $state<HTMLIFrameElement | null>(null);
  let frameSection = $state<HTMLElement | null>(null);

  // The embed's mock title-bar dots are wired to real state. Never persisted:
  // every fresh load starts 'open', regardless of what a previous visit did.
  let embedState = $state<'open' | 'minimized' | 'closed'>('open');

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

  // The workspace-demo iframe always needs both `app=1` (skip the landing
  // gate — see the recursion note in `apps/web/src/routes/+page.svelte`) and
  // `sample=1` (auto-open the bundled sample recording into the editor, so
  // the frame shows a working analysis instead of the empty home screen).
  const embedSrc =
    typeof window !== 'undefined' && window.location.hostname === 'about.phonia.app'
      ? 'https://phonia.app/?app=1&sample=1'
      : '/?app=1&sample=1';

  function handleFrameLoad() {
    // The embedded app is a client-rendered SPA: `load` fires once the
    // document and its script bundle have arrived, before Svelte has
    // mounted anything into the page, so the frame's body is briefly empty
    // on every successful load — and the auto-opened sample then still has
    // to boot the engine, import the recording, and render the first
    // analysis before the frame looks "ready" to a visitor. That rules out
    // an emptiness check here — the failure timer in onMount is the sole
    // failure signal (no `load` at all within the window means the frame is
    // genuinely stuck, blocked, or offline).
    frameReady = true;
  }

  onMount(() => {
    // The iframe is `loading="lazy"`, so the browser only starts fetching it
    // once the frame nears the viewport — on a normal scroll pace that can
    // be well past the failure window after mount. Starting the failure
    // timer at mount would flag a slow scroller's frame as failed before it
    // ever began loading, so the timer starts only once the frame's section
    // is close enough to the viewport that the browser's own lazy-load has
    // plausibly kicked in.
    let timer: ReturnType<typeof setTimeout> | null = null;
    let frameObserver: IntersectionObserver | null = null;
    if (frameSection) {
      frameObserver = new IntersectionObserver(
        (entries) => {
          if (entries.some((e) => e.isIntersecting)) {
            // Generous: the frame loads the whole app (engine included),
            // imports the sample, and runs an analysis before it settles.
            timer = setTimeout(() => {
              if (!frameReady) frameFailed = true;
            }, 12000);
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
        ctx.strokeStyle = 'rgba(244,236,220,0.055)';
        ctx.lineWidth = 1;
        ctx.fillStyle = 'rgba(185,178,164,0.85)';
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
          ctx.fillStyle = 'rgba(20,19,17,0.62)';
          ctx.fillRect(px, p.t, p.w - prog * p.w, p.h);
        }
        ctx.save();
        ctx.beginPath();
        ctx.rect(p.l, p.t, prog * p.w, p.h);
        ctx.clip();
        if (showPitch) strokeTrack(ctx, spec, p, spec.f0Arr, '#9cc4ff', 2.2);
        if (showFormants) {
          for (let k = 0; k < 3; k++) {
            strokeTrack(ctx, spec, p, spec.form[k], '#35c9b4', 2.2, (c) => spec.ampArr[c] > 0.05);
          }
        }
        ctx.restore();
        if (prog > 0 && prog < 1) {
          ctx.save();
          ctx.strokeStyle = 'rgba(212,149,58,0.95)';
          ctx.lineWidth = 2;
          ctx.shadowColor = 'rgba(212,149,58,0.8)';
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
          ctx.fillStyle = 'rgba(29,29,26,' + (f * 0.9).toFixed(3) + ')';
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

    // The blurred background panel behind the hero's main panel — a second,
    // smaller synthesized spectrogram, static (no sweep) since it only ever
    // reads as depth, not as a second demo.
    const side = sideCanvas;
    if (side) {
      const spec = buildSpec({ cols: 300, rows: 160, maxF: 5000, dur: 2.4, keys: MINI_KEYS });
      const off = specToCanvas(spec);
      const ctx = side.getContext('2d')!;
      const render = () => {
        const r = side.getBoundingClientRect();
        const dpr = Math.min(2, window.devicePixelRatio || 1);
        const W = r.width;
        const H = r.height;
        side.width = Math.round(W * dpr);
        side.height = Math.round(H * dpr);
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.clearRect(0, 0, W, H);
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.drawImage(off, 0, 0, W, H);
      };
      new ResizeObserver(render).observe(side);
      render();
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
          strokeTrack(ctx, spec, p, spec.form[k], '#35c9b4', 1.6, (c) => spec.ampArr[c] > 0.06);
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
        ctx.strokeStyle = '#d4953a';
        ctx.lineWidth = 2;
        ctx.lineCap = 'round';
        ctx.lineJoin = 'round';
        ctx.shadowColor = 'rgba(212,149,58,0.7)';
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
        ctx.fillStyle = 'rgba(212,149,58,0.55)';
        ctx.font = '9px ui-monospace,Menlo,Consolas,monospace';
        ctx.textAlign = 'right';
        ctx.textBaseline = 'top';
        ctx.fillText('pitch, Hz', W - 10, 8);
      };
      new ResizeObserver(render).observe(wav);
      render();
    }

    // Hero parallax: the scene tilts a couple of degrees toward the pointer,
    // on top of each panel's own static rake. A static rotation alone reads
    // as a flat, skewed card; the added motion is what sells the panels as
    // objects sitting in actual depth. Skipped for touch input (no hover to
    // drive it) and under reduced motion.
    let parallaxRaf = 0;
    if (!rm && root && window.matchMedia('(pointer: fine)').matches) {
      const heroEl = root.querySelector<HTMLElement>('.hero');
      const sceneEl = root.querySelector<HTMLElement>('.scene');
      if (heroEl && sceneEl) {
        let tx = 0;
        let ty = 0;
        let cx = 0;
        let cy = 0;
        const onMove = (event: PointerEvent) => {
          tx = (event.clientX / window.innerWidth - 0.5) * 2;
          ty = (event.clientY / window.innerHeight - 0.5) * 2;
        };
        heroEl.addEventListener('pointermove', onMove);
        const loop = () => {
          cx += (tx - cx) * 0.045;
          cy += (ty - cy) * 0.045;
          sceneEl.style.transform = `rotateY(${(cx * 2.4).toFixed(2)}deg) rotateX(${(-cy * 1.6).toFixed(2)}deg)`;
          parallaxRaf = requestAnimationFrame(loop);
        };
        parallaxRaf = requestAnimationFrame(loop);
      }
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
      cancelAnimationFrame(parallaxRaf);
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
        <svg width="32" height="32" viewBox="0 0 64 64" fill="none" aria-hidden="true">
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
        <a class="nav-link" href="#source">Source code</a>
        <a class="btn btn-primary btn-sm" href={appHref} onclick={handleOpenPhonia}>Open Phonia</a>
      </nav>
    </div>
  </header>

  <main id="main">
    <section class="hero">
      <div class="scene" aria-hidden="true">
        <div class="glow glow-a"></div>
        <div class="glow glow-b"></div>
        <div class="glow glow-c"></div>

        <div class="panel panel-side">
          <div class="p-chrome">
            <i></i><i></i><i></i><span>vowels_perturbed.wav</span>
          </div>
          <div class="p-body p-spec"><canvas bind:this={sideCanvas}></canvas></div>
        </div>

        <div class="panel panel-main">
          <div class="p-chrome">
            <i></i><i></i><i></i><span>arctic_bdl_a0001 — 16 000 Hz · 1 ch</span><em>Phonia</em>
          </div>
          <div class="p-body p-spec">
            <canvas bind:this={heroCanvas}
              >A spectrogram rendered from synthesized speech, with teal formant tracks.</canvas
            >
          </div>
          <div class="p-tiers">
            <div class="p-tier"><span>WORDS</span></div>
            <div class="p-tier"><span>PHONES</span></div>
          </div>
        </div>

        <div class="chip chip-a float-slow">F1 <b>612 Hz</b> F2 <b>1 604 Hz</b></div>
        <div class="chip chip-b float-alt">f₀ <s>182 Hz</s> · voiced</div>
      </div>

      <div class="scrim" aria-hidden="true"></div>
      <div class="vignette" aria-hidden="true"></div>

      <div class="hero-content">
        <p class="eyebrow">Free and open-source · Browser and desktop</p>
        <p class="ipa" aria-label="Phonia, transcribed">
          <svg
            class="ipa-svg"
            viewBox="0 0 10803.8 2346.8"
            xmlns="http://www.w3.org/2000/svg"
            role="img"
            aria-hidden="true"
            ><g transform="translate(-133.1,1691.9) scale(1,-1)"
              ><g class="ipa-b"
                ><path
                  transform="translate(0.0,0.0)"
                  d="M256 1556H823V1436H610V-150H823V-270H256Z"
                /></g
              ><g class="ipa-t"
                ><path
                  transform="translate(969.0,0.0)"
                  d="M212 1077V1555H360V1077Z"
                /><path
                  transform="translate(1532.0,0.0)"
                  d="M420 584V479Q420 287 493.5 186.5Q567 86 707 86Q848 86 920.5 199.0Q993 312 993 532Q993 753 920.5 865.0Q848 977 707 977Q567 977 493.5 876.0Q420 775 420 584ZM236 956H59V1063H420V897Q474 997 557.5 1044.5Q641 1092 764 1092Q960 1092 1084.0 937.0Q1208 782 1208 532Q1208 282 1084.0 126.5Q960 -29 764 -29Q641 -29 557.5 18.5Q474 66 420 166V-319H594V-426H59V-319H236Z"
                /><path
                  transform="translate(2843.0,0.0)"
                  d="M52 668V727H155V1480H46V1539H269V1157Q301 1218 351 1249Q402 1280 469 1280Q578 1280 629 1223Q681 1166 681 1047V727H782V668H467V727H566V1014Q566 1123 537 1163Q507 1203 432 1203Q352 1203 311 1151Q269 1099 269 1000V727H368V668Z"
                /><path
                  transform="translate(3729.0,0.0)"
                  d="M616 70Q764 70 839.5 187.0Q915 304 915 532Q915 760 839.5 876.5Q764 993 616 993Q468 993 392.5 876.5Q317 760 317 532Q317 304 393.0 187.0Q469 70 616 70ZM616 -29Q384 -29 243.0 124.5Q102 278 102 532Q102 786 242.5 939.0Q383 1092 616 1092Q849 1092 989.5 939.0Q1130 786 1130 532Q1130 278 989.5 124.5Q849 -29 616 -29Z"
                /><path
                  transform="translate(4857.0,0.0)"
                  d="M-271 -219V-367H-436V-532H-584V-367H-749V-219Z"
                /><path
                  transform="translate(4962.0,0.0)"
                  d="M474 0H216L315 268H376ZM216 888H474L375 620H314Z"
                /></g
              ><g class="ipa-b"
                ><path
                  transform="translate(5652.0,0.0)"
                  d="M166 162Q166 240 222.0 296.0Q278 352 356 352Q435 352 491.0 296.0Q547 240 547 162Q547 83 491.0 27.0Q435 -29 356 -29Q278 -29 222.0 27.0Q166 83 166 162Z"
                /></g
              ><g class="ipa-t"
                ><path
                  transform="translate(6365.0,0.0)"
                  d="M84 0V106H250V956H74V1063H434V874Q485 982 566.5 1037.0Q648 1092 756 1092Q932 1092 1015.0 991.0Q1098 890 1098 676V106H1262V0H754V106H913V618Q913 813 865.0 885.5Q817 958 696 958Q568 958 501.0 864.5Q434 771 434 592V106H594V0Z"
                /><path
                  transform="translate(7684.0,0.0)"
                  d="M366 1513Q394 1513 415.0 1494.0Q436 1475 436.0 1449.0Q436 1423 416 1404Q396 1386 366 1386Q337 1386 316 1404Q295 1423 295.0 1449.0Q295 1475 316.0 1494.0Q337 1513 366 1513ZM323 1204H215V1264H437V611Q437 519 377 467Q316 414 210 414Q166 414 125 423Q84 433 46 451V574H105Q109 520 135 495Q162 470 213 470Q269 470 296 504Q323 539 323 611Z"
                /><path
                  transform="translate(8225.0,0.0)"
                  d="M199 1393Q199 1439 232.5 1473.0Q266 1507 313 1507Q359 1507 392.5 1473.0Q426 1439 426 1393Q426 1346 393.0 1313.0Q360 1280 313 1280Q266 1280 232.5 1313.0Q199 1346 199 1393ZM434 106H608V0H74V106H250V956H74V1063H434Z"
                /><path
                  transform="translate(9065.0,0.0)"
                  d="M-199 -443H-295Q-308 -364 -360.0 -328.5Q-412 -293 -512.0 -293.0Q-612 -293 -664.0 -328.5Q-716 -364 -729 -443H-825Q-815 -300 -736.0 -228.0Q-657 -156 -512.0 -156.0Q-367 -156 -288.0 -228.0Q-209 -300 -199 -443Z"
                /><path
                  transform="translate(8880.0,0.0)"
                  d="M815 334V559H578Q441 559 374.0 500.0Q307 441 307 319Q307 208 375.0 143.0Q443 78 559 78Q674 78 744.5 149.0Q815 220 815 334ZM999 664V106H1163V0H815V115Q754 41 674.0 6.0Q594 -29 487 -29Q310 -29 206.0 65.0Q102 159 102 319Q102 484 221.0 575.0Q340 666 557 666H815V739Q815 860 741.5 926.5Q668 993 535 993Q425 993 360.0 943.0Q295 893 279 795H184V1010Q280 1051 370.5 1071.5Q461 1092 547 1092Q768 1092 883.5 982.5Q999 873 999 664ZM374 1569Q422 1569 456.5 1534.5Q491 1500 491 1452Q491 1401 458.0 1368.0Q425 1335 374 1335Q324 1335 291.0 1368.0Q258 1401 258 1452Q258 1500 292.5 1534.5Q327 1569 374 1569ZM764 1569Q811 1569 845.5 1534.5Q880 1500 880 1452Q880 1401 847.0 1368.0Q814 1335 764 1335Q713 1335 680.0 1368.0Q647 1401 647 1452Q647 1500 681.5 1534.5Q716 1569 764 1569Z"
                /></g
              ><g class="ipa-b"
                ><path
                  transform="translate(10101.0,0.0)"
                  d="M713 1556V-270H145V-150H358V1436H145V1556Z"
                /></g
              ></g
            ></svg
          >
        </p>
        <h1>See what you hear.</h1>
        <p class="roll-line">phonetics <RollingWord phrases={PLATFORM_PHRASES} /></p>
        <p class="lede">
          Waveform and spectrogram sit on one timeline with pitch, formant, and intensity tracks,
          so a measurement always lines up with the sound that made it. Record or import audio,
          mark it up in tiers, and export the result.
        </p>
        <div class="cta-row">
          <a class="cta" href="#demo">Start analyzing</a>
          <span class="cta-note">Runs locally — nothing is uploaded.</span>
        </div>
      </div>

      <a class="scrollcue" href="#demo" aria-label="Scroll down to the workspace">
        <span>Scroll</span>
        <svg width="14" height="9" viewBox="0 0 14 9" fill="none" aria-hidden="true">
          <path
            d="M1 1.5 7 7.5 13 1.5"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </a>
    </section>

    <section id="demo" class="section demo">
      <div class="wrap">
        <p class="kicker">The workspace</p>
        <h2>One window, the whole signal.</h2>
        <p class="snote">
          Waveform, spectrogram, pitch and formant tracks, and annotation tiers share one
          timeline. The frame below runs the same build served at this site's root, opened
          straight into the bundled sample recording.
        </p>

        <figure data-rv>
          {#if embedState === 'closed'}
            <div class="shot-closed">
              <p>The embedded preview was closed.</p>
              <button type="button" class="btn btn-ghost btn-sm" onclick={() => (embedState = 'open')}>
                Show the preview again
              </button>
            </div>
          {:else}
            <div class="shot-frame" bind:this={frameSection}>
              <div class="shot-chrome" class:collapsed={embedState === 'minimized'}>
                <button
                  type="button"
                  class="chrome-dot dot-close"
                  aria-label="Close preview"
                  onclick={() => (embedState = 'closed')}
                ></button>
                <button
                  type="button"
                  class="chrome-dot dot-min"
                  aria-label={embedState === 'minimized' ? 'Restore preview' : 'Minimize preview'}
                  onclick={() => (embedState = embedState === 'minimized' ? 'open' : 'minimized')}
                ></button>
                <button
                  type="button"
                  class="chrome-dot dot-max"
                  aria-label="Open Phonia"
                  onclick={handleMaximize}
                ></button>
                {#if embedState === 'minimized'}
                  <button
                    type="button"
                    class="shot-title shot-title-btn"
                    aria-label="Restore preview"
                    onclick={() => (embedState = 'open')}
                  >
                    arctic_bdl_a0001 — Phonia
                  </button>
                {:else}
                  <span class="shot-title">arctic_bdl_a0001 — Phonia</span>
                {/if}
              </div>
              <div class="shot-collapse" class:collapsed={embedState === 'minimized'}>
                <div class="shot-collapse-inner" inert={embedState === 'minimized' ? true : undefined}>
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
                        src={embedSrc}
                        title="Phonia, running live, with the sample recording open and analysed"
                        loading="lazy"
                        class:ready={frameReady}
                        onload={handleFrameLoad}
                      ></iframe>
                    {/if}
                  </div>
                </div>
              </div>
            </div>
          {/if}
          <figcaption>
            {#if embedState === 'minimized'}
              The live application, embedded and collapsed.
            {:else if embedState === 'closed'}
              The live application, embedded — closed.
            {:else}
              The live application, embedded, with the sample recording open.
            {/if}
          </figcaption>
        </figure>
        <div class="app-cta">
          <a class="btn btn-primary" href={appHref} onclick={handleOpenPhonia}>Open the full app</a>
        </div>
      </div>
    </section>

    <section id="capabilities" class="section">
      <div class="wrap">
        <p class="kicker">Capabilities</p>
        <h2>The daily tools of speech analysis.</h2>
        <p class="snote">Each one is a first-class part of the workspace, measured against the same timeline.</p>
        <div class="grid">
          <article class="card" data-rv>
            <p class="idx">01</p>
            <h3>Analyze Voice</h3>
            <p class="cdesc">
              Pitch, formants, intensity, and harmonicity measured from the signal and drawn over
              it. Values are checked against Praat's own routines.
            </p>
            <div class="viz">
              <canvas bind:this={wavCanvas}>A waveform with an amber pitch contour.</canvas>
            </div>
          </article>
          <article class="card" data-rv>
            <p class="idx">02</p>
            <h3>Manage Audio</h3>
            <p class="cdesc">
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
            <p class="idx">03</p>
            <h3>Show Spectrograms</h3>
            <p class="cdesc">
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
            <p class="idx">04</p>
            <h3>Annotate Recordings</h3>
            <p class="cdesc">
              Tiered intervals over the signal. Create boundaries by listening, nudge them by the
              sample, and export TextGrids.
            </p>
            <div class="viz">
              <svg viewBox="0 0 360 128" role="img" aria-label="Two annotation tiers: words and segments">
                <text x="24" y="18" font-family="ui-monospace,Menlo,monospace" font-size="9" fill="#7d786c" letter-spacing="1.5">WORDS</text>
                <rect x="63" y="26" width="234" height="34" fill="rgba(244,236,220,.035)" />
                <line x1="63" y1="26" x2="63" y2="60" stroke="#3a372e" />
                <line x1="297" y1="26" x2="297" y2="60" stroke="#3a372e" />
                <text x="180" y="47" text-anchor="middle" font-family="Georgia,serif" font-size="13" fill="#f4ecdc">Phonia</text>
                <text x="24" y="74" font-family="ui-monospace,Menlo,monospace" font-size="9" fill="#7d786c" letter-spacing="1.5">SEGMENTS</text>
                <rect x="110" y="80" width="47" height="34" fill="rgba(53,201,180,.12)" stroke="#35c9b4" stroke-opacity=".55" />
                <g stroke="#3a372e">
                  <line x1="63" y1="80" x2="63" y2="114" /><line x1="110" y1="80" x2="110" y2="114" />
                  <line x1="157" y1="80" x2="157" y2="114" /><line x1="203" y1="80" x2="203" y2="114" />
                  <line x1="250" y1="80" x2="250" y2="114" /><line x1="297" y1="80" x2="297" y2="114" />
                </g>
                <g font-family="Georgia,serif" font-size="12" fill="#b9b2a4" text-anchor="middle">
                  <text x="86" y="101">f</text><text x="133" y="101" fill="#35c9b4">o</text><text x="180" y="101">n</text>
                  <text x="226" y="101">i</text><text x="273" y="101">a</text>
                </g>
                <line x1="133" y1="10" x2="133" y2="120" stroke="#d4953a" stroke-width="1.5" stroke-opacity=".85" />
              </svg>
            </div>
          </article>
          <article class="card" data-rv>
            <p class="idx">05</p>
            <h3>Draw Plots</h3>
            <p class="cdesc">
              Pitch contours and formant tracks exported as SVG, PDF, TikZ, Typst, or Vega-Lite,
              or as plotting code for Python, R, and Julia.
            </p>
            <div class="viz">
              <svg viewBox="0 0 360 226" role="img" aria-label="Vowel plot with F1 on the vertical axis and F2 on the horizontal axis">
                <line x1="40" y1="20" x2="40" y2="190" stroke="#3a372e" />
                <line x1="40" y1="190" x2="330" y2="190" stroke="#3a372e" />
                <g font-family="ui-monospace,Menlo,monospace" font-size="8.5" fill="#7d786c">
                  <text x="34" y="39" text-anchor="end">300</text>
                  <text x="34" y="100" text-anchor="end">500</text>
                  <text x="34" y="162" text-anchor="end">700</text>
                  <text x="108" y="203" text-anchor="middle">2000</text>
                  <text x="193" y="203" text-anchor="middle">1500</text>
                  <text x="278" y="203" text-anchor="middle">1000</text>
                  <text x="14" y="105" transform="rotate(-90 14 105)" text-anchor="middle">F1 (Hz)</text>
                  <text x="330" y="220" text-anchor="end">F2 (Hz)</text>
                </g>
                <polygon points="65.6,29.3 111.6,75.6 265.2,162.2 309.5,84.9 319.8,41.6" fill="rgba(53,201,180,.06)" stroke="rgba(53,201,180,.35)" stroke-dasharray="4 4" />
                <g fill="#35c9b4">
                  <circle cx="65.6" cy="29.3" r="4.5" /><circle cx="111.6" cy="75.6" r="4.5" />
                  <circle cx="265.2" cy="162.2" r="4.5" /><circle cx="309.5" cy="84.9" r="4.5" />
                  <circle cx="319.8" cy="41.6" r="4.5" />
                </g>
                <g font-family="Georgia,serif" font-style="italic" font-size="13" fill="#f4ecdc">
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

    <section id="validation" class="section">
      <div class="wrap">
        <p class="kicker">Validation</p>
        <h2>Every measurement is checked against Praat.</h2>
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

    <section class="section final">
      <div class="wrap" data-rv>
        <p class="kicker">Get started</p>
        <h2>Open a sound and start measuring.</h2>
        <p class="facts">Free and open-source <b>·</b> Browser and desktop <b>·</b> No account</p>
        <div class="final-actions">
          <a class="cta" href="#demo">Start analyzing</a>
          <span class="cta-note">Runs locally — nothing is uploaded.</span>
        </div>
      </div>
    </section>
  </main>

  <footer id="source">
    <div class="wrap foot">
      <div class="foot-brand">
        <svg width="26" height="26" viewBox="0 0 64 64" fill="none" aria-hidden="true">
          <path d="M46.5 12.9 A 22 22 0 1 0 52.2 20.4" stroke="#5eead4" stroke-width="7" stroke-linecap="round" />
          <path d="M14 36 C20 24 25 24 31 32 C37 40 41 40 50 24" stroke="#5eead4" stroke-width="7" stroke-linecap="round" />
          <circle cx="52" cy="20" r="5.5" fill="#f5b04c" stroke="none" />
        </svg>
        <span class="wordmark">Phonia</span>
      </div>
      <p>Free and open-source software, dual-licensed under MIT and Apache-2.0. Analysis runs locally — recordings are not uploaded. <a href={appHref} onclick={handleOpenPhonia}>Open Phonia</a></p>
    </div>
  </footer>
</div>

<style>
  .landing {
    --bg: #1d1d1a;
    --bg-deep: #141311;
    --panel: #191814;
    --panel-2: #201f1a;
    --line: rgba(244, 236, 220, 0.08);
    --line-soft: rgba(244, 236, 220, 0.05);
    --l-text: #f4ecdc;
    --l-muted: #b9b2a4;
    --l-faint: #7d786c;
    --l-teal: #35c9b4;
    --l-teal-hi: #6fe6d1;
    --l-amber: #d4953a;
    --l-danger: #ff5f57;
    --l-ink: #0b1614;
    --l-serif: 'Iowan Old Style', 'Palatino Linotype', Palatino, Charter, Georgia, 'Times New Roman', serif;
    --l-sans: system-ui, -apple-system, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
    --l-mono: ui-monospace, 'SF Mono', 'Cascadia Mono', Menlo, Consolas, monospace;
    --wrap: min(1120px, 92vw);

    position: relative;
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
    color: var(--l-ink);
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

  a.skip {
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

  /* ---------- header ---------- */
  .site-header {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    z-index: 50;
    background: rgba(29, 29, 26, 0.78);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--line);
  }

  .header-in {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 64px;
  }

  a.brand {
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

  a.nav-link {
    color: var(--l-muted);
    font-size: 0.95rem;
  }

  a.nav-link:hover {
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
      color 0.18s ease,
      box-shadow 0.18s ease;
  }

  .btn.btn-primary {
    background: linear-gradient(180deg, #4adcc5, #2ab39c);
    color: var(--l-ink);
    box-shadow: 0 6px 20px rgba(53, 201, 180, 0.28), inset 0 1px 0 rgba(255, 255, 255, 0.3);
  }

  .btn.btn-primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 10px 28px rgba(53, 201, 180, 0.4), inset 0 1px 0 rgba(255, 255, 255, 0.3);
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

  /* ---------- hero: 3D scene ---------- */
  .hero {
    position: relative;
    height: 100vh;
    min-height: 640px;
    background: radial-gradient(140% 100% at 78% 30%, #232220 0%, var(--bg) 55%, #171713 100%);
    perspective: 1400px;
    perspective-origin: 62% 42%;
    overflow: hidden;
  }

  @supports (height: 100svh) {
    .hero {
      height: 100svh;
    }
  }

  .scene {
    position: absolute;
    inset: -4vh -4vw;
    transform-style: preserve-3d;
    will-change: transform;
  }

  .glow {
    position: absolute;
    pointer-events: none;
    border-radius: 50%;
    mix-blend-mode: screen;
    filter: blur(46px);
  }

  .glow-a {
    width: 76vw;
    height: 76vw;
    right: -18vw;
    top: -14vh;
    background: radial-gradient(circle, rgba(53, 201, 180, 0.28) 0%, rgba(53, 201, 180, 0.1) 42%, transparent 68%);
    animation: seapulse 13s ease-in-out infinite alternate;
  }

  .glow-b {
    width: 50vw;
    height: 50vw;
    left: -13vw;
    bottom: -18vh;
    background: radial-gradient(circle, rgba(53, 201, 180, 0.19) 0%, rgba(53, 201, 180, 0.05) 45%, transparent 62%);
    filter: blur(50px);
    animation: seapulse 17s ease-in-out -6s infinite alternate;
  }

  .glow-c {
    width: 40vw;
    height: 40vw;
    left: 24vw;
    top: 36vh;
    background: radial-gradient(circle, rgba(111, 230, 209, 0.1) 0%, transparent 60%);
    filter: blur(42px);
    animation: seapulse 15s ease-in-out -9s infinite alternate;
  }

  @keyframes seapulse {
    from {
      opacity: 0.7;
      transform: scale(1);
    }
    to {
      opacity: 1;
      transform: scale(1.06);
    }
  }

  .panel {
    position: absolute;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow:
      0 40px 90px rgba(0, 0, 0, 0.6),
      0 4px 18px rgba(0, 0, 0, 0.5),
      inset 0 1px 0 rgba(244, 236, 220, 0.05);
    overflow: hidden;
    transform-style: preserve-3d;
  }

  .p-chrome {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 12px;
    background: linear-gradient(180deg, #1e1d1a, #191814);
    border-bottom: 1px solid rgba(244, 236, 220, 0.07);
    font-family: var(--l-mono);
    font-size: 10px;
    letter-spacing: 0.02em;
    color: #8f8a7d;
    white-space: nowrap;
  }

  .p-chrome i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #3a3833;
    flex: none;
  }

  .p-chrome i:first-child {
    background: #4a453c;
  }

  .p-chrome span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .p-chrome em {
    margin-left: auto;
    font-style: normal;
    color: #5f5b52;
  }

  .p-body {
    position: relative;
    line-height: 0;
  }

  .p-spec canvas {
    display: block;
    width: 100%;
    height: 100%;
  }

  .p-tiers {
    border-top: 1px solid var(--line);
    background: #151412;
  }

  .p-tier {
    position: relative;
    height: 22px;
    border-bottom: 1px solid rgba(244, 236,220, 0.05);
    display: flex;
    align-items: center;
  }

  .p-tier:last-child {
    border-bottom: 0;
  }

  .p-tier span {
    padding-left: 8px;
    font-family: var(--l-mono);
    font-size: 8px;
    letter-spacing: 0.12em;
    color: #57534a;
  }

  /* The panel that reads as "the editor": raked back in depth, not merely
     bled off the viewport edge — a genuine rotateY/rotateX rake plus the
     pointer parallax on .scene above is what sells the recession. */
  .panel-main {
    width: min(60vw, 1080px);
    right: -1.5vw;
    top: 14vh;
    transform: translateZ(-60px) rotateY(-8deg) rotateX(2.4deg);
    box-shadow:
      0 60px 130px rgba(0, 0, 0, 0.7),
      0 8px 30px rgba(0, 0, 0, 0.55),
      0 0 140px rgba(53, 201, 180, 0.1),
      inset 0 1px 0 rgba(244, 236, 220, 0.06);
  }

  .panel-main .p-spec {
    height: min(30vw, 420px);
  }

  .panel-side {
    width: min(30vw, 480px);
    left: -4vw;
    top: 62vh;
    transform: translateZ(-320px) rotateY(14deg) rotateX(4deg) rotateZ(1deg);
    filter: blur(3px) brightness(0.6) saturate(0.9);
    opacity: 0.55;
  }

  .panel-side .p-spec {
    height: min(15vw, 220px);
  }

  .chip {
    position: absolute;
    font-family: var(--l-mono);
    font-size: 10.5px;
    letter-spacing: 0.02em;
    color: #d8f4ec;
    background: rgba(22, 24, 22, 0.72);
    border: 1px solid rgba(111, 230, 209, 0.28);
    border-radius: 6px;
    padding: 6px 10px;
    backdrop-filter: blur(6px);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5), 0 0 24px rgba(53, 201, 180, 0.1);
    white-space: nowrap;
  }

  .chip b {
    color: #ff8a80;
    font-weight: 400;
  }

  .chip s {
    color: #9cc4ff;
    text-decoration: none;
  }

  .chip-a {
    right: 20vw;
    top: 30vh;
    transform: translateZ(90px);
  }

  .chip-b {
    right: 10vw;
    top: 56vh;
    transform: translateZ(70px);
  }

  .float-slow {
    animation: floaty 11s ease-in-out infinite;
  }

  .float-alt {
    animation: floaty 13s ease-in-out -5s infinite;
  }

  @keyframes floaty {
    0%,
    100% {
      margin-top: 0;
    }
    50% {
      margin-top: -9px;
    }
  }

  .scrim {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      linear-gradient(
        101deg,
        rgba(23, 22, 19, 0.96) 0%,
        rgba(23, 22, 19, 0.88) 24%,
        rgba(23, 22, 19, 0.55) 46%,
        rgba(23, 22, 19, 0.12) 66%,
        rgba(23, 22, 19, 0) 80%
      ),
      linear-gradient(0deg, rgba(20, 19, 16, 0.38) 0%, rgba(20, 19, 16, 0) 30%);
  }

  .vignette {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(125% 95% at 50% 42%, transparent 52%, rgba(11, 11, 9, 0.6) 100%);
  }

  .hero-content {
    position: relative;
    z-index: 5;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 60px 8vw 0;
    max-width: 100vw;
  }

  .eyebrow {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--l-mono);
    font-size: 11px;
    letter-spacing: 0.28em;
    text-transform: uppercase;
    color: var(--l-teal-hi);
    text-shadow: 0 1px 14px rgba(0, 0, 0, 0.7);
    margin: 0 0 20px;
  }

  .eyebrow::before {
    content: '';
    width: 26px;
    height: 1px;
    background: var(--l-teal);
    box-shadow: 0 0 8px rgba(53, 201, 180, 0.8);
  }

  .ipa {
    margin: 0 0 12px;
    line-height: 1;
  }

  .ipa-svg {
    display: block;
    height: clamp(2.6rem, 6vw, 4.6rem);
    width: auto;
    filter: drop-shadow(0 2px 24px rgba(0, 0, 0, 0.7));
  }

  .ipa-svg .ipa-t {
    fill: var(--l-text);
  }

  .ipa-svg .ipa-b {
    fill: var(--l-teal-hi);
  }

  h1 {
    font-family: var(--l-serif);
    font-weight: 500;
    font-size: clamp(2rem, 4.4vw, 3.7rem);
    line-height: 1.08;
    letter-spacing: -0.015em;
    color: var(--l-text);
    text-shadow: 0 2px 30px rgba(0, 0, 0, 0.65), 0 1px 4px rgba(0, 0, 0, 0.8);
    margin: 0 0 10px;
  }

  .roll-line {
    font-family: var(--l-serif);
    font-style: italic;
    font-size: clamp(1.05rem, 1.7vw, 1.4rem);
    line-height: 1.3;
    color: var(--l-muted);
    text-shadow: 0 1px 12px rgba(0, 0, 0, 0.8);
    margin: 0 0 20px;
  }

  .roll-line :global(.rolling) {
    color: var(--l-teal-hi);
    text-shadow: 0 0 26px rgba(53, 201, 180, 0.3), 0 1px 10px rgba(0, 0, 0, 0.7);
  }

  .lede {
    font-size: clamp(0.98rem, 1.3vw, 1.14rem);
    color: var(--l-muted);
    max-width: 50ch;
    text-shadow: 0 1px 12px rgba(0, 0, 0, 0.8);
    margin: 0 0 34px;
  }

  .cta-row {
    display: flex;
    align-items: center;
    gap: 22px;
    flex-wrap: wrap;
  }

  a.cta {
    display: inline-block;
    font-family: var(--l-sans);
    font-size: 0.95rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: var(--l-ink);
    background: linear-gradient(180deg, #4adcc5, #2ab39c);
    padding: 15px 30px;
    border-radius: 9px;
    box-shadow: 0 10px 34px rgba(53, 201, 180, 0.32), 0 2px 8px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.35);
    transition:
      transform 0.18s ease,
      box-shadow 0.18s ease;
  }

  .cta:hover {
    transform: translateY(-2px);
    box-shadow: 0 16px 44px rgba(53, 201, 180, 0.42), 0 2px 8px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.35);
  }

  .cta-note {
    font-family: var(--l-mono);
    font-size: 0.78rem;
    letter-spacing: 0.02em;
    color: var(--l-faint);
    text-shadow: 0 1px 10px rgba(0, 0, 0, 0.8);
  }

  a.scrollcue {
    position: absolute;
    left: 50%;
    bottom: 26px;
    transform: translateX(-50%);
    z-index: 6;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    color: var(--l-faint);
    font-family: var(--l-mono);
    font-size: 9.5px;
    letter-spacing: 0.34em;
    text-transform: uppercase;
    transition: color 0.18s ease;
  }

  .scrollcue:hover {
    color: var(--l-teal-hi);
  }

  .scrollcue svg {
    animation: hop 1.9s ease-in-out infinite;
  }

  @keyframes hop {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(7px);
    }
  }

  /* ---------- sections ---------- */
  .section {
    position: relative;
    padding: 6.5rem 6vw;
    scroll-margin-top: 4.5rem;
  }

  footer {
    scroll-margin-top: 4.5rem;
  }

  .kicker {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--l-mono);
    font-size: 10.5px;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    color: var(--l-teal);
    margin: 0 0 1.1rem;
  }

  .kicker::before {
    content: '';
    width: 22px;
    height: 1px;
    background: var(--l-teal);
    opacity: 0.7;
  }

  h2 {
    font-family: var(--l-serif);
    font-weight: 500;
    font-size: clamp(1.7rem, 3.2vw, 2.5rem);
    letter-spacing: -0.01em;
    line-height: 1.15;
    margin: 0 0 0.9rem;
  }

  .snote {
    color: var(--l-muted);
    max-width: 56ch;
    font-size: 0.98rem;
    margin: 0 0 3rem;
  }

  /* demo window */
  .demo {
    background:
      radial-gradient(70% 50% at 50% 0%, rgba(53, 201, 180, 0.06) 0%, transparent 70%),
      var(--bg-deep);
    border-top: 1px solid var(--line-soft);
    border-bottom: 1px solid var(--line-soft);
  }

  figure {
    margin: 0;
  }

  .shot-frame {
    border: 1px solid var(--line);
    border-radius: 16px;
    overflow: hidden;
    background: var(--bg-deep);
    box-shadow: 0 50px 110px rgba(0, 0, 0, 0.65), 0 6px 24px rgba(0, 0, 0, 0.5), 0 0 130px rgba(53, 201, 180, 0.09);
  }

  .shot-chrome {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    height: 34px;
    padding: 0 0.7rem;
    border-bottom: 1px solid var(--line-soft);
    background: var(--panel);
  }

  .shot-chrome.collapsed {
    border-bottom-color: transparent;
  }

  .shot-chrome .chrome-dot {
    appearance: none;
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    padding: 0;
    margin: 0;
    border: none;
    background: transparent;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    cursor: pointer;
    position: relative;
  }

  .shot-chrome .chrome-dot::before {
    content: '';
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #4a463c;
    transition:
      background 0.15s ease,
      transform 0.15s ease;
  }

  .shot-chrome .chrome-dot::after {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--l-mono);
    font-size: 10px;
    font-weight: 700;
    line-height: 1;
    color: var(--l-ink);
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .shot-chrome .dot-close::after {
    content: '×';
  }

  .shot-chrome .dot-min::after {
    content: '−';
  }

  .shot-chrome .dot-max::after {
    content: '+';
  }

  .shot-chrome .dot-close:hover::before,
  .shot-chrome .dot-close:focus-visible::before {
    background: var(--l-danger);
    transform: scale(1.15);
  }

  .shot-chrome .dot-min:hover::before,
  .shot-chrome .dot-min:focus-visible::before {
    background: var(--l-amber);
    transform: scale(1.15);
  }

  .shot-chrome .dot-max:hover::before,
  .shot-chrome .dot-max:focus-visible::before {
    background: var(--l-teal);
    transform: scale(1.15);
  }

  .shot-chrome .chrome-dot:hover::after,
  .shot-chrome .chrome-dot:focus-visible::after {
    opacity: 1;
  }

  .shot-title {
    margin-left: auto;
    margin-right: auto;
    font-family: var(--l-mono);
    font-size: 0.68rem;
    color: var(--l-faint);
    transform: translateX(-1.2rem);
    pointer-events: none;
  }

  .shot-title-btn {
    background: transparent;
    border: none;
    padding: 0.15rem 0.5rem;
    border-radius: 6px;
    cursor: pointer;
    pointer-events: auto;
    transition: color 0.15s ease;
  }

  .shot-title-btn:hover {
    color: var(--l-text);
  }

  .shot-collapse {
    display: grid;
    grid-template-rows: 1fr;
    transition: grid-template-rows 0.28s ease;
  }

  .shot-collapse.collapsed {
    grid-template-rows: 0fr;
  }

  .shot-collapse-inner {
    overflow: hidden;
    min-height: 0;
  }

  .shot-body {
    aspect-ratio: 16 / 10;
    position: relative;
  }

  .shot-closed {
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--bg-deep);
    padding: 2.6rem 1.5rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    text-align: center;
  }

  .shot-closed p {
    margin: 0;
    color: var(--l-muted);
    font-size: 0.95rem;
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

  .app-cta {
    margin-top: 1.4rem;
  }

  /* capability grid */
  .grid {
    display: grid;
    gap: 1.2rem;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  }

  .card {
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 12px;
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
    border-color: rgba(53, 201, 180, 0.35);
    background: var(--panel-2);
  }

  .card .idx {
    font-family: var(--l-mono);
    font-size: 0.7rem;
    letter-spacing: 0.2em;
    color: var(--l-teal);
    margin: 0;
  }

  .card h3 {
    font-size: 1.05rem;
    font-weight: 600;
    letter-spacing: -0.005em;
    margin: 0;
  }

  .card .cdesc {
    font-size: 0.9rem;
    color: var(--l-muted);
    line-height: 1.55;
    margin: 0;
  }

  .viz {
    margin-top: auto;
    height: 150px;
    border-radius: 10px;
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
    background: rgba(53, 201, 180, 0.55);
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

  /* final cta */
  .final {
    text-align: center;
    padding: 8rem 6vw 7rem;
    background: radial-gradient(58% 74% at 50% 64%, rgba(53, 201, 180, 0.1) 0%, rgba(53, 201, 180, 0.03) 46%, transparent 72%);
  }

  .final .kicker {
    justify-content: center;
  }

  .final h2 {
    margin: 0 auto 1.1rem;
    max-width: 22ch;
    font-size: clamp(2rem, 4.2vw, 3rem);
  }

  .final .facts {
    font-family: var(--l-mono);
    font-size: 11px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--l-faint);
    margin: 0 0 2.6rem;
  }

  .final .facts b {
    color: var(--l-teal);
    font-weight: 400;
    padding: 0 6px;
  }

  .final-actions {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 22px;
    flex-wrap: wrap;
  }

  footer {
    position: relative;
    z-index: 1;
    border-top: 1px solid var(--line-soft);
    padding: 2.4rem 6vw 2.6rem;
  }

  .foot {
    max-width: var(--wrap);
    margin: 0 auto;
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
    flex: none;
  }

  .foot-brand .wordmark {
    font-size: 1.1rem;
  }

  .foot p {
    font-family: var(--l-mono);
    font-size: 0.75rem;
    color: var(--l-faint);
    margin: 0;
    max-width: 56ch;
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

    .scene {
      transform: none !important;
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

    .panel-side,
    .chip {
      display: none;
    }

    .panel-main {
      width: 150vw;
      right: -40vw;
      top: 8vh;
      opacity: 0.4;
      transform: none;
      filter: blur(1px) brightness(0.8);
    }

    .hero-content {
      padding: 5rem 7vw 0;
    }
  }
</style>
