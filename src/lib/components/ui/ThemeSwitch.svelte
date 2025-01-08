<script lang="ts">
  import { config } from '$lib/config.svelte';
</script>

<select
  class="bg-gray-200 dark:bg-gray-900 text-gray-900 dark:text-gray-200 rounded-md px-2 py-1 w-24"
  bind:value={config.theme}
>
  <option value="auto">Auto</option>
  <option value="light">Light</option>
  <option value="dark">Dark</option>
</select>
