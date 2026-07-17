<script lang="ts">
  import IconPackage from '~icons/lucide/package';
  import IconX from '~icons/lucide/x';
  import IconDownload from '~icons/lucide/download';
  import type { ProjectExportMode } from './types';

  interface Props {
    /** Recordings in the project, so the dialog can state what a bundle carries. */
    recordingCount: number;
    busy?: boolean;
    onExport: (mode: ProjectExportMode) => void;
    onClose: () => void;
  }

  let { recordingCount, busy = false, onExport, onClose }: Props = $props();

  let mode = $state<ProjectExportMode>('bundle');

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop" data-testid="project-export">
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Export project">
    <header class="head">
      <h2><IconPackage aria-hidden="true" />Export project</h2>
      <button type="button" class="close" data-testid="project-export-close" aria-label="Close" onclick={onClose}>
        <IconX aria-hidden="true" />
      </button>
    </header>

    <div class="body">
      <fieldset>
        <legend>Contents</legend>
        <label class="option" class:active={mode === 'bundle'}>
          <input
            type="radio"
            name="export-mode"
            value="bundle"
            data-testid="export-mode-bundle"
            checked={mode === 'bundle'}
            onchange={() => (mode = 'bundle')}
          />
          <span class="option-body">
            <span class="option-title">Self-contained bundle</span>
            <span class="option-sub">
              Embeds all {recordingCount}
              {recordingCount === 1 ? 'recording' : 'recordings'}. Opens on any machine.
            </span>
          </span>
        </label>
        <label class="option" class:active={mode === 'references'}>
          <input
            type="radio"
            name="export-mode"
            value="references"
            data-testid="export-mode-references"
            checked={mode === 'references'}
            onchange={() => (mode = 'references')}
          />
          <span class="option-body">
            <span class="option-title">References only</span>
            <span class="option-sub">
              Manifest and annotations, no audio. Re-links recordings by content hash on import.
            </span>
          </span>
        </label>
      </fieldset>

      <button
        type="button"
        class="download"
        data-testid="project-export-download"
        disabled={busy}
        onclick={() => onExport(mode)}
      >
        <IconDownload aria-hidden="true" /><span>{busy ? 'Exporting…' : 'Download'}</span>
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
    width: min(28rem, calc(100vw - 2rem));
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
    border: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  legend {
    padding: 0;
    font-size: 0.72rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: 0.3rem;
  }

  .option {
    display: flex;
    align-items: flex-start;
    gap: 0.55rem;
    padding: 0.6rem 0.7rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    cursor: pointer;
    transition:
      border-color var(--t-fast),
      background var(--t-fast);
  }

  .option.active {
    border-color: color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    background: var(--accent-tint);
  }

  .option input {
    margin-top: 0.15rem;
    accent-color: var(--accent);
  }

  .option-body {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }

  .option-title {
    font-size: 0.9rem;
    font-weight: 600;
  }

  .option-sub {
    font-size: 0.78rem;
    color: var(--muted);
    line-height: 1.35;
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
