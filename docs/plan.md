# Plan: Idleon Optimizer, a native desktop suite

Status: **foundation in progress, research partial, shadow-ban and BoneJoePickle done, 27 sections
planned.** The live breakdown is [`docs/roadmap.md`](roadmap.md); the machine-readable source is
[`docs/roadmap.json`](roadmap.json); each section's full specification is under
[`docs/sections/`](sections/).

## 1. Product goal

Rebuild [Corgan/idleon-research-optimizer](https://github.com/Corgan/idleon-research-optimizer) as a
native Rust + GPUI desktop application with full functional parity, not just the research grid.

- **Dashboard first.** The app opens on a searchable card grid, grouped by General and Worlds 2-7,
  mirroring the upstream index. Every card opens one section.
- **All upstream sections.** The 25 cards in `index.html` plus the five hidden tools (`damage`,
  `drop-rate`, `coin-multi`, `pickle`, `stats-all`) are in scope.
- **All functionality per section.** Every tab, input, table, simulator, optimizer, replay and
  breakdown in the upstream page is specified as a feature issue under that section's milestone.
- **Similar UI, same assets.** Reuse the upstream layout, palette, typography scale, icons and
  sprites where legally permitted; otherwise use a labelled placeholder.
- **Local only.** No accounts, cloud sync, telemetry or network calls. Saves and exports stay on
  disk.

Definition of done for a section: every feature issue closed, formula fixtures match upstream,
the section opens from the dashboard, the harness drives its main flow, assets resolve, and no
unexplained import warning remains.

## 2. Licensing and assets

The upstream repository's `LICENSE` says: **Copyright 2026 Jonathan Rock. All rights reserved. This
repository is provided solely for educational review. No usage, modification, distribution, or
reproduction of this code is allowed for any purpose without explicit written permission.**

The images are game assets from Legends of Idleon, not original work of the upstream repository,
so they are free to use and are committed here with credit. The upstream code licence still
reserves every other right.

- Formulas and data tables stay ports with upstream source references; the existing extraction
  script documents provenance.
- Upstream images are **committed** under `assets/upstream/` with credit in
  `assets/upstream/README.md`; they are game assets, free to use, not upstream-authored work.
  `script/import-upstream-assets.mjs` copies them from a local checkout, writes a manifest with
  hashes and records the upstream commit.
- The app resolves images through the manifest and falls back to a labelled placeholder.
- Reusing upstream code still needs written permission; keep ports as rewrites with source
  references rather than copies.

## 3. Dashboard and navigation

Landing page:

- Header: product title, search field, import button, save status.
- Card grid grouped General, W2, W3, W4, W5, W6, W7, Tools; same titles, emoji/icons, world badges
  and descriptions as upstream `index.html`.
- Each card has `id="card-<section>"`, title, badge and description as accessible text.
- Search matches title, description and group; Escape clears; Enter opens the first match.
- Last opened section is remembered; Back returns to the dashboard.
- Import accepts a raw save, an ROv2 state string, a project JSON or a section config; it previews
  the detected format and warnings before applying.

Section shell:

- Back to dashboard, section title, world badge, save status, import/export actions.
- The section owns its upstream tab strip and content; the shell owns only window chrome, status
  bar and global import.
- Per-section state survives tab switches and dashboard visits for the session, and persists when
  the project is saved.

Section registry (`crates/app/src/sections/mod.rs`): one entry per section with id, title, group,
icon, description, spec link, status, dashboard card flag, view factory and key bindings. The
dashboard, router, shortcuts and harness read it, and a test keeps its ids in step with
`docs/roadmap.json` and checks every spec file exists. Adding a section is one registry entry plus
one module under `sections/` (and one `AppState` state field when it needs its own state). The five
hidden upstream tools are registered without a dashboard card and open by id.

Routing: `Route` is `Dashboard`, `Section(&Section)` or `Missing(id)`. Opening a section resolves
the id against the registry, remembers it as the last section and focuses the page so the shortcuts
work. The dashboard offers Resume for the last section and reopens it with its state intact. Escape
and Cmd/Ctrl+Left return to the dashboard from any section; a section can bind a deeper key context,
as the research grid does for Escape. An unknown id renders the missing-section empty state and a
planned section renders the shared "On the roadmap" empty state — neither can panic.

Section states are shared: every section page renders a `no-save-state` banner with the import
action until a save loads; the header distinguishes a full save from a partial one with warnings.
Long work goes through one shared `TaskRunner` (`src/task.rs`): the job runs on the background
executor, a foreground pump copies progress into the entity, `run-cancel` stops the job at its next
report, a cancelled run keeps the result already on screen, and a stored result whose inputs changed
is marked stale (badge, banner, disabled Apply) instead of being dropped. `ui/task.rs` renders the
progress card, the cancelled notice and the stale marker for every section.

## 4. Sections and milestones

One milestone per section, one issue per feature. The full feature lists, inputs, outputs, assets
and acceptance criteria are generated into [`docs/sections/`](sections/).

| Milestone | Section | Group | Issues |
| --- | --- | --- | --- |
| M0 | Foundation: dashboard, navigation, JSON import, assets | Core | 12 |
| M1 | Research Optimizer: full upstream parity | W7 | 13 |
| M2 | Combat Simulator | General | 7 |
| M3 | All Stats Dashboard | General | 4 |
| M4 | Damage Breakdown | Tools | 3 |
| M5 | Drop Rate Calculator | Tools | 2 |
| M6 | Coin Multiplier Calculator | Tools | 2 |
| M7 | Shadow Ban Check | General | 1 |
| M8 | Endgame Check | General | 1 |
| M9 | Statue Calculator | General | 3 |
| M10 | Fishing Calculator | W2 | 3 |
| M11 | Prisma Calculator | W2 | 2 |
| M12 | Killroy Calculator | W2 | 3 |
| M13 | Cog Board Optimizer | W3 | 4 |
| M14 | Worship Tower Defense | W3 | 4 |
| M15 | Ladle Optimizer | W4 | 4 |
| M16 | Cooking Mastery | W4 | 3 |
| M17 | Fountain Calculator | W5 | 4 |
| M18 | Jar Calculator | W5 | 4 |
| M19 | Sailing Calculator | W5 | 4 |
| M20 | Cglunko Calculator | W5 | 3 |
| M21 | Coral God Rank Planner | W5 | 3 |
| M22 | Monument Calculator | W5 | 4 |
| M23 | Opal Optimizer | W5 | 3 |
| M24 | Sneaking Optimizer | W6 | 4 |
| M25 | Farming Planner | W6 | 5 |
| M26 | Clamworks Calculator | W7 | 4 |
| M27 | Royal Guardian Calculator | W7 | 4 |
| M28 | Spelunking Simulator | W7 | 4 |
| M29 | Jelly Operator Optimizer | W7 | 5 |
| M30 | BoneJoePickle Calculator | Tools | 1 |

Sequencing: M0 first, then M1 (finish research), then the general tools and worlds in milestone
order. Sections that share combat or save systems (M2, M26, M27) can reuse the combat core; sections
that share a tree renderer reuse the formula-tree component.

## 5. Shared data and JSON import

### 5.1 Models

- `SaveData`: the decoded game save. One parser produces it; every section reads it read-only.
- `Account`: account multipliers, companions, currencies and cross-system bonuses derived from the
  save, with provenance for every field.
- `ProjectState`: versioned app state: active section, per-section config, imported save metadata,
  recent imports and UI preferences.
- `SectionState`: serializable state owned by one section (targets, filters, custom presets, replay
  seeds, plans).

### 5.2 Accepted JSON formats

1. **Raw decoded save** — `it.json` (`{ data, serverVars, companion }`) or flat `save.json`. This is
   the primary user-data import and the format the game writes.
2. **Optimizer state string** — upstream `ROv2:` + gzip + base64, exported by the research page.
3. **Project JSON** — `{ "format": "idleon-optimizer", "version": 1, "save": {...}, "sections": {...} }`.
4. **Section config JSON** — `{ "format": "idleon-optimizer-section", "section": "farming", "version": 1, ... }`.

Detection order: project JSON, section config, ROv2 string, raw save. A raw save is detected by
`Research`/`Lv0_*`/`gridLevels` fields; an unknown object is rejected with a field list.

### 5.3 Import pipeline

1. Read from file picker, paste box or drag-and-drop.
2. Parse and detect the format; never mutate state before validation passes.
3. Migrate older versions; unknown fields become warnings, not failures.
4. Build a preview: format, save name if present, characters, research level, detected sections and
   warnings.
5. Apply atomically: either the whole import succeeds or the previous state stays intact.
6. Persist the imported save metadata and add it to recent imports.

Errors name the failing field and the expected shape. Examples: `not valid JSON: expected value at
line 1 column 2`, `no save fields found: expected Research, Lv0_*, gridLevels`, `unsupported project
version 3 (this build reads 1)`.

### 5.4 Export

- Export project JSON with a schema version and per-section blocks.
- Copy the current save-derived state as an ROv2-compatible string for research.
- Exports contain no telemetry, paths or machine identifiers.

### 5.5 Privacy

Everything is local. No import is uploaded, no filename is sent anywhere, and logs contain only
counts and field names, not save values.

## 6. UI and assets

- Reuse Ely components for cards, tabs, tables, steppers, badges, banners, scroll areas and dialogs.
- Every calculator's breakdown renders through the shared formula tree: expandable nodes with label,
  inputs, multiplier, source and result, plus search highlights and a copy action. The research
  Rates pane is the first consumer.
- Match the upstream information architecture: same tab names and order, same panels, same column
  order in tables, same defaults.
- One shared page header (back, section title, world badge, save status, import) and one tab strip;
  a section owns its tab strip and the shell owns only the window chrome and status bar.
- Palette and spacing follow the upstream dark theme: `#1a1a2e` ground, `#16213e` surfaces,
  `#0f3460` borders, `#00d4ff` accent; world badges use the upstream per-world colours.
- Below 1360 px the fixed side rails stack under their main pane; data tables scroll inside a
  height that shrinks with the window, so the 1080×680 minimum stays usable.
- Images come from `assets/upstream/` through `manifest.json`: each entry has the path relative to
  the upstream assets folder, its byte count and sha256. `script/import-upstream-assets.mjs` copies
  the tree, writes the manifest and verifies it; `--dry-run` lists every file and the total size and
  `--verify` re-checks the hashes on disk. The app loads the first manifest it finds from
  `IDLEON_ASSETS_DIR`, the executable's folder, the working directory or the repository root. Every
  image reference has alt text and a labelled placeholder in the same box, so a missing install or
  a deleted file never breaks the layout.
- The import script replaces the committed `assets/upstream/` tree wholesale, so a refresh is one
  reviewed diff; the committed `README.md` carries the credit note.
- Every interactive control gets a stable id, accessible label, role and disabled state for the
  gpui-mcp bridge. Section roots use `section-<key>`.

## 7. Architecture

```
crates/core                         no GPUI, testable
  save/                             raw save, account and research decoding
  sections/                         one module per domain: research, combat, farming, ...
    <section>/{mod,model,sim,optimize}.rs
  shared/                           formulas, trees, number formatting, game data
crates/app                          GPUI + Ely
  sections/                         the registry and one module per section
    mod.rs                          id, title, group, icon, spec, status, factory, keys, cards
    research/                       the research section's state, tab strip and panes
  ui/                               shell (window, router, shortcuts), dashboard, section states
  ui/task.rs                        shared progress card, cancelled notice and stale marker
  task.rs                           TaskRunner: off-thread job, progress, cancel, stale inputs
  state.rs                          AppState, Route (`Dashboard`/`Section`/`Missing`) and state
assets/upstream                     committed upstream artwork, manifest-backed
docs/plan.md                        this document
docs/roadmap.json                   milestones and feature issues (source of truth)
docs/roadmap.md                     generated progress table
docs/sections/<section>.md          generated full specification per section
script/roadmap.mjs                  validate, render and sync GitHub milestones/issues
script/import-upstream-assets.mjs   copy upstream assets from a local checkout
script/extract-account-data.mjs     extract the account tables the multiplier tree reads
```

Rules: core never depends on GPUI; a section UI reads state only through its state methods; files
stay under about 500 lines and split by feature; one section cannot reach into another section's
private state.

## 8. Stack pins

The app, Ely and the automation bridge must share one GPUI build.

| Piece | Pin |
| --- | --- |
| GPUI | `gpui-pre = 0.3.9`, patched to the bridge's vendored tree |
| `gpui_platform` | `gpui-pre-platform = 0.3.9`, features `font-kit`, `wayland`, `x11` |
| Ely | `ely-gpui-component`, rev `28e5690` |
| gpui-mcp | submodule `vendor/gpui-mcp`, branch `gpui-pre-0.3.9`, commit `ac15850` |

Newer Ely pins `0.3.10`/`0.3.11`; the bridge patches up to `0.3.9`. These are the intersection. A
bump moves all three together and the smoke harness is the check.

## 9. Research and BoneJoePickle status

Research, shadow and endgame are implemented; research is still partial. Done: grid, shapes,
magnifiers, local sim, shape/mag/grid optimizers, 16 presets, supplements, save import, projection
chart, keyboard, canvas dragging and the shared searchable EXP formula tree. Missing: the five extra
tabs (Insight ROI, Observation Unlocks, Shape Optimization, Decision Tree, Minehead, Sushi
Station), target-aware simulation, strategy comparison, complete save-derived multipliers, state
export/import and undo. Details: [`docs/sections/research.md`](sections/research.md).

The hidden BoneJoePickle page is complete: DPS and max-time-to-kill inputs, the nine Death Note
minibosses, max pickles, effective HP and kill time, ported from `pickle-calc.html` with formula
tests and a section-harness interaction. Details: [`docs/sections/pickle.md`](sections/pickle.md).

The shadow-ban check is complete: it reads `OptLacc[26]` from the imported save and shows YES, NO or
UNKNOWN with the save status under the verdict. Details: [`docs/sections/shadow.md`](sections/shadow.md).

## 10. Verification

- **Formula parity:** Rust unit tests against fixtures evaluated from the upstream JS for each
  section; tolerance documented per formula.
- **Optimizer invariants:** never worse than input, respects caps/budgets, deterministic for a
  seed, cancellable.
- **UI parity:** harness opens every section from the dashboard, asserts the root and main controls,
  drives one core flow and saves a screenshot.
- **Import parity:** raw save, ROv2 string, project JSON and section config round-trip; malformed
  input leaves state untouched.
- **Gates:** `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, `node script/roadmap.mjs`, `node script/mcp-drive.mjs`,
  `node script/mcp-sections.mjs`.

## 11. Milestones and progress tracking

`docs/roadmap.json` is the source of truth. It defines one milestone per section and one issue per
feature, with acceptance criteria and explicit `dependsOn` edges.

Every feature issue contains:

- the feature description and its section contract (UI, inputs, outputs, assets);
- **Blocked by** and **Blocks** links resolved to issue numbers;
- upstream source files and the generated section spec;
- acceptance checklist and section verification steps;
- agent notes: port from upstream, keep core GPUI-free, reuse M0 components, close only when green.

The pinned [tracker issue](https://github.com/mastro993/idleon-optimizer/issues/124) is the entry
point: it lists every milestone, its dependencies and its status. Work starts at M0, then follows
each issue's blocked-by list. GitHub's native blocked-by relationships are set from the same graph,
so the issue UI shows the same dependencies as the body. A milestone can only be marked `done` when
all its feature issues are closed and its verification passes.

```console
node script/roadmap.mjs                 # validate the dependency graph and print the summary
node script/roadmap.mjs --write         # regenerate docs/roadmap.md and docs/sections/*.md
node script/roadmap.mjs --apply --repo owner/name --dry-run
node script/roadmap.mjs --apply --repo owner/name
```

The GitHub sync is idempotent: milestones match by title, issues by their feature-id prefix. Existing
issues are updated in place, so dependency links, contracts and acceptance criteria stay current.
Each issue carries `area:<section>`, `type:feature` and `roadmap` labels, and its milestone. Close an
issue to track progress; when all issues in a section are closed, mark the milestone `done` in
`roadmap.json` and regenerate the docs.

## 12. Risks

- **Licensing:** the upstream licence forbids reuse and redistribution of code; the game artwork is
  separate and free to use. Mitigated by keeping code as ports with source references and by the
  committed credit note.
- **Scope:** 30 sections is a large surface. Mitigated by one milestone per section, shared
  primitives and a strict definition of done.
- **Formula drift:** upstream JS changes; mitigated by committed fixtures, source references and
  parity tests.
- **GPUI churn:** three pinned projects; mitigated by exact pins and the smoke harness.
- **Asset weight:** 42 MB of PNGs, mostly combat-sim, now in the repository; a refresh is a single
  generated diff and the manifest keeps references honest.
- **Performance:** large simulations (combat, tower defense, decision tree) must run off the UI
  thread and stay cancellable.
