import { getContext, setContext } from 'svelte';

/**
 * The two key modes Phonia ships. `phonia` is the app's own convention set
 * (Tab moves between intervals, matching a modern text/code editor); `praat`
 * matches Praat's documented Sound-editor and TextGridEditor behavior as
 * closely as Phonia's command surface allows — most visibly, Tab plays
 * instead of navigating. See `docs/research/praat-shortcuts.md` for the
 * cited research behind every Praat-mode binding below.
 */
export type KeyModeId = 'praat' | 'phonia';

export const KEY_MODES: readonly { id: KeyModeId; label: string; description: string }[] = [
  {
    id: 'phonia',
    label: 'Phonia',
    description:
      'Tab moves to the next interval while a tier is focused. Space plays. Matches a modern text editor more than a tape recorder.'
  },
  {
    id: 'praat',
    label: 'Praat-compatible',
    description:
      'Tab plays or stops, matching Praat’s Sound and TextGrid editors — Space still plays too, since Praat leaves it unbound. Interval navigation has no default key, since Praat has none.'
  }
];

/** A key chord, identified by physical key (`KeyboardEvent.code`) plus modifiers.
 *  Ctrl and Cmd are folded into one `primary` flag so a binding matches on
 *  every platform without a separate Mac table. */
export interface Chord {
  code: string;
  primary?: boolean;
  shift?: boolean;
  alt?: boolean;
}

/** One scope per place in the UI that owns its own keydown listener. A chord
 *  can repeat freely across scopes — Tab in the editor and Tab in the tier
 *  pane never contend — but must stay unique within one. */
export type KeyScope = 'editor' | 'tierpane' | 'global';

/** Static description of a rebindable command, independent of whether the
 *  screen that runs it is currently mounted — the shortcut editor needs to
 *  list every command regardless of live registration state. */
export interface KeymapCommand {
  id: string;
  title: string;
  group: string;
  scope: KeyScope;
}

export const KEYMAP_COMMANDS: readonly KeymapCommand[] = [
  { id: 'playPause', title: 'Play / pause', group: 'Playback', scope: 'editor' },
  { id: 'playWindow', title: 'Play visible window', group: 'Playback', scope: 'editor' },
  { id: 'fitFile', title: 'Fit whole file', group: 'View', scope: 'editor' },
  { id: 'zoomToSelection', title: 'Zoom to selection', group: 'View', scope: 'editor' },
  { id: 'toggleInspector', title: 'Toggle inspector', group: 'View', scope: 'editor' },
  { id: 'toggleWaveform', title: 'Toggle waveform pane', group: 'View', scope: 'editor' },
  { id: 'clearSelection', title: 'Clear selection', group: 'Selection', scope: 'editor' },
  { id: 'exportFigure', title: 'Export figure', group: 'Figures', scope: 'editor' },
  { id: 'insertBoundary', title: 'Split interval at cursor', group: 'Annotation', scope: 'tierpane' },
  { id: 'removeBoundary', title: 'Merge intervals', group: 'Annotation', scope: 'tierpane' },
  { id: 'editLabel', title: 'Edit label', group: 'Annotation', scope: 'tierpane' },
  { id: 'nextInterval', title: 'Next interval', group: 'Annotation', scope: 'tierpane' },
  { id: 'previousInterval', title: 'Previous interval', group: 'Annotation', scope: 'tierpane' },
  { id: 'undo', title: 'Undo', group: 'Annotation', scope: 'global' },
  { id: 'redo', title: 'Redo', group: 'Annotation', scope: 'global' }
];

const code = (code: string, mods: Partial<Omit<Chord, 'code'>> = {}): Chord => ({ code, ...mods });

/** Phonia's own bindings — the shipped defaults prior to key modes, unchanged. */
export const PHONIA_BINDINGS: Readonly<Record<string, readonly Chord[]>> = {
  playPause: [code('Space')],
  playWindow: [],
  fitFile: [code('Digit0')],
  zoomToSelection: [code('KeyF')],
  toggleInspector: [code('KeyI')],
  toggleWaveform: [code('KeyW')],
  clearSelection: [code('Escape')],
  exportFigure: [code('KeyE')],
  insertBoundary: [code('KeyS')],
  removeBoundary: [code('KeyM')],
  editLabel: [code('Enter')],
  nextInterval: [code('Tab')],
  previousInterval: [code('Tab', { shift: true })],
  undo: [code('KeyZ', { primary: true })],
  redo: [code('KeyZ', { primary: true, shift: true }), code('KeyY', { primary: true })]
};

