<script lang="ts">
  import Logo from '$assets/images/logo.svg.svelte';
</script>

<header class="flex items-center justify-between h-4 p-4 pt-6 gap-2 select-none">
  <Logo class="text-gray-900 dark:text-gray-300 h-4" />
  <nav class="flex items-center h-4 flex-1 gap-1">
    {#snippet tab(name: string, active: boolean)}
      <!-- Fin -->
      <div
        class={[
          // Layout
          'px-4 py-1',
          'rounded-t-lg border-b-2',
          'flex items-center gap-4',
          // Coloring
          'text-gray-900 dark:text-gray-200 dark:bg-gray-900 bg-gray-200',
          'border-gray-900 dark:border-gray-200',
          {
            'border-blue-600 dark:border-blue-600': active,
          },
        ]}
      >
        {name}
        <button>✕</button>
      </div>
    {/snippet}
    {@render tab('Untitled Project 1', true)}
    {@render tab('Untitled Project 2', false)}
    <button
      class="text-gray-900 dark:text-gray-200 dark:bg-gray-900 bg-gray-200 px-3 py-1 rounded-t-md border-b-2 border-gray-900 dark:border-gray-200"
    >
      ＋
    </button>
  </nav>
</header>
