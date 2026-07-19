<script lang="ts">
  import IconLibrary from '~icons/lucide/library';
  import IconActivity from '~icons/lucide/activity';

  interface Props {
    /** Which mode is current; drives aria-current and the active visual state. */
    active: 'library' | 'analyze';
    /** False disables the Analyze button (no recording is open to analyze). */
    analyzeEnabled: boolean;
    onNavigate: (mode: 'library' | 'analyze') => void;
  }

  let { active, analyzeEnabled, onNavigate }: Props = $props();

  const MODES: { id: 'library' | 'analyze'; label: string }[] = [
    { id: 'library', label: 'Library' },
    { id: 'analyze', label: 'Analyze' }
    // Studio, Plot, and Script are planned modes; add each as one more entry
    // here plus one icon import below — never a disabled placeholder button.
  ];

  const ICONS = { library: IconLibrary, analyze: IconActivity };
</script>

<nav class="rail" aria-label="Modes">
  <div class="brand" aria-hidden="true">
    <svg
      class="mark"
      aria-hidden="true"
      viewBox="0 0 64 64"
      fill="none"
      stroke-width="6"
      stroke-linecap="round"
    >
      <path stroke="currentColor" d="M46.5 12.9 A 22 22 0 1 0 52.2 20.4" />
      <path stroke="currentColor" d="M14 36 C20 24 25 24 31 32 C37 40 41 40 50 24" />
      <circle class="dot" cx="52" cy="20" r="5" />
    </svg>
  </div>

  <div class="modes">
    {#each MODES as mode (mode.id)}
      {@const Icon = ICONS[mode.id]}
      <button
        type="button"
        class="mode"
        class:active={active === mode.id}
        aria-current={active === mode.id ? 'page' : undefined}
        disabled={mode.id === 'analyze' && !analyzeEnabled}
        title={mode.id === 'analyze' && !analyzeEnabled ? 'Analyze — open a recording first' : mode.label}
        onclick={() => onNavigate(mode.id)}
      >
        <Icon aria-hidden="true" />
        <span>{mode.label}</span>
      </button>
    {/each}
  </div>
</nav>

<style>
  .rail {
    position: fixed;
    inset: 0 auto 0 0;
    /* Keep this width in sync with the host app-content offset. */
    width: 4.75rem;
    z-index: 10;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border-right: 1px solid var(--chrome-strong);
  }

  .brand {
    height: 4rem;
    display: grid;
    place-items: center;
  }

  .mark {
    width: 1.6rem;
    height: 1.6rem;
    color: var(--accent);
  }

  .mark .dot {
    fill: var(--warn);
  }

  .modes {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .mode {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.35rem;
    padding: 0.65rem 0.25rem;
    border: none;
    border-left: 2px solid transparent;
    background: transparent;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast),
      color var(--t-fast),
      border-color var(--t-fast);
  }

  .mode :global(svg) {
    width: 1.15rem;
    height: 1.15rem;
  }

  .mode span {
    font-size: 0.62rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .mode:hover:not(:disabled):not(.active) {
    color: var(--text);
    background: var(--panel-soft);
  }

  .mode.active {
    color: var(--accent);
    border-left-color: var(--accent);
    background: var(--accent-tint);
  }

  .mode:disabled {
    color: color-mix(in oklab, var(--muted), transparent 45%);
    cursor: default;
  }
</style>
