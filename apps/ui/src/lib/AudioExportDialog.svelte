<script lang="ts">
  import { untrack } from 'svelte';
  import IconAudioLines from '~icons/lucide/audio-lines';
  import IconX from '~icons/lucide/x';
  import IconDownload from '~icons/lucide/download';
  import type { AudioExportOptions, WavBitDepth } from './types';

  interface Props {
    /** Whether a selection is active, enabling the selection scope. */
    hasSelection: boolean;
    /** Whether the active selection is a frequency box, enabling filtered export. */
    isBoxSelection: boolean;
    busy?: boolean;
    onExport: (options: AudioExportOptions) => void;
    onClose: () => void;
  }

  let { hasSelection, isBoxSelection, busy = false, onExport, onClose }: Props = $props();

  let scope = $state<'whole' | 'selection'>(untrack(() => hasSelection) ? 'selection' : 'whole');
  let bits = $state<WavBitDepth>('Pcm16');
  let filtered = $state(false);

  const DEPTHS: Array<{ value: WavBitDepth; label: string }> = [
    { value: 'Pcm16', label: '16-bit PCM' },
    { value: 'Pcm24', label: '24-bit PCM' },
    { value: 'Pcm32', label: '32-bit PCM' },
    { value: 'Float32', label: '32-bit float' }
  ];

  const canFilter = $derived(scope === 'selection' && isBoxSelection);

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop" data-testid="audio-export">
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Export audio">
    <header class="head">
      <h2><IconAudioLines aria-hidden="true" />Export audio</h2>
      <button type="button" class="close" data-testid="audio-export-close" aria-label="Close" onclick={onClose}>
        <IconX aria-hidden="true" />
      </button>
    </header>

    <div class="body">
      <fieldset>
        <legend>Range</legend>
        <label class="radio">
          <input type="radio" name="audio-scope" value="whole" checked={scope === 'whole'} onchange={() => (scope = 'whole')} />
          Whole recording
        </label>
        <label class="radio" class:disabled={!hasSelection}>
          <input
            type="radio"
            name="audio-scope"
            value="selection"
            data-testid="audio-scope-selection"
            disabled={!hasSelection}
            checked={scope === 'selection'}
            onchange={() => (scope = 'selection')}
          />
          Current selection
        </label>
        {#if canFilter}
          <label class="check" data-testid="audio-filtered-row">
            <input
              type="checkbox"
              data-testid="audio-filtered"
              checked={filtered}
              onchange={(event) => (filtered = event.currentTarget.checked)}
            />
            Band-limit to the selection box
          </label>
        {/if}
      </fieldset>

      <fieldset>
        <legend>Bit depth</legend>
        <select
          data-testid="audio-bits"
          value={bits}
          onchange={(event) => (bits = event.currentTarget.value as WavBitDepth)}
        >
          {#each DEPTHS as depth (depth.value)}
            <option value={depth.value}>{depth.label}</option>
          {/each}
        </select>
      </fieldset>

      <button
        type="button"
        class="download"
        data-testid="audio-export-download"
        disabled={busy}
        onclick={() => onExport({ scope, bits, filtered: canFilter && filtered })}
      >
        <IconDownload aria-hidden="true" /><span>{busy ? 'Exporting…' : 'Download WAV'}</span>
      </button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, #000 52%, transparent);
    backdrop-filter: blur(2px);
    z-index: 25;
  }

  .dialog {
    width: min(24rem, calc(100vw - 2rem));
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.6rem 0.9rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
  }

  .head h2 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.95rem;
    font-weight: 600;
  }

  .head h2 :global(svg) {
    font-size: 1rem;
    color: var(--accent);
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    padding: 0.2rem;
    cursor: pointer;
  }

  .close:hover {
    background: var(--panel);
    color: var(--text);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    padding: 0.85rem 0.9rem 0.95rem;
  }

  fieldset {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    padding: 0.5rem 0.7rem 0.6rem;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  legend {
    padding: 0 0.35rem;
    font-size: 0.72rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .radio,
  .check {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.85rem;
    padding: 0.1rem 0;
  }

  .radio input,
  .check input {
    accent-color: var(--accent);
  }

  .radio.disabled {
    color: var(--muted);
  }

  select {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    min-height: 2rem;
    padding: 0.25rem 0.4rem;
    font-size: 0.85rem;
    width: 100%;
  }

  .download {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    border: 1px solid var(--action);
    border-radius: var(--radius-md);
    background: var(--action);
    color: #fff;
    padding: 0.5rem 0.6rem;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .download :global(svg) {
    font-size: 1rem;
  }

  .download:hover:not(:disabled) {
    background: var(--action-strong);
    border-color: var(--action-strong);
  }

  .download:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
