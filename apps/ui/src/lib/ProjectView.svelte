<script lang="ts">
  import WaveThumb from './WaveThumb.svelte';
  import { filesFromDataTransfer } from './dnd';
  import { registerCommands } from './commands.svelte';
  import { formatTime, type CoreClientLike, type RecordingEntry } from './types';

  interface Props {
    client: CoreClientLike | null;
    projectName: string;
    recordings: RecordingEntry[];
    theme: 'light' | 'dark';
    busy: boolean;
    busyLabel: string;
    dirty: boolean;
    onOpenRecording: (recording: RecordingEntry) => void;
    onImportFiles: (files: File[]) => void;
    onBack: () => void;
    onSave: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
  }

  let {
    client,
    projectName,
    recordings,
    theme,
    busy,
    busyLabel,
    dirty,
    onOpenRecording,
    onImportFiles,
    onBack,
    onSave,
    onThemeChange
  }: Props = $props();

  let dragging = $state(false);
  let fileInput = $state<HTMLInputElement | null>(null);

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    if (!event.dataTransfer) return;
    const files = await filesFromDataTransfer(event.dataTransfer);
    if (files.length > 0) onImportFiles(files);
  }

  function handleInput(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = '';
    if (files.length > 0) onImportFiles(files);
  }

  function sampleRateLabel(hz: number): string {
    return hz >= 1000 ? `${(hz / 1000).toFixed(hz % 1000 === 0 ? 0 : 1)} kHz` : `${hz} Hz`;
  }

  registerCommands([
    {
      id: 'saveProject',
      title: 'Save project',
      group: 'Project',
      api: ['saveProjectContainer'],
      keywords: ['write', 'store'],
      enabled: () => dirty,
      run: () => onSave()
    },
    {
      id: 'backToHome',
      title: 'Back to projects',
      group: 'Project',
      keywords: ['home', 'exit', 'close project'],
      run: () => onBack()
    }
  ]);
</script>

<div
  class="project"
  class:dragging
  data-testid="corpus"
  data-project-name={projectName}
  data-recording-count={recordings.length}
  role="region"
  aria-label={`Project ${projectName}`}
  ondragover={(event) => {
    event.preventDefault();
    dragging = true;
  }}
  ondragleave={(event) => {
    if (event.currentTarget === event.target) dragging = false;
  }}
  ondrop={handleDrop}
