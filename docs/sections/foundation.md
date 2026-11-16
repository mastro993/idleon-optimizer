# Foundation: dashboard, navigation, JSON import, assets

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M0`
- Section key: `foundation`
- Group: `core`
- Status: `in-progress`
- Depends on: nothing

Turn the research-only window into a sectioned desktop suite: a dashboard that opens every calculator, one shared save/state model, JSON import/export, the game asset pipeline and the per-section verification harness.

## UI

- Home, the landing page, is a searchable card grid grouped by General and W2-W7, with each section's title, icon, world badge and one-line description.
- A header across the window holds the sidebar toggle, the save status, and Export and Import; the active sidebar item marks the open section.
- A sidebar lists Home, then every section under its world (General, World 2-7) and the five hidden calculators under Tools; any item opens its section in one click, and the sidebar folds to an icon rail that survives a restart.
- Section pages keep their own tab strip; the shell owns only the header, sidebar and status bar.
- Import dialog accepts a file, a paste or drag-and-drop; it previews the detected format before applying.

## Inputs

- Raw decoded save (it.json or save.json)
- App project JSON
- Section JSON config

## Outputs

- Loaded account and simulation state
- Section configuration
- Import warnings and errors
- Persisted last session

## Assets

assets/game is committed to the repository; the manifest records per-file hashes.

## Feature issues

### FND-01 — Dashboard card grid

Landing dashboard with a searchable, grouped card grid: icon, title, world badge, description and stable card ids.

Depends on: None; this is an entry point.

Acceptance:

- All 25 dashboard cards are present and grouped by world.
- Search filters by title, description and world; Escape clears it.
- Every card exposes a stable id (card-<section>) and opens its section.

### FND-02 — Section registry and routing

A single registry declares every section: id, title, group, icon, spec link, status and factory. The shell routes to a section, keeps per-section state alive and restores the last opened section.

Depends on: FND-01 Dashboard card grid

Acceptance:

- Adding a section touches one registry entry and one module.
- Unknown or disabled sections render a clear empty state, not a panic.
- Back navigation and keyboard shortcuts work from every section.

### FND-03 — Shared save JSON import

One import path for raw decoded saves used by every section, replacing the research-only paste panel. Supports file, paste and drag-and-drop.

Depends on: FND-02 Section registry and routing

Acceptance:

- Raw it.json, save.json and the existing fixture import with the same values as today.
- Validation reports the detected format, research level, character count and warnings before applying.
- A failed import leaves the current state untouched.

### FND-04 — Project and section JSON import/export

Export the whole app state as versioned JSON and import it back. Also accept per-section config JSON.

Depends on: FND-03 Shared save JSON import

Acceptance:

- Round-trip export then import reproduces every section's visible state.
- Version and section ids are validated; unknown fields warn, not fail.

### FND-05 — Import preview, migration and errors

A single import pipeline: detect format, parse, migrate to the current schema, show a preview, apply atomically, and keep a warning list.

Depends on: FND-03 Shared save JSON import, FND-04 Project and section JSON import/export

Acceptance:

- Malformed JSON, wrong format and unsupported versions each get a specific message.
- Warnings name the field and the fallback used.
- Applying is atomic and logs what changed.

### FND-06 — Game asset pipeline

Committed assets/game tree with a manifest that the app resolves images through. The images are game assets, free to use; the credit lives in assets/game/README.md.

Depends on: FND-02 Section registry and routing

Acceptance:

- assets/game holds every referenced file.
- The manifest records per-file hashes; the app resolves images through the manifest.
- Missing assets render a labelled placeholder, never a broken layout.

### FND-07 — Session persistence and recent imports

Persist the active section, imported save metadata and section configs across launches in a local app-data file.

Depends on: FND-04 Project and section JSON import/export

Acceptance:

- Relaunch restores the last section and imported state.
- Recent imports are listed with time and format.
- A corrupt state file is backed up and the app starts clean.

### FND-08 — Shared layout, theme and responsive rules

Ely-based shell, cards, tabs, tables and form controls shared by all sections, with one dark palette and spacing scale.

Depends on: FND-01 Dashboard card grid

Acceptance:

- All sections use the shared page header, tab strip, card and table primitives.
- Minimum window size keeps tables scrollable and cards readable.
- No section reimplements a component that exists in Ely.

### FND-09 — Empty, loading, error and stale states

Consistent states for no save, partial save, missing data, long-running work, cancelled work and stale results.

Depends on: FND-08 Shared layout, theme and responsive rules

Acceptance:

- Every section renders a helpful no-save state.
- Long tasks expose progress and cancel; cancelling restores the previous result.
- Stale results are visibly marked after inputs change.

### FND-10 — Per-section verification harness

Extend the gpui-mcp harness so each section has a smoke path, stable ids and a screenshot baseline.

Depends on: FND-02 Section registry and routing, FND-08 Shared layout, theme and responsive rules

Acceptance:

- script/mcp-sections.mjs opens every dashboard card and asserts its root node.
- Each section has at least one interaction check and one screenshot.
- Lane isolation still works when sections run in parallel.

### FND-11 — Shared formula tree and breakdown component

One expandable formula-tree/breakdown component used by every calculator: nodes show label, input, multiplier, source and result, with search and copy.

Depends on: FND-08 Shared layout, theme and responsive rules

Acceptance:

- Every breakdown section renders through the shared component.
- Search highlights matching nodes.
- Node values and sums match the section totals.

### FND-12 — Background task runner with progress and cancel

Shared runner for simulations and optimizers: off-thread work, progress reporting, cancellation and a stale-result marker.

Depends on: FND-09 Empty, loading, error and stale states

Acceptance:

- Long tasks never block the UI thread.
- Cancel restores the previous result.
- Results are marked stale when their inputs change.

## Verification

- cargo test, clippy and fmt stay clean.
- Dashboard opens all 30 sections in the harness.
- Import round-trip covers raw save and project JSON.
