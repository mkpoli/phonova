# Task specs — phase 7 (polish + demo) — skeleton

Skeleton to be expanded at phase start. Lane key and standing constraints as
in `phase-1.md`. Acceptance criteria: the five wow-moments in `../ux.md`,
run live and unassisted.

### T7.1 · grok · command palette
Ctrl/Cmd-K overlay covering the complete action surface with API-parity
naming (`ux.md` §Overlays); fuzzy search; recent-first; every palette entry
executes through the same engine calls as menus/keys (wow-moment 4's
GUI-=-API demonstration depends on this parity).

### T7.2 · grok · onboarding, empty states, keyboard map
First-run empty states per ux.md; a keyboard reference overlay (`?`);
parameter-explanation copy reviewed against the de-ai checklists before
merge.

### T7.3 · codex · scripting parity demonstration
A minimal script surface (CLI or in-app console calling `phx-engine`
directly) sufficient to run wow-moment 4: palette measurement vs scripted
measurement, numerically identical, side by side.

### T7.4 · sonnet · sample project + demo deployment
A bundled sample project (openly licensed recordings, annotated) and the
public read-only web deployment with it preloaded.

### T7.5 · architect · demo script + external dry run
The five-moment demo script written and rehearsed; a phonetician outside the
project runs it unassisted; findings triaged into fix-now vs backlog.

### T7.6 · architect · v0.1 gate
All five wow-moments pass live; naming decision (`../naming.md`) executed;
release notes; tag v0.1.

Sequencing: T7.1 → T7.3 (palette names feed the parity demo); T7.2/T7.4
parallel; T7.5 after all; T7.6 last.
