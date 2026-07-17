<script lang="ts">
  import IconChevronDown from '~icons/lucide/chevron-down';
  import IconPencil from '~icons/lucide/pencil';
  import IconPlus from '~icons/lucide/plus';
  import {
    BUILTIN_PALETTES,
    builtinGradientCss,
    paletteGradientCss,
    paletteKey,
    paletteLabel,
    rampGradientCss,
    type CustomRamp,
    type PaletteSelection
  } from './palette';

  interface Props {
    palette: PaletteSelection;
    customRamps: CustomRamp[];
    onSelect: (palette: PaletteSelection) => void;
    onNewRamp: () => void;
    onEditRamp: (ramp: CustomRamp) => void;
  }

  let { palette, customRamps, onSelect, onNewRamp, onEditRamp }: Props = $props();

  type Row =
    | { kind: 'builtin'; sel: PaletteSelection; label: string; gradient: string; note?: string }
    | { kind: 'custom'; sel: PaletteSelection; ramp: CustomRamp; gradient: string }
    | { kind: 'new' };

  const rows = $derived.by<Row[]>(() => {
    const out: Row[] = BUILTIN_PALETTES.map((p) => ({
      kind: 'builtin' as const,
      sel: { kind: 'builtin' as const, name: p.name },
      label: p.label,
      gradient: builtinGradientCss(p.preview),
      note: p.note
    }));
    for (const ramp of customRamps) {
      out.push({
        kind: 'custom',
        sel: { kind: 'custom', ramp },
        ramp,
        gradient: rampGradientCss(ramp.stops)
      });
    }
    out.push({ kind: 'new' });
    return out;
  });

  // Only the selectable rows take arrow focus; the New row is a button of its own.
  const selectable = $derived(rows.filter((r): r is Exclude<Row, { kind: 'new' }> => r.kind !== 'new'));

  let open = $state(false);
  let activeIndex = $state(0);
  let rootEl = $state<HTMLDivElement | null>(null);
  let triggerEl = $state<HTMLButtonElement | null>(null);
  let listEl = $state<HTMLUListElement | null>(null);

  const currentKey = $derived(paletteKey(palette));
  const optionId = (i: number) => `palette-opt-${i}`;

  function show() {
    if (open) return;
    const idx = selectable.findIndex((r) => paletteKey(r.sel) === currentKey);
    activeIndex = idx >= 0 ? idx : 0;
    open = true;
    queueMicrotask(() => listEl?.focus());
  }

  function hide(returnFocus = true) {
    if (!open) return;
    open = false;
    if (returnFocus) queueMicrotask(() => triggerEl?.focus());
  }

  function toggle() {
    if (open) hide();
    else show();
  }

  function choose(sel: PaletteSelection) {
    hide();
    onSelect(sel);
  }

  function onListKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      hide();
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      activeIndex = (activeIndex + 1) % selectable.length;
      return;
    }
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      activeIndex = (activeIndex - 1 + selectable.length) % selectable.length;
      return;
    }
    if (event.key === 'Home') {
      event.preventDefault();
      activeIndex = 0;
      return;
    }
    if (event.key === 'End') {
      event.preventDefault();
      activeIndex = selectable.length - 1;
      return;
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      const row = selectable[activeIndex];
      if (row) choose(row.sel);
    }
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (!open) return;
    if (rootEl && event.target instanceof Node && !rootEl.contains(event.target)) hide(false);
  }

  $effect(() => {
    if (!open) return;
    const active = selectable[activeIndex];
    if (!active) return;
    queueMicrotask(() => {
      listEl
        ?.querySelector<HTMLElement>(`#${CSS.escape(optionId(activeIndex))}`)
        ?.scrollIntoView({ block: 'nearest' });
    });
  });
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="picker" bind:this={rootEl}>
  <button
    type="button"
    class="trigger"
    bind:this={triggerEl}
    data-testid="palette-picker"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label="Spectrogram palette"
    onclick={toggle}
  >
    <span class="strip" style="background: {paletteGradientCss(palette)}"></span>
    <span class="trigger-name" data-testid="palette-current">{paletteLabel(palette)}</span>
    <span class="chev" class:open aria-hidden="true"><IconChevronDown /></span>
  </button>

  {#if open}
    <div class="popover" data-testid="palette-popover">
      <ul
        bind:this={listEl}
        class="list"
        role="listbox"
        tabindex="0"
        aria-label="Spectrogram palette"
        aria-activedescendant={optionId(activeIndex)}
        onkeydown={onListKeydown}
      >
        {#each rows as row, i (row.kind === 'new' ? 'new' : paletteKey(row.sel))}
          {#if row.kind === 'new'}
            <li class="sep" role="presentation"></li>
            <li role="presentation">
              <button
                type="button"
                class="new-row"
                data-testid="palette-new"
                onclick={() => {
                  hide(false);
                  onNewRamp();
                }}
              >
                <span class="new-icon"><IconPlus aria-hidden="true" /></span>
                New custom ramp…
              </button>
            </li>
          {:else}
            {@const selIndex = selectable.indexOf(row)}
            {@const active = selIndex === activeIndex}
            {@const current = paletteKey(row.sel) === currentKey}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <li
              id={optionId(selIndex)}
              class="option"
              class:active
              class:current
              role="option"
              aria-selected={current}
              data-testid="palette-option"
              data-name={row.kind === 'builtin' ? row.label : row.ramp.name}
              data-kind={row.kind}
              onpointermove={() => (activeIndex = selIndex)}
              onclick={(e) => {
                if (e.target instanceof Element && e.target.closest('.edit')) return;
                choose(row.sel);
              }}
            >
              <span class="strip" style="background: {row.gradient}"></span>
              <span class="opt-name">{row.kind === 'builtin' ? row.label : row.ramp.name}</span>
              {#if row.kind === 'builtin' && row.note}
                <span class="note">{row.note}</span>
              {/if}
              {#if row.kind === 'custom'}
                <button
                  type="button"
                  class="edit"
                  data-testid="palette-edit"
                  aria-label={`Edit ${row.ramp.name}`}
                  onclick={() => {
                    hide(false);
                    onEditRamp(row.ramp);
                  }}
                >
                  <IconPencil aria-hidden="true" />
                </button>
              {/if}
            </li>
          {/if}
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    display: inline-flex;
    min-width: 0;
  }

  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    min-height: 2.1rem;
    padding: 0.2rem 0.5rem 0.2rem 0.35rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    font: inherit;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .trigger:hover,
  .trigger[aria-expanded='true'] {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .trigger:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .strip {
    flex: none;
    width: 2.4rem;
    height: 1rem;
    border-radius: 3px;
    border: 1px solid color-mix(in oklab, var(--text) 18%, transparent);
  }

  .trigger-name {
    min-width: 0;
    max-width: 8rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.85rem;
    font-weight: 500;
  }

  .chev {
    display: inline-flex;
    flex: none;
    color: var(--muted);
    transition: transform var(--t-fast);
  }

  .chev :global(svg) {
    font-size: 0.9rem;
  }

  .chev.open {
    transform: rotate(180deg);
  }

  .popover {
    position: absolute;
    top: calc(100% + 0.35rem);
    right: 0;
    z-index: 30;
    width: min(20rem, calc(100vw - 2rem));
    max-height: min(60vh, 24rem);
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  .list {
    margin: 0;
    padding: 0.3rem;
    list-style: none;
    overflow-y: auto;
    max-height: inherit;
    outline: none;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.35rem 0.45rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .option .strip {
    width: 3.4rem;
    height: 1.2rem;
  }

  .option.active {
    background: var(--accent-tint);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  .option.current .opt-name {
    font-weight: 600;
  }

  .opt-name {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.85rem;
  }

  .note {
    flex: none;
    font-size: 0.68rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .edit {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 1.6rem;
    height: 1.6rem;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    cursor: pointer;
  }

  .edit:hover {
    background: var(--panel-soft);
    color: var(--accent-strong);
  }

  .sep {
    height: 1px;
    margin: 0.3rem 0.2rem;
    background: var(--chrome-strong);
  }

  .new-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.4rem 0.45rem;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--accent-strong);
    font: inherit;
    font-size: 0.85rem;
    cursor: pointer;
    text-align: left;
  }

  .new-row:hover {
    background: var(--accent-tint);
  }

  .new-icon {
    display: inline-grid;
    place-items: center;
  }
</style>
