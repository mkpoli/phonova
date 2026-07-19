<script lang="ts">
  import IconKeyboard from '~icons/lucide/keyboard';
  import IconX from '~icons/lucide/x';
  import { KEY_MODES, type KeyModeId } from './keybindings.svelte';

  interface Props {
    onChoose: (mode: KeyModeId) => void;
    onDismiss: () => void;
  }

  let { onChoose, onDismiss }: Props = $props();

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') onDismiss();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<!-- Non-modal by design: this never covers the rest of the app, so a person
     who wants to start working can simply ignore it. See DESIGN.md
     "Non-modal by default" — a key-mode preference is not a data-loss
     decision, so it never earns a blocking backdrop. -->
<div class="card" role="dialog" aria-label="Keyboard mode" data-testid="key-mode-prompt">
  <div class="head">
    <IconKeyboard aria-hidden="true" />
    <span class="title">Keyboard mode</span>
    <button type="button" class="close" data-testid="key-mode-prompt-close" aria-label="Decide later" onclick={onDismiss}>
      <IconX aria-hidden="true" />
    </button>
  </div>
  <p class="intro">Two key bindings ship with Phonia. Pick one now, or decide later from settings.</p>
  <div class="options">
    {#each KEY_MODES as mode (mode.id)}
      <button
        type="button"
        class="option"
        data-testid={`key-mode-choice-${mode.id}`}
        title={mode.description}
        onclick={() => onChoose(mode.id)}
      >
        {mode.label}
      </button>
    {/each}
  </div>
  <button type="button" class="later" data-testid="key-mode-prompt-later" onclick={onDismiss}>Decide later</button>
</div>

<style>
  .card {
    position: fixed;
    left: 1.25rem;
    bottom: 1.25rem;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    width: min(21rem, calc(100vw - 2.5rem));
    padding: 0.75rem 0.85rem;
    border-radius: var(--radius-lg);
    border: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .head :global(svg:first-child) {
    color: var(--accent);
    font-size: 1rem;
    flex: none;
  }

  .title {
    flex: 1;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--muted);
    padding: 0.15rem;
    cursor: pointer;
  }

  .close:hover {
    color: var(--text);
  }

  .intro {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
    line-height: 1.4;
  }

  .options {
    display: flex;
    gap: 0.4rem;
  }

  .option {
    flex: 1;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.4rem 0.5rem;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .option:hover {
    background: var(--accent-tint);
    border-color: color-mix(in oklab, var(--accent) 40%, var(--chrome-strong));
    color: var(--accent-strong);
  }

  .later {
    align-self: center;
    border: none;
    background: transparent;
    color: var(--muted);
    font-size: 0.76rem;
    padding: 0.1rem 0.3rem;
    cursor: pointer;
  }

  .later:hover {
    color: var(--text);
    text-decoration: underline;
  }
</style>
