<script lang="ts">
  interface Props {
    value: string;
    onInput: (value: string) => void;
    onCommit: () => void;
    onCancel: () => void;
  }

  let { value, onInput, onCommit, onCancel }: Props = $props();

  let input = $state<HTMLInputElement | null>(null);
  // Composition guards the Enter key so an IME candidate confirmation does not
  // commit the label; IPA entered through a direct keyboard layout is untouched.
  let composing = $state(false);

  $effect(() => {
    if (!input) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  });

  function handleKeydown(event: KeyboardEvent) {
    event.stopPropagation();
    if (composing) return;
    if (event.key === 'Enter') {
      event.preventDefault();
      onCommit();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      onCancel();
    }
  }
</script>

<input
  bind:this={input}
  class="label-editor"
  data-testid="label-editor"
  type="text"
  autocomplete="off"
  autocapitalize="off"
  autocorrect="off"
  spellcheck="false"
  {value}
  oninput={(event) => onInput(event.currentTarget.value)}
  onkeydown={handleKeydown}
  oncompositionstart={() => (composing = true)}
  oncompositionend={() => (composing = false)}
  onblur={onCommit}
/>

<style>
  .label-editor {
    position: absolute;
    inset: 2px;
    width: calc(100% - 4px);
    min-width: 0;
    border: 1px solid var(--accent);
    border-radius: 3px;
    background: var(--panel);
    color: var(--text);
    padding: 0 0.25rem;
    font-size: 0.82rem;
    text-align: center;
    outline: none;
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent), transparent 65%);
  }
</style>