/**
 * Praat-compatible bindings, researched against Praat's source
 * (`FunctionEditor.cpp`'s `PLAY_DATA__playOrStop`) and manual — see
 * docs/research/praat-shortcuts.md for the full citation trail.
 *
 * - `playPause` on Tab is Praat's real "play or stop" toggle: press to play
 *   (selection, else the rest of the window from the cursor, else the whole
 *   window), press again mid-playback to stop where the cursor sits. Phonia's
 *   existing `handleTransportToggle` already implements that same priority
 *   order for Space, so Tab reuses it verbatim rather than a new function —
 *   "as closely as our command surface allows" is exact here, not
 *   approximate. One deliberate deviation: real Praat leaves Space bound to
 *   nothing at all, which the research confirmed by grepping the editor
 *   source for a bare `' '` accelerator. Removing Space here would make a
 *   modern-feeling app go silent on the key most users try first, so Space
 *   stays live as an addition on top of Praat's own binding, not a faithful
 *   copy of its absence.
 * - `playWindow` on Shift-Tab matches Praat's "Play window" — always plays
 *   the entire visible window regardless of selection, and (unlike Tab)
 *   restarts rather than toggling to a stop on a second press.
 * - Interval navigation (`nextInterval`/`previousInterval`) has no Praat
 *   equivalent — Praat has no keyboard command that moves the selected
 *   interval — so both ship unbound rather than guessing a replacement key;
 *   a Praat-mode user can add one in the shortcut editor.
 * - Every command with no documented Praat behavior — the view/appearance
 *   actions Phonia adds on top of Praat's surface — keeps its Phonia-mode
 *   key, since there is nothing in Praat for it to conflict with.
 */
export const PRAAT_BINDINGS: Readonly<Record<string, readonly Chord[]>> = {
  ...PHONIA_BINDINGS,
  playPause: [code('Space'), code('Tab')],
  playWindow: [code('Tab', { shift: true })],
  nextInterval: [],
  previousInterval: []
};

export const DEFAULT_BINDINGS: Readonly<Record<KeyModeId, Readonly<Record<string, readonly Chord[]>>>> = {
  phonia: PHONIA_BINDINGS,
  praat: PRAAT_BINDINGS
};

// --- Chord matching, formatting ---

export function chordFromEvent(event: KeyboardEvent): Chord {
  return {
    code: event.code,
    primary: event.ctrlKey || event.metaKey,
    shift: event.shiftKey,
    alt: event.altKey
  };
}

export function chordEquals(a: Chord, b: Chord): boolean {
  return a.code === b.code && !!a.primary === !!b.primary && !!a.shift === !!b.shift && !!a.alt === !!b.alt;
}

const isLetterCode = (codeValue: string): boolean => /^Key[A-Z]$/.test(codeValue);

/**
 * Whether a live keydown `event` fires `bound`. Exact on every modifier
 * except one deliberate forgiveness: a plain-letter binding with no Shift of
 * its own (e.g. `S` for split) still fires when Shift is only there to
 * produce a capital letter, not to request a different chord. Tab, digits,
 * and every other physical key stay exact-match — only letters have a case.
 */
export function chordMatchesEvent(bound: Chord, event: Chord): boolean {
  if (bound.code !== event.code) return false;
  if (!!bound.primary !== !!event.primary) return false;
  if (!!bound.alt !== !!event.alt) return false;
  if (!!bound.shift === !!event.shift) return true;
  return !bound.shift && isLetterCode(bound.code);
}

export function chordKey(chord: Chord): string {
  return `${chord.primary ? 'primary+' : ''}${chord.alt ? 'alt+' : ''}${chord.shift ? 'shift+' : ''}${chord.code}`;
}

const isMac =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

