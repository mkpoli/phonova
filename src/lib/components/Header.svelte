<script lang="ts">
  import Logo from '$assets/images/logo.svg.svelte';
  import ThemeSwitch from '$lib/components/ui/ThemeSwitch.svelte';
  import About from '$lib/components/dialogs/About.svelte';
  import { projectsManager, type Project } from '$lib/project.svelte';
  let open = $state(false);
  let renameProjectName: Record<string, string> = $state({});
  let renamingMap: Record<string, boolean> = $state({});
</script>

<header class="grid grid-cols-[auto_1fr_auto] items-center w-full h-fit py-2 px-4 gap-2 select-none">
  <button onclick={() => (open = true)}>
    <Logo class="text-gray-900 dark:text-gray-300 h-4" />
  </button>
  <About bind:open />
  <nav class="flex items-center h-fit flex-1 gap-1 overflow-scroll w-full relative scrollbar-none">
    {#snippet tab(project: Project)}
      {@const active = project.id == projectsManager.currentProject}
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
          'break-keep w-fit whitespace-nowrap',
          {
            'border-blue-600 dark:border-blue-600': active,
          },
        ]}
      >
        {#if renamingMap[project.id]}
          <form onsubmit={() => projectsManager.renameProject(project.id, renameProjectName[project.id])}>
            <input
              class="bg-black/50 p-0"
              bind:value={renameProjectName[project.id]}
              onblur={() => {
                renamingMap[project.id] = false;
                projectsManager.renameProject(project.id, renameProjectName[project.id]);
              }}
            />
          </form>
        {:else}
          <button
            ondblclick={() => {
              renamingMap[project.id] = true;
              renameProjectName[project.id] = project.name;
            }}
          >
            {project.name}
          </button>
        {/if}
        <button
          class="text-gray-900 dark:text-gray-200"
          onclick={() => {
            if (!project.dirty) {
              projectsManager.deleteProject(project.id);
            } else {
              alert('Project is dirty, please save before deleting');
            }
          }}
        >
          ✕
        </button>
      </div>
    {/snippet}
    {#each projectsManager.projects as project}
      {@render tab(project)}
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
