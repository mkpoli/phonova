<script lang="ts">
  // https://zenn.dev/scirexs/articles/fe6453fcddf452

  import { type Snippet, untrack } from 'svelte';

  function stop<T extends Element>(func?: (this: T, evt: Event) => void) {
    return function (this: T, evt: Event) {
      if (func !== undefined) {
        evt.stopPropagation();
        func.call(this, evt);
      }
    };
  }

  interface Props {
    open: boolean;
    closable?: boolean;
    children: Snippet;
    element?: HTMLDialogElement;
    class?: string;
  }
  let {
    open = $bindable(),
    closable = true,
    children,
    element = $bindable(),
    class: className = $bindable(),
  }: Props = $props();
  $effect.pre(() => {
    open;
    untrack(() => {
      toggleDialog();
    });
  });

  function toggleDialog() {
    if (element === undefined) {
      return;
    }
    if (open) {
      element.showModal();
    } else {
      element.close();
    }
  }

  const closeDialog = closable
    ? () => {
        open = false;
      }
    : undefined;
  const preventClose = closable ? stop(() => {}) : undefined;
  const preventEsc = !closable
    ? (ev: KeyboardEvent) => {
        if (ev.key === 'Escape') {
          ev.preventDefault();
        }
      }
    : undefined;
  const oncancel = closable
    ? () => {
        open = false;
      }
    : undefined;
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={element}
  onclick={closeDialog}
  onkeydown={preventEsc}
  {oncancel}
  class={[
    'backdrop:bg-gray-900 backdrop:bg-opacity-50 backdrop:backdrop-blur-lg dark:bg-slate-900 bg-slate-200 text-gray-900 dark:text-gray-200',
    className,
  ]}
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div onclick={preventClose}>
    {@render children()}
  </div>
</dialog>

<style>
  :global(html:has(dialog[open])) {
    overflow: hidden;
  }
</style>