const CODE_LABELS: Record<string, string> = {
  Space: 'Space',
  Tab: 'Tab',
  Enter: 'Enter',
  Escape: 'Esc',
  Backspace: 'Backspace',
  Delete: 'Delete',
  ArrowLeft: '←',
  ArrowRight: '→',
  ArrowUp: '↑',
  ArrowDown: '↓',
  Digit0: '0',
  Digit1: '1',
  Digit2: '2',
  Digit3: '3',
  Digit4: '4',
  Digit5: '5',
  Digit6: '6',
  Digit7: '7',
  Digit8: '8',
  Digit9: '9',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backquote: '`',
  CapsLock: 'Caps'
};

/** The single-key label for a physical key code, as shown on a keycap. */
export function codeLabel(codeValue: string): string {
  if (codeValue in CODE_LABELS) return CODE_LABELS[codeValue];
  if (codeValue.startsWith('Key')) return codeValue.slice(3);
  if (codeValue.startsWith('Digit')) return codeValue.slice(5);
  return codeValue;
}

/** Full chord label for shortcut rows and tooltips, e.g. "Shift+Tab", "Ctrl/Cmd+Z". */
export function chordLabel(chord: Chord): string {
  const parts: string[] = [];
  if (chord.primary) parts.push(isMac ? '⌘' : 'Ctrl');
  if (chord.alt) parts.push(isMac ? '⌥' : 'Alt');
  if (chord.shift) parts.push(isMac ? '⇧' : 'Shift');
  parts.push(codeLabel(chord.code));
  return parts.join(isMac ? '' : '+');
}

// --- Virtual keyboard layout ---

export interface KeyboardKey {
  code: string;
  label: string;
  /** Width in key units; 1 = one standard keycap. */
  width?: number;
}

/** A compact but complete ANSI-ish layout, physical-key order. Modifier keys
 *  (Shift/Ctrl/Alt/Meta/CapsLock) render as inert frame keys — they set the
 *  board's modifier state via the tabs above it, not by being bindable
 *  targets themselves. */
export const KEYBOARD_ROWS: readonly (readonly KeyboardKey[])[] = [
  [
    { code: 'Escape', label: 'Esc' },
    { code: 'F1', label: 'F1' },
    { code: 'F2', label: 'F2' },
    { code: 'F3', label: 'F3' },
    { code: 'F4', label: 'F4' },
    { code: 'F5', label: 'F5' },
    { code: 'F6', label: 'F6' },
    { code: 'F7', label: 'F7' },
    { code: 'F8', label: 'F8' },
    { code: 'F9', label: 'F9' },
    { code: 'F10', label: 'F10' },
    { code: 'F11', label: 'F11' },
    { code: 'F12', label: 'F12' }
  ],
  [
    { code: 'Backquote', label: '`' },
    { code: 'Digit1', label: '1' },
    { code: 'Digit2', label: '2' },
    { code: 'Digit3', label: '3' },
    { code: 'Digit4', label: '4' },
    { code: 'Digit5', label: '5' },
    { code: 'Digit6', label: '6' },
    { code: 'Digit7', label: '7' },
    { code: 'Digit8', label: '8' },
    { code: 'Digit9', label: '9' },
    { code: 'Digit0', label: '0' },
    { code: 'Minus', label: '-' },
    { code: 'Equal', label: '=' },
    { code: 'Backspace', label: 'Backspace', width: 2 }
  ],
  [
    { code: 'Tab', label: 'Tab', width: 1.5 },
    { code: 'KeyQ', label: 'Q' },
    { code: 'KeyW', label: 'W' },
    { code: 'KeyE', label: 'E' },
    { code: 'KeyR', label: 'R' },
    { code: 'KeyT', label: 'T' },
    { code: 'KeyY', label: 'Y' },
    { code: 'KeyU', label: 'U' },
    { code: 'KeyI', label: 'I' },
    { code: 'KeyO', label: 'O' },
    { code: 'KeyP', label: 'P' },
    { code: 'BracketLeft', label: '[' },
    { code: 'BracketRight', label: ']' },
    { code: 'Backslash', label: '\\', width: 1.5 }
  ],
  [
    { code: 'CapsLock', label: 'Caps', width: 1.75 },
    { code: 'KeyA', label: 'A' },
    { code: 'KeyS', label: 'S' },
    { code: 'KeyD', label: 'D' },
    { code: 'KeyF', label: 'F' },
    { code: 'KeyG', label: 'G' },
    { code: 'KeyH', label: 'H' },
    { code: 'KeyJ', label: 'J' },
    { code: 'KeyK', label: 'K' },
    { code: 'KeyL', label: 'L' },
    { code: 'Semicolon', label: ';' },
    { code: 'Quote', label: "'" },
    { code: 'Enter', label: 'Enter', width: 2.25 }
  ],
  [
    { code: 'ShiftLeft', label: 'Shift', width: 2.25 },
    { code: 'KeyZ', label: 'Z' },
    { code: 'KeyX', label: 'X' },
    { code: 'KeyC', label: 'C' },
    { code: 'KeyV', label: 'V' },
    { code: 'KeyB', label: 'B' },
    { code: 'KeyN', label: 'N' },
    { code: 'KeyM', label: 'M' },
    { code: 'Comma', label: ',' },
    { code: 'Period', label: '.' },
    { code: 'Slash', label: '/' },
    { code: 'ShiftRight', label: 'Shift', width: 2.75 }
  ],
  [
    { code: 'ControlLeft', label: 'Ctrl', width: 1.25 },
    { code: 'AltLeft', label: 'Alt', width: 1.25 },
    { code: 'Space', label: 'Space', width: 6.25 },
    { code: 'AltRight', label: 'Alt', width: 1.25 },
    { code: 'ArrowLeft', label: '←' },
    { code: 'ArrowUp', label: '↑' },
    { code: 'ArrowDown', label: '↓' },
    { code: 'ArrowRight', label: '→' }
  ]
];

