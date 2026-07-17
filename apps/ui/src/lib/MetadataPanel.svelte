<script lang="ts">
  import IconX from '~icons/lucide/x';

  interface Metadata {
    description: string;
    authors: string[];
    tags: string[];
  }

  interface Props {
    /** What the panel edits: the whole project, or one recording. */
    scope: 'project' | 'recording';
    /** Heading naming the subject (project or recording name). */
    title: string;
    description: string;
    authors: string[];
    tags: string[];
    onSave: (metadata: Metadata) => void;
    onClose: () => void;
  }

  let { scope, title, description, authors, tags, onSave, onClose }: Props = $props();

  // Local drafts, seeded from props. The parent remounts the panel per subject
  // (keyed on the target), so these initialize fresh each time it opens.
  // svelte-ignore state_referenced_locally
  let draftDescription = $state(description);
  // svelte-ignore state_referenced_locally
  let draftAuthors = $state<string[]>([...authors]);
  // svelte-ignore state_referenced_locally
  let draftTags = $state<string[]>([...tags]);
  let authorInput = $state('');
  let tagInput = $state('');

  function persist() {
    onSave({
      description: draftDescription,
      authors: draftAuthors,
      tags: draftTags
    });
  }

  function addAuthor() {
    const value = authorInput.trim();
    authorInput = '';
    if (!value || draftAuthors.includes(value)) return;
    draftAuthors = [...draftAuthors, value];
    persist();
  }

  function removeAuthor(name: string) {
    draftAuthors = draftAuthors.filter((a) => a !== name);
    persist();
  }

  function addTag() {
    const value = tagInput.trim();
    tagInput = '';
    if (!value || draftTags.includes(value)) return;
    draftTags = [...draftTags, value];
    persist();
  }

  function removeTag(tag: string) {
    draftTags = draftTags.filter((t) => t !== tag);
    persist();
  }

  function chipKeydown(event: KeyboardEvent, add: () => void) {
    if (event.key === 'Enter' || event.key === ',') {
      event.preventDefault();
      add();
    }
  }
</script>

<aside class="panel" data-testid="metadata-panel" data-scope={scope} aria-label={`Details: ${title}`}>
  <header>
    <div class="heading">
      <span class="eyebrow">{scope === 'project' ? 'Project' : 'Recording'}</span>
      <h2 title={title}>{title}</h2>
    </div>
    <button type="button" class="close" aria-label="Close details" data-testid="metadata-close" onclick={onClose}>
      <IconX aria-hidden="true" />
    </button>
  </header>

  <label class="field">
    <span class="label">Description</span>
    <textarea
      class="description"
      data-testid="metadata-description"
      rows="4"
      placeholder="Notes about this {scope === 'project' ? 'project' : 'recording'}"
      bind:value={draftDescription}
      onblur={persist}
    ></textarea>
  </label>

  <div class="field">
    <span class="label">Authors</span>
    {#if draftAuthors.length > 0}
      <div class="chips" data-testid="metadata-authors">
        {#each draftAuthors as author (author)}
          <span class="chip">
            {author}
            <button type="button" aria-label={`Remove ${author}`} onclick={() => removeAuthor(author)}>
              <IconX aria-hidden="true" />
            </button>
          </span>
        {/each}
      </div>
    {/if}
    <input
      type="text"
      class="chip-input"
      data-testid="metadata-author-input"
      placeholder="Add author, press Enter"
      bind:value={authorInput}
      onkeydown={(event) => chipKeydown(event, addAuthor)}
      onblur={addAuthor}
    />
  </div>

  <div class="field">
    <span class="label">Tags</span>
    {#if draftTags.length > 0}
      <div class="chips" data-testid="metadata-tags">
        {#each draftTags as tag (tag)}
          <span class="chip tag">
            {tag}
            <button type="button" aria-label={`Remove ${tag}`} onclick={() => removeTag(tag)}>
              <IconX aria-hidden="true" />
            </button>
          </span>
        {/each}
      </div>
    {/if}
    <input
      type="text"
      class="chip-input"
      data-testid="metadata-tag-input"
      placeholder="Add tag, press Enter"
      bind:value={tagInput}
      onkeydown={(event) => chipKeydown(event, addTag)}
      onblur={addTag}
    />
  </div>
</aside>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    width: 20rem;
    padding: 1rem 1.1rem;
    border-left: 1px solid var(--chrome-strong);
    background: var(--panel);
    overflow-y: auto;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .eyebrow {
    font-size: 0.7rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }

  h2 {
    margin: 0.1rem 0 0;
    font-size: 1rem;
    font-weight: 600;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    padding: 0.25rem;
  }

  .close:hover {
    color: var(--text);
    background: var(--panel-soft);
    border-color: var(--chrome-strong);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .label {
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .description,
  .chip-input {
    font: inherit;
    color: var(--text);
    background: var(--panel-soft);
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    padding: 0.4rem 0.55rem;
    width: 100%;
  }

  .description {
    resize: vertical;
    min-height: 3.5rem;
  }

  .description:focus,
  .chip-input:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.28rem;
    padding: 0.12rem 0.2rem 0.12rem 0.55rem;
    border-radius: 999px;
    background: var(--panel-soft);
    border: 1px solid var(--chrome-strong);
    color: var(--text);
    font-size: 0.8rem;
  }

  .chip.tag {
    background: var(--accent-tint);
    border-color: color-mix(in oklab, var(--accent) 30%, transparent);
    color: var(--accent-strong);
  }

  .chip button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: inherit;
    opacity: 0.6;
    padding: 0.05rem;
    border-radius: 999px;
  }

  .chip button:hover {
    opacity: 1;
  }

  .chip button :global(svg) {
    font-size: 0.7rem;
  }
</style>
