<script lang="ts">
  // The v20 landing (architect-built static design), ported wholesale.
  // Markup/CSS/JS live as strings so the design iterates in the gallery and
  // ships here unchanged; Svelte owns mounting, cleanup, and app entry.
  import { onMount } from 'svelte';
  import { LANDING_CSS, LANDING_HTML, LANDING_JS } from './landing-v20-assets';

  interface Props {
    /** Swap straight to the app when this page is shown inline at the app
     * root (first visit) - no reload. Omitted on /landing and the
     * marketing subdomain, where a real navigation is correct. */
    onEnterApp?: () => void;
  }
  let { onEnterApp }: Props = $props();

  let root: HTMLDivElement;

  function appOrigin(): string {
    const h = location.hostname;
    if (h === 'phonia.app') return '';
    if (h === 'about.phonia.app') return 'https://phonia.app';
    return '';
  }

  onMount(() => {
    const w = window as any;
    w.LP_APP_URL = () => appOrigin() + '/?app=1&sample=1';
    w.LP_ENTER = () => {
      if (onEnterApp) onEnterApp();
      else location.assign(w.LP_APP_URL());
    };
    const script = document.createElement('script');
    script.textContent = LANDING_JS;
    root.appendChild(script);
    return () => {
      delete w.LP_APP_URL;
      delete w.LP_ENTER;
    };
  });
</script>

<svelte:head>
  {@html '<sty' + 'le>' + LANDING_CSS + '</sty' + 'le>'}
</svelte:head>

<div class="lp-root dark" bind:this={root}>
  {@html LANDING_HTML}
</div>
