# Foundation: dashboard, navigation, JSON import, assets

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M0`
- Section key: `foundation`
- Group: `core`
- Status: `in-progress`

Turn the research-only window into a sectioned desktop suite: a dashboard that opens every calculator, one shared save/state model, JSON import/export, the upstream asset pipeline and the per-section verification harness.

## Upstream sources

- index.html
- js/ui/site-nav.js
- js/ui/save-loader.js
- js/ui/state-io.js
- js/state.js

## UI

- Dashboard replaces the three-tab shell as the landing page: searchable card grid grouped by General, W2-W7 and Tools, with the same titles, icons, world badges and one-line descriptions as upstream index.html.
- Each card opens a section route with a shared header: back to dashboard, section title, world badge, save status and an import button.
- Section pages keep their upstream tab strip; the app shell only owns the window chrome and status bar.
- Import dialog accepts a file, a paste or drag-and-drop; it previews the detected format before applying.

## Inputs

- Raw decoded save (it.json or save.json)
- Optimizer state string (ROv2: gzip+base64)
- App project JSON
- Section JSON config

## Outputs

- Loaded account and simulation state
- Section configuration
- Import warnings and errors
- Persisted last session

## Assets

assets/upstream is generated from a local upstream checkout; nothing is committed until permission is confirmed.

## Feature issues

### FND-01 — Dashboard card grid

Landing dashboard with a searchable, grouped card grid mirroring upstream index.html: icon, title, world badge, description and stable card ids.

Acceptance:

- All 25 upstream dashboard cards are present and grouped exactly like index.html.
- Search filters by title, description and world; Escape clears it.
- Every card exposes a stable id (card-<section>) and opens its section.

### FND-02 — Section registry and routing

A single registry declares every section: id, title, group, icon, spec link, status and factory. The shell routes to a section, keeps per-section state alive and restores the last opened section.

Acceptance:

- Adding a section touches one registry entry and one module.
- Unknown or disabled sections render a clear empty state, not a panic.
- Back navigation and keyboard shortcuts work from every section.

### FND-03 — Shared save JSON import

One import path for raw decoded saves used by every section, replacing the research-only paste panel. Supports file, paste and drag-and-drop.

Acceptance:

- Raw it.json, save.json and the existing fixture import with the same values as today.
- Validation reports the detected format, research level, character count and warnings before applying.
- A failed import leaves the current state untouched.

### FND-04 — Project and section JSON import/export

Export the whole app state as versioned JSON and import it back. Also accept the upstream ROv2 state string and per-section config JSON.

Acceptance:

- Round-trip export then import reproduces every section's visible state.
- ROv2 state strings are decompressed and migrated.
- Version and section ids are validated; unknown fields warn, not fail.

### FND-05 — Import preview, migration and errors

A single import pipeline: detect format, parse, migrate to the current schema, show a preview, apply atomically, and keep a warning list.

Acceptance:

- Malformed JSON, wrong format and unsupported versions each get a specific message.
- Warnings name the field and the fallback used.
- Applying is atomic and logs what changed.

### FND-06 — Upstream asset pipeline

Script that copies the upstream assets tree from a local checkout into gitignored assets/upstream, writes a manifest and verifies referenced files. Requires explicit permission before any asset is committed or distributed.

Acceptance:

- script/import-upstream-assets.mjs --from <checkout> --dry-run lists every file and total size.
- Applying writes assets/upstream plus manifest with hashes; the app resolves images through the manifest.
- Missing assets render a labelled placeholder, never a broken layout.

### FND-07 — Session persistence and recent imports

Persist the active section, imported save metadata and section configs across launches in a local app-data file.

Acceptance:

- Relaunch restores the last section and imported state.
- Recent imports are listed with time and format.
- A corrupt state file is backed up and the app starts clean.

### FND-08 — Shared layout, theme and responsive rules

Ely-based shell, cards, tabs, tables and form controls shared by all sections; dark palette and spacing match upstream.

Acceptance:

- All sections use the shared page header, tab strip, card and table primitives.
- Minimum window size keeps tables scrollable and cards readable.
- No section reimplements a component that exists in Ely.

### FND-09 — Empty, loading, error and stale states

Consistent states for no save, partial save, missing data, long-running work, cancelled work and stale results.

Acceptance:

- Every section renders a helpful no-save state.
- Long tasks expose progress and cancel; cancelling restores the previous result.
- Stale results are visibly marked after inputs change.

### FND-10 — Per-section verification harness

Extend the gpui-mcp harness so each section has a smoke path, stable ids and a screenshot baseline.

Acceptance:

- script/mcp-sections.mjs opens every dashboard card and asserts its root node.
- Each section has at least one interaction check and one screenshot.
- Lane isolation still works when sections run in parallel.

## Verification

- cargo test, clippy and fmt stay clean.
- Dashboard opens all 30 sections in the harness.
- Import round-trip covers raw save, ROv2 state and project JSON.
