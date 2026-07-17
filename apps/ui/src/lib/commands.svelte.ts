import { getContext, onDestroy, setContext } from 'svelte';

/** The eight areas the palette groups actions under, in display order. */
export type CommandGroup =
  | 'Playback'
  | 'View'
  | 'Analysis'
  | 'Annotation'
  | 'Selection'
  | 'Figures'
  | 'Project'
  | 'Appearance';

/** Group display order, fixed so the palette reads the same on every screen. */
export const COMMAND_GROUP_ORDER: readonly CommandGroup[] = [
  'Playback',
  'View',
  'Analysis',
  'Annotation',
  'Selection',
  'Figures',
  'Project',
  'Appearance'
];

/**
 * One palette action. `id` and `api` carry the engine surface names so the
 * palette doubles as searchable API documentation; a script driving the engine
 * finds the same call the palette runs.
 */
export interface Command {
  /** Stable, unique action id. Equals the engine method name where one exists. */
  id: string;
  /** Human title shown in the palette, plain register. */
  title: string;
  group: CommandGroup;
  /** Engine-API names this action drives, searchable as aliases. */
  api?: readonly string[];
  /** Shortcut label shown on the row, when the action has a key binding. */
  shortcut?: string;
  /** Extra search terms that do not belong in the visible title. */
  keywords?: readonly string[];
  /** Whether the action can run now; a false result hides the entry. */
  enabled?: () => boolean;
  /** Runs the action through the same code path as its button or key. */
  run: () => void | Promise<void>;
}

const RECENT_KEY = 'phonix-command-recent';
const RECENT_MAX = 8;

function loadRecent(): string[] {
  try {
    const raw = globalThis.localStorage?.getItem(RECENT_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((entry) => typeof entry === 'string') : [];
  } catch {
    return [];
  }
}

function saveRecent(list: string[]): void {
  try {
    globalThis.localStorage?.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    // A blocked or absent storage is not worth surfacing; recency is best-effort.
  }
}

/**
 * The command surface the screens contribute to. Each screen registers its
 * actions while mounted and drops them on unmount, so the palette holds exactly
 * the actions the current screen can run — the source of the palette's
 * context-sensitivity.
 */
export class CommandRegistry {
  // A key→actions map, kept off the reactive graph so removing one screen's
  // actions can never disturb another's. A version signal drives readers.
  #sources = new Map<symbol, Command[]>();
  #version = $state(0);
  /** Recently run action ids, most recent first, persisted best-effort. */
  recent = $state<string[]>(loadRecent());

  /** Every registered action, in registration order. */
  get commands(): Command[] {
    void this.#version;
    return [...this.#sources.values()].flat();
  }

  /** Registers a set of actions and returns a disposer that removes them. */
  register(commands: Command[]): () => void {
    const token = Symbol('command-source');
    this.#sources.set(token, commands);
    this.#version += 1;
    return () => {
      if (this.#sources.delete(token)) this.#version += 1;
    };
  }

  /** The action with this id, if one is registered. */
  find(id: string): Command | undefined {
    return this.commands.find((command) => command.id === id);
  }

  /** Runs an action by id, recording it as recent. Disabled actions no-op. */
  async run(id: string): Promise<void> {
    const command = this.find(id);
    if (!command) return;
    if (command.enabled && !command.enabled()) return;
    this.recent = [id, ...this.recent.filter((entry) => entry !== id)].slice(0, RECENT_MAX);
    saveRecent(this.recent);
    await command.run();
  }
}

const REGISTRY_KEY = Symbol('phonia-command-registry');

/** Creates a registry and publishes it to descendants. Call during init. */
export function provideCommandRegistry(): CommandRegistry {
  const registry = new CommandRegistry();
  setContext(REGISTRY_KEY, registry);
  return registry;
}

/** The registry published by an ancestor, or `null` outside a provider. */
export function getCommandRegistry(): CommandRegistry | null {
  return getContext<CommandRegistry | undefined>(REGISTRY_KEY) ?? null;
}

/**
 * Registers a screen's actions for its lifetime. Call during component init;
 * the actions drop automatically when the component unmounts.
 */
export function registerCommands(commands: Command[]): void {
  const registry = getCommandRegistry();
  if (!registry) return;
  const dispose = registry.register(commands);
  onDestroy(dispose);
}

/** A matched command with its fuzzy score, higher meaning a closer match. */
export interface CommandMatch {
  command: Command;
  score: number;
}

const WORD_BREAK = /[\s\-_/]/;

/**
 * Scores `query` as a subsequence of `text`, rewarding consecutive runs and
 * word-start hits. Returns `null` when the query is not a subsequence.
 */
function subsequenceScore(query: string, text: string): number | null {
  let qi = 0;
  let score = 0;
  let previous = -2;
  for (let ti = 0; ti < text.length && qi < query.length; ti += 1) {
    if (text[ti] !== query[qi]) continue;
    let step = 1;
    if (ti === previous + 1) step += 2;
    if (ti === 0 || WORD_BREAK.test(text[ti - 1])) step += 3;
    score += step;
    previous = ti;
    qi += 1;
  }
  return qi === query.length ? score : null;
}

/** The best field score for a command against a query, or `null` if no field matches. */
function commandScore(command: Command, query: string): number | null {
  const title = subsequenceScore(query, command.title.toLowerCase());
  const api = command.api
    ?.map((name) => subsequenceScore(query, name.toLowerCase()))
    .reduce<number | null>((best, value) => bestOf(best, value), null);
  const keywords = command.keywords
    ?.map((word) => subsequenceScore(query, word.toLowerCase()))
    .reduce<number | null>((best, value) => bestOf(best, value), null);
  const group = subsequenceScore(query, command.group.toLowerCase());
  const weighted = [
    scale(title, 2),
    scale(api ?? null, 1.5),
    scale(keywords ?? null, 1),
    scale(group, 0.5)
  ];
  return weighted.reduce<number | null>((best, value) => bestOf(best, value), null);
}

function scale(value: number | null, factor: number): number | null {
  return value === null ? null : value * factor;
}

function bestOf(a: number | null, b: number | null): number | null {
  if (a === null) return b;
  if (b === null) return a;
  return Math.max(a, b);
}

/**
 * Ranks commands against a query. An empty query keeps registration order; a
 * non-empty query returns only matches, best first.
 */
export function searchCommands(commands: Command[], query: string): Command[] {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return commands;
  const matches: CommandMatch[] = [];
  for (const command of commands) {
    const score = commandScore(command, trimmed);
    if (score !== null) matches.push({ command, score });
  }
  matches.sort((a, b) => b.score - a.score || a.command.title.localeCompare(b.command.title));
  return matches.map((match) => match.command);
}