>
  <header class="top">
    <div class="left">
      <button type="button" class="ghost" onclick={onBack} data-testid="back-home">← Projects</button>
      <span class="name">{projectName}</span>
    </div>
    <div class="right">
      <span class="dirty" data-testid="dirty-state" data-dirty={dirty}>
        {dirty ? 'Unsaved changes' : 'All changes saved'}
      </span>
      <button type="button" onclick={onSave} data-testid="save-project" disabled={!dirty}>Save</button>
      <button
        type="button"
        class="ghost"
        aria-label="Toggle theme"
        onclick={() => onThemeChange(theme === 'light' ? 'dark' : 'light')}
      >
        {theme === 'light' ? 'Dark' : 'Light'}
      </button>
    </div>
  </header>

  <input
    bind:this={fileInput}
    type="file"
    accept=".wav,audio/wav,.TextGrid"
    multiple
    class="hidden-input"
    data-testid="corpus-file-input"
    onchange={handleInput}
  />

  <main class="body">
    {#if recordings.length === 0}
      <div class="empty" data-testid="corpus-empty">
        <p class="empty-lead">No recordings yet.</p>
        <p class="empty-sub">
          Drop WAV files here, or choose them. A TextGrid beside a WAV of the same name attaches as
          its annotation.
        </p>
        <button type="button" class="empty-action" data-testid="corpus-choose-files" onclick={() => fileInput?.click()}>
          Choose files
        </button>
      </div>
    {:else}
      <table class="corpus">
        <thead>
          <tr>
            <th class="thumb-col">Waveform</th>
            <th>Name</th>
            <th class="num">Duration</th>
            <th class="num">Sample rate</th>
            <th class="num">Channels</th>
            <th>Annotation</th>
          </tr>
        </thead>
        <tbody>
          {#each recordings as recording (recording.mediaId)}
            <tr
              class="row"
              data-testid="corpus-row"
              data-recording-name={recording.name}
              data-has-annotation={recording.hasAnnotation}
              tabindex="0"
              role="button"
              onclick={() => onOpenRecording(recording)}
              onkeydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  onOpenRecording(recording);
                }
              }}
            >
              <td class="thumb-col">
                <WaveThumb
                  {client}
                  audioId={recording.audioId}
                  duration={recording.duration}
                  {theme}
                />
              </td>
              <td class="name-cell">{recording.name}</td>
              <td class="num">{formatTime(recording.duration)}</td>
              <td class="num">{sampleRateLabel(recording.sampleRate)}</td>
              <td class="num">{recording.channels}</td>
              <td>
                {#if recording.hasAnnotation}
                  <span class="tag" data-testid="annotation-present">tiers</span>
                {:else}
                  <span class="tag muted">—</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </main>

  {#if dragging}
    <div class="drop-hint" aria-hidden="true"><span>Drop to add recordings</span></div>
  {/if}

  {#if busy}
    <div class="busy" role="status" data-testid="corpus-busy">{busyLabel}</div>
  {/if}
</div>

<style>
  .project {
    position: relative;
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: var(--chrome);
    color: var(--text);
  }

  .project.dragging {
    outline: 2px dashed var(--accent);
    outline-offset: -8px;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.7rem 1.1rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
  }

  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }

  .name {
    font-weight: 600;
  }

  button {
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.3rem 0.65rem;
  }

  button:hover:not(:disabled) {
    background: var(--panel);
  }

  button:disabled {
    opacity: 0.5;
  }

  .ghost {
    color: var(--muted);
  }

  .dirty {
    font-size: 0.8rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .dirty[data-dirty='true'] {
    color: var(--warn);
  }

  .body {
    min-height: 0;
    overflow: auto;
    padding: 1.25rem 1.5rem;
  }

  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    text-align: center;
    color: var(--muted);
  }

  .empty-lead {
    margin: 0;
    font-size: 1.1rem;
    color: var(--text);
  }

  .empty-sub {
    margin: 0;
    max-width: 32rem;
    font-size: 0.9rem;
  }

  .empty-action {
    margin-top: 0.4rem;
  }

  .hidden-input {
    display: none;
  }

  .corpus {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
  }

  .corpus thead th {
    text-align: left;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--chrome-strong);
    color: var(--muted);
    font-weight: 500;
    font-size: 0.8rem;
    position: sticky;
    top: 0;
    background: var(--chrome);
  }

  .corpus th.num,
  .corpus td.num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .row {
    cursor: pointer;
  }

  .row td {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--chrome-strong);
  }

  .row:hover td {
    background: var(--panel-soft);
  }

  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .thumb-col {
    width: 150px;
  }

  .name-cell {
    font-weight: 500;
  }

  .tag {
    display: inline-block;
    padding: 0.1rem 0.5rem;
    border-radius: 999px;
    background: color-mix(in oklab, var(--accent) 18%, transparent);
    color: var(--accent-strong);
    font-size: 0.76rem;
  }

  .tag.muted {
    background: transparent;
    color: var(--muted);
  }

  .drop-hint {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, var(--accent) 12%, transparent);
    pointer-events: none;
  }

  .drop-hint span {
    padding: 0.6rem 1.1rem;
    border-radius: 8px;
    background: var(--panel);
    border: 1px solid var(--accent);
    color: var(--text);
    font-weight: 600;
  }

  .busy {
    position: fixed;
    left: 50%;
    bottom: 1.25rem;
    transform: translateX(-50%);
    padding: 0.5rem 0.9rem;
    border-radius: 8px;
    border: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
    box-shadow: 0 10px 30px rgba(15, 23, 42, 0.18);
    font-size: 0.85rem;
  }
</style>