/** Physical keys that hold a modifier rather than a bindable action — the
 *  board dims these and never assigns them a command. */
export const MODIFIER_CODES: ReadonlySet<string> = new Set([
  'ShiftLeft',
  'ShiftRight',
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'MetaLeft',
  'MetaRight',
  'CapsLock'
]);

// --- Persistence ---

const MODE_KEY = 'phonia:key-mode';
const OVERRIDES_KEY = 'phonia:key-overrides';

type OverrideMap = Record<KeyModeId, Record<string, Chord[]>>;

function emptyOverrides(): OverrideMap {
  return { praat: {}, phonia: {} };
}

function loadMode(): KeyModeId | null {
  try {
    const raw = globalThis.localStorage?.getItem(MODE_KEY);
    return raw === 'praat' || raw === 'phonia' ? raw : null;
  } catch {
    return null;
  }
}

function saveMode(mode: KeyModeId): void {
  try {
    globalThis.localStorage?.setItem(MODE_KEY, mode);
  } catch {
    // A blocked storage means the mode choice does not survive reload; not
    // worth surfacing to the user for a preference this low-stakes.
  }
}

function loadOverrides(): OverrideMap {
  try {
    const raw = globalThis.localStorage?.getItem(OVERRIDES_KEY);
    if (!raw) return emptyOverrides();
    const parsed = JSON.parse(raw);
    return {
      praat: parsed?.praat ?? {},
      phonia: parsed?.phonia ?? {}
    };
  } catch {
    return emptyOverrides();
  }
}

function saveOverrides(overrides: OverrideMap): void {
  try {
    globalThis.localStorage?.setItem(OVERRIDES_KEY, JSON.stringify(overrides));
  } catch {
    // Best-effort persistence, same reasoning as saveMode.
  }
}

/** A conflict: two commands in the same scope both claim `chord`. */
export interface KeyConflict {
  chord: Chord;
  commandIds: string[];
}

/**
 * Reactive key-binding state: the active mode, and per-mode user overrides
 * layered on top of the mode's defaults. One instance lives at the app root
 * and is read by every scope's keydown handler and by the shortcut editor.
 */
export class KeyBindingsStore {
  mode = $state<KeyModeId>(loadMode() ?? 'phonia');
  #overrides = $state<OverrideMap>(loadOverrides());
  /** True until the user has answered (or dismissed) the first-run prompt. */
  promptDue = $state<boolean>(loadMode() === null);

