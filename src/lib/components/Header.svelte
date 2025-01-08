<script lang="ts">
  import Logo from '$assets/images/logo.svg.svelte';
  import ThemeSwitch from '$lib/components/ui/ThemeSwitch.svelte';
  import About from '$lib/components/dialogs/About.svelte';
  import { projectsManager } from '$lib/project.svelte';
  let open = $state(false);
</script>

<header class="flex items-center justify-between h-4 p-4 pt-6 gap-2 select-none">
  <button onclick={() => (open = true)}>
    <Logo class="text-gray-900 dark:text-gray-300 h-4" />
  </button>
  <About bind:open />
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
          'text-gray-900 dark:text-gray-200 dark:bg-gray-950 bg-gray-200',
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
    {#each projectsManager.projects as project}
      {@render tab(project.name, project.id == projectsManager.currentProject)}
    {/each}
    <button
      class="text-gray-900 dark:text-gray-200 dark:bg-gray-950 bg-gray-200 px-3 py-1 rounded-t-md border-b-2 border-gray-900 dark:border-gray-200"
      onclick={() => projectsManager.createProject(null)}
    >
      ＋
    </button>
  </nav>
  <ThemeSwitch />
</header>
