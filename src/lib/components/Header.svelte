<script lang="ts">
  import Logo from '$assets/images/logo.svg.svelte';
  import ThemeSwitch from '$lib/components/ui/ThemeSwitch.svelte';
  import About from '$lib/components/dialogs/About.svelte';
  import { projectsManager, type Project } from '$lib/project.svelte';
  import MaterialSymbolsArrowLeft from '~icons/material-symbols/arrow-left';
  import MaterialSymbolsArrowRight from '~icons/material-symbols/arrow-right';
  let open = $state(false);
  let renameProjectName: Record<string, string> = $state({});
  let renamingMap: Record<string, boolean> = $state({});
  let nav: HTMLElement | null = $state(null);
  class ReactiveNavScrollable {
    scrollable: boolean = $state(false);
    scrolled: boolean = $state(false);
    constructor(nav: HTMLElement) {
      this.scrollable = nav.scrollWidth > nav.clientWidth;

      const resizeObserver = new ResizeObserver(() => {
        this.scrollable = nav.scrollWidth > nav.clientWidth;
        this.scrolled = nav.scrollLeft > 0;
      });

      resizeObserver.observe(nav);

      for (const child of nav.children) {
        resizeObserver.observe(child as HTMLElement);
      }

      const mutationObserver = new MutationObserver(() => {
        this.scrollable = nav.scrollWidth > nav.clientWidth;
        this.scrolled = nav.scrollLeft > 0;
      });

      mutationObserver.observe(nav, { childList: true });

      nav.onscroll = () => {
        this.scrolled = nav.scrollLeft > 0;
      };
    }
  }

  let navScrollable = $derived(nav ? new ReactiveNavScrollable(nav) : null);
</script>

<header class="grid grid-cols-[auto_1fr_auto] items-center w-full h-fit py-2 px-4 gap-2 select-none">
  <button onclick={() => (open = true)}>
    <Logo class="text-gray-900 dark:text-gray-300 h-4" />
  </button>
  <About bind:open />
  <nav class="flex items-center h-fit flex-1 gap-1 overflow-scroll w-full relative scrollbar-none" bind:this={nav}>
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
    {#if navScrollable?.scrolled}
      <div class="sticky left-0 h-full flex items-center justify-center">
        <button
          class="rounded-full dark:bg-gray-900/50 bg-gray-200 dark:text-gray-200 text-gray-900 p-1 flex items-center justify-center cursor-pointer shadow-glow-white hover:shadow-glow-blue"
          onclick={() => {
            nav?.scrollTo({ left: nav.scrollLeft - 100, behavior: 'smooth' });
          }}
        >
          <MaterialSymbolsArrowLeft class="w-4 h-4" />
        </button>
      </div>
    {/if}
    {#if navScrollable?.scrollable}
      <div class="sticky right-0 h-full flex items-center justify-center">
        <button
          class="rounded-full dark:bg-gray-900/50 bg-gray-200 dark:text-gray-200 text-gray-900 p-1 flex items-center justify-center cursor-pointer shadow-glow-white hover:shadow-glow-blue"
          onclick={() => {
            nav?.scrollTo({ left: nav.scrollWidth, behavior: 'smooth' });
          }}
        >
          <MaterialSymbolsArrowRight class="w-4 h-4" />
        </button>
      </div>
    {/if}
  </nav>
  <ThemeSwitch />
</header>