  /** The resolved chords for a command under the active mode: an override
   *  when one exists, else the mode's default. */
  chordsFor(commandId: string): readonly Chord[] {
    const override = this.#overrides[this.mode][commandId];
    if (override) return override;
    return DEFAULT_BINDINGS[this.mode][commandId] ?? [];
  }

  /** Whether `commandId`'s current chords differ from the mode's shipped default. */
  isCustomized(commandId: string): boolean {
    return commandId in this.#overrides[this.mode];
  }

  /** The command in `scope` bound to `chord` under the active mode, if any. */
  commandForChord(scope: KeyScope, chord: Chord): string | undefined {
    for (const command of KEYMAP_COMMANDS) {
      if (command.scope !== scope) continue;
      if (this.chordsFor(command.id).some((bound) => chordEquals(bound, chord))) return command.id;
    }
    return undefined;
  }

  /** Every conflict in the active mode: two commands in one scope sharing a chord. */
  conflicts(): KeyConflict[] {
    const byScope = new Map<KeyScope, Map<string, string[]>>();
    for (const command of KEYMAP_COMMANDS) {
      let byChord = byScope.get(command.scope);
      if (!byChord) {
        byChord = new Map();
        byScope.set(command.scope, byChord);
      }
      for (const chord of this.chordsFor(command.id)) {
        const key = chordKey(chord);
        const ids = byChord.get(key) ?? [];
        ids.push(command.id);
        byChord.set(key, ids);
      }
    }
    const out: KeyConflict[] = [];
    for (const byChord of byScope.values()) {
      for (const ids of byChord.values()) {
        if (ids.length < 2) continue;
        const first = KEYMAP_COMMANDS.find((c) => c.id === ids[0]);
        const chord = first ? this.chordsFor(first.id).find((c) => ids.length > 0) : undefined;
        if (chord) out.push({ chord, commandIds: [...ids] });
      }
    }
    return out;
  }

  setMode(mode: KeyModeId): void {
    this.mode = mode;
    saveMode(mode);
  }

  /** Records the user's first-run choice; the prompt never returns after this. */
  answerPrompt(mode: KeyModeId): void {
    this.setMode(mode);
    this.promptDue = false;
  }

  /** Dismissing without choosing keeps the current (default Phonia) mode and
   *  still counts as answered — the prompt does not nag on later visits. */
  dismissPrompt(): void {
    saveMode(this.mode);
    this.promptDue = false;
  }

  /** Rebinds `commandId` to exactly `chords` in the active mode. */
  setOverride(commandId: string, chords: Chord[]): void {
    this.#overrides = {
      ...this.#overrides,
      [this.mode]: { ...this.#overrides[this.mode], [commandId]: chords }
    };
    saveOverrides(this.#overrides);
  }

  /** Restores `commandId` to its mode default. */
  clearOverride(commandId: string): void {
    const rest = { ...this.#overrides[this.mode] };
    delete rest[commandId];
    this.#overrides = { ...this.#overrides, [this.mode]: rest };
    saveOverrides(this.#overrides);
  }

  /** Restores every command in the active mode to its shipped defaults. */
  resetMode(): void {
    this.#overrides = { ...this.#overrides, [this.mode]: {} };
    saveOverrides(this.#overrides);
  }

  /** Display label for a command's current chords, e.g. "Tab", "Shift+Tab",
   *  or "" when the command carries no binding in the active mode — palette
   *  rows and shortcut hints read this instead of a hardcoded string, so they
   *  stay correct across a mode switch or a rebind. */
  labelFor(commandId: string): string {
    return this.chordsFor(commandId).map(chordLabel).join(' / ');
  }
}

const STORE_KEY = Symbol('phonia-key-bindings');

export function provideKeyBindings(): KeyBindingsStore {
  const store = new KeyBindingsStore();
  setContext(STORE_KEY, store);
  return store;
}

export function getKeyBindings(): KeyBindingsStore | null {
  return getContext<KeyBindingsStore | undefined>(STORE_KEY) ?? null;
}
