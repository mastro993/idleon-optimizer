# Plan: Idleon Optimizer, a native desktop suite

Status: **foundation in progress, research (including the Minehead and Sushi Station tabs), statue, jar, farming, ladle,
the Prisma calculator, Cooking Mastery, Fountain, the Opal Optimizer's current state, the
Royal Guardian overview, outpost map, resource nodes, currencies, upgrades, breakdowns and forecast,
the Coral God Rank planner, the Spelunking controls, depth estimates, expedition run and Amber
policy, the Sneaking loadout, inventory, formula breakdown, charm optimizer and floor × mastery
workspaces, the combat simulator's loadout editor and mechanics overrides, the cog board optimizer,
the Clamworks optimizer, the Jelly Operator overview, layout editor, operation simulator and section
planner, the drop-rate inputs, dungeon total and formula tree, the Coin Multiplier inputs, total
and formula tree, the
Damage Breakdown context and summary shell partial, sailing, Fishing, Killroy,
BoneJoePickle, Cglunko, Clamworks and Fountain done, Monument, the All Stats context, breakdowns,
explorer and input ledger built.** The live breakdown is
[`docs/roadmap.md`](roadmap.md); the machine-readable source is [`docs/roadmap.json`](roadmap.json);
each section's full specification is under [`docs/sections/`](sections/).

## 1. Product goal

A native Rust + GPUI desktop suite of calculators and optimizers for Legends of Idleon, not just the
research grid.

- **Dashboard first.** The app opens on a searchable card grid, grouped by General and Worlds 2-7.
  Every card opens one section.
- **Every section.** The 25 dashboard cards plus the five hidden tools (`damage`, `drop-rate`,
  `coin-multi`, `pickle`, `stats-all`) are in scope.
- **All functionality per section.** Every tab, input, table, simulator, optimizer, replay and
  breakdown is specified as a feature issue under that section's milestone.
- **Game artwork.** Icons and sprites come from the game's own art; a missing image falls back to a
  labelled placeholder.
- **Local only.** No accounts, cloud sync, telemetry or network calls. Saves and exports stay on
  disk.

Definition of done for a section: every feature issue closed, its formula tests pass, the section
opens from the dashboard, the harness drives its main flow, assets resolve, and no unexplained
import warning remains.

## 2. Assets and distribution

The images are game assets from Legends of Idleon, committed under `assets/game/` with credit in
`assets/game/README.md`. The app resolves them through the manifest and falls back to a labelled
placeholder.

**Distribution decision (2026-11-14).** The app is distributed through GitHub only: unsigned
builds for macOS, Linux and Windows are attached to this repository's GitHub Releases, and there
is no other channel. A release bundles the game artwork under `assets/game/` with its credit
note (`assets/game/README.md`). How releases are built and cut: [`release.md`](release.md).

## 3. Dashboard and navigation

> **Redesign in progress:** [`specs/app-shell.md`](specs/app-shell.md). The header, the sidebar and
> the workspace breakpoint (slice 1) are built and described below; page history (slice 2),
> per-section run activity (slice 3) and game-art icons (slice 4) are still planned. The grid stays as
> Home ([ADR 0001](adr/0001-sidebar-is-the-primary-navigation-and-home-keeps-the-grid.md)).

Shell (`ui/shell.rs`):

- Header across the window (`ui/header.rs`): sidebar toggle, save status, Export and Import. The
  open place is named by its active sidebar item, not the header.
- Sidebar (`ui/sidebar.rs`, Ely `Sidebar`): Home first, then one `NavGroup` per world (General,
  World 2-7) and Tools, which lists every section without a dashboard card. Items are the app's
  `SidebarItem` ([ADR 0003](adr/0003-sidebar-items-are-built-in-the-app-from-ely-parts.md)) with
  ids `nav-home` and `nav-<section>`; the open place's item is active and scrolled into view, and
  clicking it does nothing. The sidebar scrolls with an edge shade (Ely `ScrollShadow`) and no
  scrollbar, which would cover the rail's icons. The toggle folds the sidebar from 240 px to the 56 px rail (icons, title
  in a tooltip) and back; the fold is saved in the session file as
  `"shell": {"sidebar": "expanded" | "rail"}` and a session without it opens expanded.
- The workspace beside the sidebar holds the place and, on a section without a save, the
  `no-save-state` banner. There is no status bar: the last run or import message
  (`AppState::status`) is a zero-size labelled node, `status-status`, for screen readers and the
  harnesses.

Home (the landing page):

- Search field above the grid.
- Card grid grouped General, W2, W3, W4, W5, W6, W7, Tools; each card has its title, emoji/icon,
  world badge and description.
- Each card has `id="card-<section>"`, title, badge and description as accessible text.
- Search matches title, description and group; Escape clears; Enter opens the first match.
- Last opened section is remembered and offered as Resume.
- Import accepts a raw save, a project JSON or a section config; it previews the detected format and
  warnings before applying. `--import <file>` (or `IDLEON_OPTIMIZER_IMPORT`) applies one at launch
  the same way, after the session restores and before `--section` opens.

Section pages:

- The active sidebar item marks the section; the sidebar opens any other section in one click.
- The section owns its tab strip and content; the shell owns only window chrome, status
  bar and global import.
- Per-section state survives tab switches and dashboard visits for the session, and persists across
  launches in the local session file with the imported save and the active section.

Section registry (`crates/app/src/sections/mod.rs`): one entry per section with id, title, group,
icon, description, spec link, status, dashboard card flag, view factory and key bindings. The
dashboard, router, shortcuts and harness read it, and a test keeps its ids in step with
`docs/roadmap.json` and checks every spec file exists. Adding a section is one registry entry plus
one module under `sections/` that owns its state, import step and config (see
[`docs/section-state.md`](section-state.md)); the shared core gains one `AppState` field and one line
each in `load_imported`, `configs`, `Config`, `decode` and `assign`, and the harness one interaction.
The five hidden tools are registered without a dashboard card and open by id.

Routing: `Route` is `Dashboard`, `Section(&Section)` or `Missing(id)`. Opening a section resolves
the id against the registry, remembers it as the last section and focuses the page so the shortcuts
work. The dashboard offers Resume for the last section and reopens it with its state intact. Escape
and Cmd/Ctrl+Left return to the dashboard from any section; a section can bind a deeper key context,
as the research grid does for Escape. An unknown id renders the missing-section empty state and a
planned section renders the shared "On the roadmap" empty state — neither can panic.

Section states are shared: every section page renders a `no-save-state` banner with the import
action until a save loads; the header distinguishes a full save from a partial one with warnings.
Long work goes through one shared `TaskRunner` (`crates/ui/src/runner.rs`): the job runs on the
background executor, a foreground pump copies progress into the entity, `run-cancel` stops the job at
its next report, a cancelled run keeps the result already on screen, and a stored result whose inputs
changed is marked stale (badge, banner, disabled Apply) instead of being dropped. Importing a save
discards the runs it invalidates: whatever a discarded run still returns is dropped. A job that panics settles like a
cancelled run, so the runner is usable again. `crates/ui/src/run_chrome.rs` renders the
progress card, the cancelled notice and the stale marker for every section.

## 4. Sections and milestones

One milestone per section, one issue per feature. The full feature lists, inputs, outputs, assets
and acceptance criteria are generated into [`docs/sections/`](sections/).

| Milestone | Section | Group | Issues |
| --- | --- | --- | --- |
| M0 | Foundation: dashboard, navigation, JSON import, assets | Core | 12 |
| M1 | Research Optimizer | W7 | 13 |
| M2 | Combat Simulator | General | 7 |
| M3 | All Stats Dashboard | General | 4 |
| M4 | Damage Breakdown | Tools | 3 |
| M5 | Drop Rate Calculator | Tools | 2 |
| M6 | Coin Multiplier Calculator | Tools | 2 |
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

The Coin Multiplier Calculator (M6) is complete: the character selector, the map/target/HP/damage
cards, the overkill tier and all 23 multiplicative groups are built. The formula breakdown renders
through the shared formula tree: the total's 23 group factors, each carrying the source that owns it
and the additive terms it sums back to. Every group multiplies back to the total card, and the shared
search reaches group names, term names and sources. The Cash Bubbles group reads the shared stat
engine's total STR/AGI/WIS, including its equipment affixes, alchemy slab and tome bubbles, the
golden-food multiplier chain, all-stat percentage and quest-scaled LUK, so the totals are fully
justified instead of lower bounds. The Divinity and Crop Scientist group resolves the minor divinity
and Crop Scientist bonuses, including the Big P bubble and the mainframe/Grimoire/exotic/Vault
multiplier. The Dustwalker and Cooking Level group resolves through the talent system (the talent's
own level plus the all-talent bonus levels) multiplied by the base-10 log of the cooking level.
Other Additive Sources sums the Monster Cash vial, talents 657/22/643/644, card and golden-food
bonuses, the flurbo shop, box rewards, the guild map curve, the vault kill-count tiers and the
remaining account effects.

## 5. Shared data and JSON import

### 5.1 Models

- `SaveData`: the decoded game save. One parser produces it; every section reads it read-only.
- `Account`: account multipliers, companions, currencies and cross-system bonuses derived from the
  save, with provenance for every field.
- `ProjectState`: versioned app state: active section, per-section config, imported save metadata,
  recent imports and UI preferences. The app writes it to the platform app-data directory as
  `session.json` (override with `--state-dir` or `IDLEON_OPTIMIZER_STATE_DIR`): the project document
  plus the last applied imports, rewritten only when its content changed. A corrupt file is renamed to
  `session.corrupt-<timestamp>.json` and the app starts clean.
- `SectionState`: serializable state owned by one section (targets, filters, custom presets, replay
  seeds, plans).

### 5.2 Accepted JSON formats

1. **Raw decoded save** — `it.json` (`{ data, serverVars, companion }`) or flat `save.json`. This is
   the primary user-data import and the format the game writes.
2. **Project JSON** — `{ "format": "idleon-optimizer-project", "version": 1, "save": {...}, "sections": {...} }`.
   The optional `research` and `supplements` blocks replace the live research state and
   multipliers; a block the document omits leaves the embedded save's (or, with no save, the
   window's) value alone. The multipliers the embedded save decodes stay the baseline "Reset
   supplements" returns to.
3. **Section config JSON** — `{ "format": "idleon-optimizer-section", "section": "farming", "version": 1, ... }`.

Detection order: project JSON, section config, raw save. A raw save is detected by
`Research`/`Lv0_*`/`gridLevels` fields; an object with a `format` marker this build does not read is
rejected by name, and an object with no marker and no save field is rejected with the field list it
expected. `crates/import/src/pipeline.rs` is the one pipeline every accepted format goes through.

### 5.3 Import pipeline

1. Read from file picker, paste box or drag-and-drop. Text or a file over 64 MB is refused before
   parsing. While decoding, a save key suffix names at most 128 characters and a sparse row at most
   100,000 cells (`idleon_save::MAX_CHARACTERS`, `MAX_ROW_CELLS`); a larger index is ignored rather
   than allocated. A non-finite number (`NaN`, `inf`) reads as missing.
2. Parse and detect the format; never mutate state before validation passes.
3. Migrate older versions; unknown fields become warnings, not failures.
4. Build a preview: format, save name if present, characters, research level, detected sections and
   warnings.
5. Decode everything the document needs while previewing (the embedded save and each section
   config), then apply atomically: either the whole import succeeds or the previous state stays
   intact. A config for a section that stores no config (or one this build does not declare) is
   named in the preview's warnings instead of applying silently. Log every applied field change as `field: old → new`, summarise it in the status message and
   keep the list in the dialog.
6. Persist the imported save metadata and add it to recent imports; the session file keeps the
   whole project so the next launch restores the section, save and section configs together.

Errors name the failing field and the expected shape. Examples: `not valid JSON: expected value at
line 1 column 2`, `unknown format "some-other-tool"; this build reads "idleon-optimizer-project",
"idleon-optimizer-section" or a decoded save`, `unsupported version: project version 3; this build
reads 1`, `no save fields found: expected Research, Lv0_*, gridLevels`.

### 5.4 Export

- Export project JSON with a schema version and per-section blocks.
- Exports contain no telemetry, paths or machine identifiers.

### 5.5 Privacy

Everything is local. No import is uploaded, no filename is sent anywhere, and the local log
(`idleon-optimizer.log` beside the session file: warnings, errors and panics) contains only counts,
ids and field names, not save values. The automation bridge the harnesses use is the `automation`
Cargo feature of the app crate: on by default for development, absent from a
`--no-default-features` build, and reachable only by the same user's processes when present. If it
cannot start, the window opens without it.

## 6. UI and assets

- Reuse Ely components for cards, tabs, tables, steppers, badges, banners, scroll areas and dialogs.
- Every calculator's breakdown renders through the shared formula tree: expandable nodes with label,
  inputs, multiplier, source and result, plus search highlights and a copy action. The research
  Rates pane is the first consumer. Node ids are scoped by the tree's own id, so a page can show
  several trees (the All Stats breakdown card and its formula card, for instance) without the
  accessibility tree collapsing their rows onto one another.
- One shared header (sidebar toggle, save status, Export, Import), one
  sidebar and one tab strip; a section owns its tab strip and the shell owns the header and sidebar.
- Palette and spacing follow one dark theme: `#1a1a2e` ground, `#16213e` surfaces, `#0f3460`
  borders, `#00d4ff` accent; world badges use per-world colours.
- When the workspace (the window minus the sidebar) is narrower than 1180 px, the fixed side rails
  stack under their main pane; data tables scroll inside a
  height that shrinks with the window, so the 1080×680 minimum stays usable.
- Images come from `assets/game/` through `manifest.json`: each entry has the path relative to
  that folder, its byte count and sha256. The app loads the first manifest it finds from
  `IDLEON_ASSETS_DIR`, the executable's folder, the working directory or the repository root. Every
  image reference has alt text and a labelled placeholder in the same box, so a missing install or
  a deleted file never breaks the layout.
- Every interactive control gets a stable id, accessible label, role and disabled state for the
  gpui-mcp bridge. Section roots use `section-<key>`.

## 7. Architecture

```
crates/common                       formula trees, game curve evaluator, run Control/Progress
crates/save                         raw save document: codec, JSON helpers, characters, companions
crates/research                     research grid model, EXP/insight maths, grid/shape/mag optimizers
crates/stats-data                   static map, monster, class and equipment-set tables
crates/account                      account-wide bonus decoder (`Decoder`): one view of every system
crates/stats-engine                 shared stat engine: total STR/AGI/WIS/LUK and the damage layers
crates/minehead                     Depth Charge upgrades, floor simulation and strategy searches;
                                    reads the account decoder, so it sits beside `stats`
crates/sushi                        Sushi Station formulas, save inputs and upgrade ranking; reads
                                    the account decoder and the Ring of Glory table
crates/<domain>                     one crate per calculator, no GPUI, testable in isolation:
                                    cglunko, clamworks, cog, coin-multi, combat, cooking-mastery,
                                    coral, damage, drop-rate, farming, fishing, fountain,
                                    jar, jelly, killroy, ladle, monument, opal, pickle, prisma,
                                    royal-guardian, sailing, sneaking, spelunking, stats, statue,
                                    worship; each owns its `data/*.json` and `tests/`
crates/import                       save -> every calculator's state (`import`), text `pipeline`
crates/project                      project document format and migration
crates/ui                           GPUI building blocks: TaskRunner, run chrome, formula tree,
                                    number formats
crates/app                          GPUI + Ely binary
  sections/                         the registry and one module per section
    mod.rs                          id, title, group, icon, spec, status, factory, keys, cards
    <section>/state.rs              the section's own state, import step and project config
    <section>/actions.rs            what the section's controls do to `AppState`
  ui/                               shell (window, router, shortcuts), dashboard, section states
  state/                            AppState, Route, run/subscription helpers, import/export
assets/game                         committed game artwork, manifest-backed
docs/plan.md                        this document
docs/roadmap.json                   milestones and feature issues (source of truth)
docs/roadmap.md                     generated progress table
docs/sections/<section>.md          generated full specification per section
script/roadmap.mjs                  validate, render and sync GitHub milestones/issues
```

Rules: core never depends on GPUI; a section UI reads state only through its state methods; files
stay under about 500 lines and split by feature; one section cannot reach into another section's
private state; a section owns its state, its import step and its config in its own module (see
[`docs/section-state.md`](section-state.md)).

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

## 9. Section status

Every section has a built page; [`docs/roadmap.md`](roadmap.md) lists which are complete.
Research is still partial. Done: the dashboard (summary, grid and observation previews,
EXP/AFK/insight breakdowns and the grid-point split), grid, shapes, magnifiers, local sim,
shape/mag/grid optimizers, 16 presets, supplements, save import, projection chart, keyboard, canvas
dragging, the manual editing surface (grid levels, shape add/drag/rotate/remove with a canvas
rotation handle, per-observation magnifier assignment, and undo/redo over every edit), the
shared searchable EXP formula tree, target validation, the three run toggles (assume
observations unlocked, extended insight lookahead and the daily tournament +12h) and the
target-aware engine: the run simulates to the target, spends each level's grid points and re-fits
magnifiers and shapes on the level-up events, the saved and optimized layouts are ranked by the
target they reach, and the grid spend is scored with a forward simulation. The Run tab's result
reports the best and worst tested layouts, cuts the projection into a timeline of level, insight and
tournament phases, charts the same samples' EXP/hour and lists each phase as an expandable action
that marks its segment. The Insight ROI tab prices each observation's next insight levels against the
research EXP the grind gives up, with the concentrated grind layout, the re-optimized post-grind
layout, the recoup and break-even columns, four sort modes and the hide-free filter. The
Observation Unlocks tab ranks the observations the save has not found by the EXP/hour each adds per
expected day of rolls, with the roll bounds, the failed-roll pity, the usability filter and the name
search, and it reports the roll fields a save does not carry instead of assuming them.
The dashboard's AFK rate reports Gambit Milestone 15 as not modelled; its Hole measurement chain
lands with the Minehead work. The Minehead and Sushi Station tabs are built, each as a subtab strip
over its own crate. The Insight ROI tab is described in [`docs/insight-roi.md`](insight-roi.md) and
the Observation Unlocks tab in [`docs/observation-unlocks.md`](observation-unlocks.md). The Decision
Tree tab opens a run or the loaded save as a root node, steps it to the next research or insight
level-up, branches, re-simulates, deletes, compares branches by rate, level and time, edits each
node's grid, shapes and magnifiers, and runs Auto Insight, which re-fits the monocles at every
insight level-up ([`docs/decision-tree.md`](decision-tree.md)). Missing: complete save-derived
multipliers.
Details: [`docs/sections/research.md`](sections/research.md).

The Fishing section implements FI-01, FI-02 and FI-03: the character/toolkit/location toolbar, an
every-location table that switches the selected pool, the selected pool's fish split,
real-versus-panel EXP, the shared formula tree, the ranking of all 225 lure/line
combinations for the selected pool and depth by either Fishing EXP or fish quantity, with
the selected and saved rows marked, a `Use` action, the tie explanation and the
best-combination KPI, and the Trench depth projection: the saved depth's fish requirement
with its jar and Bolaia reductions, the efficiency requirements each projected depth
raises, the selected and fish-maximizing lure + line hours per depth with cumulative
totals, and the other characters' fish per hour, which sum into the team ETA. The save
supplies the character, rod, saved toolkit, spot and AFK target plus the cavern rows the Trench
reads (`Holes`, `Spelunk`), while the account skill values (efficiency, EXP multiplier, AFK rate,
Multi-Fish, speed sources, prowess and equipment depths) are entered in the account card until the stats dashboard
decodes them, and the ranking and the depth search hold efficiency and the EXP multiplier
fixed across combinations, because recomputing them per toolkit needs the skill
systems. Details: [`docs/sections/fishing.md`](sections/fishing.md).

The Clamworks section implements CW-01 to CW-04: the overview with the pearl-value
breakdown, every pearl upgrade, the upgrade priority ranking, the AFK claim with its drop
distribution, payout and `+1` comparison, the budgeted upgrade plan (a greedy pass over the same
ranking, priced at the levels each purchase reaches and run through the shared task runner, with
progress, cancel, a stale marker and an applied projection), the promotion tracker and all twelve
worker-class rewards, plus save decoding for pearl balance, upgrade levels, unlock progress,
promotion and Scurge. Multi-Scalping takes a manual multikill total, and the claim takes the
character's kills per hour and drop rate, because the combat simulator's auto-mode player is not
built. The claim does not carry the reroll-for-pure-pearls policy, which needs the talent and guild
bonuses the stats context resolves.
CW-04 adds the Scurge tab: in `idleon_clamworks::scurge` one Clamworks kill banks
`(1 + RECKONING) × (1 + EVEN BETTER / 100) × (1 + SELF AFFLICTION / 100) ×
(1 + SWARMING × other players / 100)` Scurge EXP, the Self Affliction panel tests one level from
zero to the selected limit, and the upgrade panel water-fills the unspent points across the
unlocked RECKONING, EVEN BETTER and SWARMING. The tab renders both panels, names the best allocation
an hour, lists the nine Scurge upgrades with their unlock levels, and refuses a save whose Scurge
menu is not bought or whose saved upgrades overspend its level, while the import names every
missing `Menagerie` row. The rates multiply the row's EXP per kill by the character's
kills-per-hour stand-in, because the shared simulator does not run Clam Work.
Details: [`docs/sections/clamworks.md`](sections/clamworks.md).

The Killroy section implements KL-01 to KL-03: the weekly forecast with every character's three
encounters and records, and the `+1` upgrade comparison with the Baseline and Upgrade tabs, the
upgrade list, the selection queue preview, deterministic trials and the best-option highlight. The
player's damage belongs to the combat simulator, so a trial takes the character's kills per second
as an input because the simulator's auto-mode player is not built; the arrival schedule, respawn
bonus, `100 + Time` duration and the per-kill skull, talent-point, credit and pearl rolls are the
game's own formulas. Details: [`docs/sections/killroy.md`](sections/killroy.md).

The hidden BoneJoePickle page is complete: DPS and max-time-to-kill inputs, the nine Death Note
minibosses, max pickles, effective HP and kill time, with formula tests and a section-harness
interaction. Details: [`docs/sections/pickle.md`](sections/pickle.md).

The Combat Simulator is built through CS-07. The loadout editor (CS-01) has the character picker,
the 16 armor and 7 tool slots with their saved `EMm` upgrade deltas, the talent table from
`SL_<n>`/`SM_<n>` and named loadout presets in the project JSON; the item catalog is the game's item
table, every edit recomputes the gear totals, and unknown or invalid items are flagged. The
mechanics overrides (CS-02) carry the combat toggle catalog. The encounter run (CS-03) replays the
game's 10 ms tick and its spawn/respawn/kill handlers; the comparison (CS-04) runs it per setup with
deltas and a JSON export; the results (CS-05) cover Killroy practice, obols, cards, kills by monster
and the kills timeline. CS-06 adds combat-driven progress and smelting — the saved furnace advances
on the run's own clock and is reported by bar, with the Scurge, Coral Kid and Omega credits and
their notices — and The Den as an own projection of the cavern's hound waves at the run's kill rate.
The two terms this build cannot decode (the shiny speed's star-sign term and the per-kill
forge/pet/alchemy talent grants) are named as warnings instead of guessed. CS-07 adds the replays:
the encounter run records its own frames through a delta codec, the Replay tab draws them with the
map, platform and monster art resolved through the asset manifest (a labelled placeholder when a
file is missing) and offers play/pause, scrub, the playback speeds and Den wave jumps, and a Killroy
practice captures as a version-1 replay document that re-opens and re-runs it. Details:
[`docs/sections/combat-sim.md`](sections/combat-sim.md).

The statue section has its table, its estimates and its batch controls (ST-01 to ST-03). Done: the
32-row table, the saved level and EXP progress, the next-level requirement including the
statue-1/2/3 rounding gates, per-row availability, the character selector and search/sort; the
STARTUE EXP bubble, Skinny 0 Cal vial, Onyx/Zenith tier and Royal Guardian Flair behind a per-row
target, the statues that target needs under a guaranteed or average deposit estimate, and the time
from a raw statues/hour rate with its source shown; the Set targets entry (costed for every statue
first and rejected as a whole with a message), Next level for all, a one-step undo of the
batch action, and the farming breakdown behind one statue's row through the shared formula tree.
Missing: a "Measure statue farming" panel, which needs a character kill rate the combat simulator's
stand-in player cannot give (its auto-mode player is not built), so rates are manual entries today.
Details: [`docs/sections/statue.md`](sections/statue.md).

The shared stat engine is `crates/stats-engine` (`idleon-stats-engine`). It owns the total-stat
computation for STR, AGI, WIS and LUK — the equipment, gallery, obol, stamp, star-sign, sigil,
family, card, meal, guild, talent, bubble and account pools — and the damage descriptor's three
layers on top of them. Every provider is read from the same `idleon_account::Decoder` the other
sections use; a term whose formula is not modelled yet is named by source and makes the result a
lower bound, never a silently wrong number. The four `total-*` rows of the All Stats breakdowns
resolve through it with their partial reason, and the Damage Breakdown's summary, layer tabs and
mastery bar read the real layers instead of the missing-pipeline state.

The All Stats Dashboard has its calculation context (SA-01), its category breakdowns (SA-02) and its
system formula explorer (SA-03). The context is the character, map, map-list, skill and
construction-mode selectors with their save defaults (character one and its current map), and the
map access walk. The breakdowns are the descriptor registry (60 descriptors): every row keeps its id,
name, scope and category, and the nine category tabs group them. Eight carry-capacity rows are
modelled in full (saved rows, vault 11, bundles, stamps with the lab and pristine multipliers and the
exalted doubler chain, star signs with Seraph Cosmos, gem shop, guild, talents 78 and 634, companion
18, the Pantheon Shrine with its Boss3B card and map rule, prayers 4 and 12, bribe 23, materials and
the material talent), and the two research rows (`research-exp`, `research-afk-gains`) reuse the
simulator's own terms. The four `total-*` rows and the `damage` row read the shared stat engine
(`crates/stats-engine`): `damage` is the maximum damage over its three layers, partial with the
missing-metadata reason, and a dungeon map hides it as the Damage section does. The other
forty-five descriptors resolve to a warning instead of a guessed number. Wiring the remaining landed
providers into the sections that still name a gap is planned in
[`wiring-landed-providers.md`](wiring-landed-providers.md). `crates/stats/tests/stats.rs` checks
every category renders.

The explorer is the registry's second view, built from those same rows so a formula card can never
disagree with the breakdown above it: a search over names and every saved input a formula's terms
read, a category filter and a computed/not-modelled filter, and one card per formula whose value,
explanation and terms are the selected row's. The ledger line counts every category
(`15 of 60 formulas computed · Combat 0/13 · …`), so a category with no computed formula still shows
its place instead of disappearing.

The input change ledger is the sandbox behind those values (SA-04). The vault, gem-shop, talent,
guild and stamp levels the capacity rows read are editable, each edit is spliced into a clone of the
save (`crates/stats/src/edits.rs`), and the page resolves both the imported save and the
hypothetical one, so the breakdowns, the explorer and the ledger always describe the same state. An
edited input is marked with the saved value it replaced and resets one input or every input; the
affected-formula list reports every descriptor the edit moved with its saved and hypothetical value,
the delta and the percentage, and a row links back to the descriptor it names. Missing: a per-node
editor inside the formula inspector, which needs a level-table catalogue and a combat-setup save
writer, so equipment, obol, companion and bubble/prisma cells are not editable yet; an edit to a
cell the current character no longer lists is dropped, so the ledger and the applied save always
agree.
Details: [`docs/sections/stats-all.md`](sections/stats-all.md).

The Drop Rate Calculator has its inputs, total and formula tree (DR-01, DR-02): the character and
map selectors, the runtime panel that only a dungeon map shows, the total card and the breakdown
tree under it. A dungeon map's total ignores every saved term, so it is
`1 + (Dungeon Total LUK + Dungeon Drop Rate stat) / 100` over the two live run values; the tree's
children add back up to the card's number, with a source on every term and the shared tree's search
over labels, inputs and sources. With either field blank the card shows the reason and the tree
asks for the two run values. Every other map's total is the descriptor's whole pool chain, which the
shared stat engine does not carry a descriptor for yet (it totals LUK, not the drop-rate pool chain),
so the card and the tree both name that gap instead of guessing a number; the section stays partial
until that descriptor is modelled.
Details: [`docs/sections/drop-rate.md`](sections/drop-rate.md).

The Damage Breakdown's context and summary shell is built (part of DMG-01): the character and map
selectors, their save defaults, the map list (every named map in index order, with the dungeon and
current markers), the summary cards and mastery bar, the dungeon, missing-pipeline and
missing-metadata states, project/session persistence, and the layer arithmetic of the final
combine. The damage layers and mastery (DMG-02) are the `Damage Layers` tabs and the
`Mastery Breakdown` card: DDL0, DDL1 and DDL2 carry the descriptor's own title, value, `note`
(`after softcaps`) and pre-softcap value, and the mastery ratio is `min(0.8, 0.35 - talent 113 /
100 + (Mastery bubble + card 21 + talent 123 + etc 21) / 100)`, drawn as the page's tree with the
base and one node per term, so the terms add up to the ratio the bar shows.
The three layers resolve from the shared pipeline, including the golden-food multiplier, the bubble
post-passes (slab, Tome, class-level and family), the box rewards, the Lab mainframe composites and
the win-bonus options. The damage descriptor models the whole DDL0 weapon-power chain (Flurbo shop,
food Weapon Power, W5 card level, guild 3, the Beginner talent cap, the obol misc multiplier), the
Max HP / Max MP / Movement Speed descriptors behind the HP-, MP- and speed-scaled bubbles, the DDL1
talent chain and every DDL2 group from the Grudge/Slabbo rows through the post-softcap multipliers,
with each term kept by name in `Layers::debug`.
Details: [`docs/sections/damage.md`](sections/damage.md).

The Fountain section implements FN-01 to FN-04: the fountain summary, the fill and income rates per
currency, lucky coins and rubber ducks, the ground coin stacks, the eighteen cross-system bonuses and
the level planner, then the currency value table with a simulated desired currency, the per-water
POW 10 multipliers, the desired-currency simulator, the next unlock tracker and every water's upgrade
list with its cap state and a log-scale cost curve, then the upgrade planner and greedy optimizer,
the marble planner and marble optimizer, and the simulated run. `idleon_fountain::Fountain` reads
the cavern rows and names every bonus source the save does not carry. The simulation
has no randomness or wall clock, so the same cavern and limit replay the same timeline, and both
optimizers and the run go through the shared task runner, whose cancel keeps the previous result
and leaves the section's controls untouched.
Details: [`docs/sections/fountain.md`](sections/fountain.md).

The Jar Calculator implements JR-01 to JR-04: the overview and its rupie balances, the shared
production rate and six-tier slot table, the rupie value breakdown with its extra-chance and
per-break payouts, the opal/collectible/enchant chances and their tier scaling, the forty
collectibles grouped by effect with the enchant priority, the nineteen jar schematics and the
jar-type upgrade plan, the next-tier tab with its pow10 milestone ranking and off-map tier
accumulation, and the enchant simulator with its history. `idleon_jar::Jar` reads `Holes`,
`Spelunk`, `OptLacc`, `Gaming` and the account rows. The upgrade plan marks only the next jar type
affordable and never spends past the rupie balance it is paid from. `idleon_jar::simulate` runs the
enchant simulation with a seeded generator, so the same inputs and seed reproduce the same spread; the tab sets the enchant count, simulation count, strategy, focus and every found
collectible's target, and the result adds the enchant odds per jar tier. Every applied save records
one history snapshot in the project document, a repeat of the last spread is skipped, the history
keeps the newest 200, and the tab clears it. Missing: a sparkline behind each history tile; the
history reads as the current-spread and recent-snapshot tables instead.
Details: [`docs/sections/jar.md`](sections/jar.md).

The Royal Guardian section implements RG-01 to RG-04: the overview with the counters (built
outposts, worlds unlocked, Armory levels, stationed units, militia clearing), the world progress
cards, the raw clearing values every unbuilt outpost reports (progress, requirement, remaining,
militia, rate, ETA) and the range target behind an outpost's Advanced Logistics plan; the outpost map
with its saved Normal, Support and Savage links, the resource nodes with their grade, capacity,
collected progress and drain projection, the currencies with their balances and source nodes, and the
banked hours and daily reset countdown; the upgrade list with every Armory shelf priced from the
seven-factor Masterclass cost and every Orblet, with the next fifty levels of one shelf priced level
by level against the saved balance; the seven global breakdown rules (collection, range, rank EXP,
points, units, support, savage) through the shared formula tree; and the forecast, which projects the
native collect pass over the horizon, the collects the player places, the menu-open windows and the
daily reset, with the fill, rank, clear and affordable-upgrade events each collection would commit.
`idleon_royal_guardian` reads `RoyalG`, `RoyalMaps`, `Spelunk`, `Sushi`, `ArcadeUpg`, `OptLacc`,
`BundlesReceived`, `ChestOrder`/`ChestQuantity`, `InventoryOrder_<n>`/`ItemQTY_<n>`, `Lv0_<n>` and
`TimeAway`; its tables carry the Armory name, cost, level-cap, currency and unlock-order columns,
the Orblet rows, the Legend talent's daily discount rate, the two rank-EXP shop rows, the resource
(capacity and currency slot), map and talent tables plus the three cross-system constants the
collection rate reads. Every rate reads its talents' purchased level;
the all-talent bonus levels that raise them are not decoded yet, and the collection multiplier also
leaves the W14 alchemy bubble and the `it.json` companion roster out, so the rate card names those
sources instead of guessing. The game's shelf descriptions read systems this build does not decode,
so the upgrade list shows each shelf's own bonus instead. The forecast has no timeline chart: the agenda lists the same events with their hours, and the sleep block and the
collect suggestions are not carried.
The optimizer runs a bounded, deterministic beam search over the legal moves (outpost upgrades,
adjacent type switches, resource and Support links, profession changes), scored by the currency the
linked nodes pay, the rank EXP the units and militia earn, or the balanced score that sums both with
the outposts' reach. Every accepted move is re-validated, so a recommendation is a layout the game
accepts and its moves replay to it. The picker offers the three objectives the kingdom metrics carry
on their own; an Armory purchase target, a currency portfolio and the reset-collection objectives
are not modelled. The unit EXP planner runs the trading/surveyor split search with its switch points and the same-world Militia, Commander and
Knight moves; its steps only retarget slots the outpost already fields, and it sets no Purity
target. The shelf planner alternates a layout step (minimum Guards for reach,
the rest Workers, the focused currency first, Trader/Surveyor rank fallback for outposts with nothing
left) with the purchase step it spends the balance on, advancing through node depletions and the
daily reset; multi-currency portfolio scoring and a profession beam are not carried, so the
plan is deterministic but not the portfolio optimum. Both planners run through the shared task
runner, so they report progress and cancel like every other long run.
Details: [`docs/sections/royal-guardian.md`](sections/royal-guardian.md).

The Spelunking section has its controls, party, elixir belt, depth estimates, expedition run and
depth optimizer (SP-01 to SP-04), with the phased Amber policy (SP-03). `idleon_spelunking` reads the tunnel rows, saved stamina, shop upgrades, elixir
unlocks and depth records from the save, normalises the party, belt, mode and policy, rejecting an
invalid one with a message, and computes the Depth limits profile. The four account multipliers that
depend on the whole stat context — Spelunking Power, the amber multiplier, stamina regeneration and the Grand Discovery
chance — stay explicit inputs here; the page carries all four, the extra-exit bonus and each
character's maximum stamina. The section renders the character/cave/mode/policy/full-clear/speed
toolbar, the party with the half-stamina rule, the equipped and memorized belt with the built-in
presets, and the two depth charts with a per-depth inspector. `idleon_spelunking::sim` runs
the whole expedition — the seeded 10 ms tick, floor generation, digging,
the elixir belt with its refills, party switching, exits, the Delve toll and the run log — so the same
inputs and seed reproduce the run; the page runs it through the shared task runner with progress and
cancel, and shows the outcome, rewards, stamina, per-member contributions and the event log. The
phased Amber policy covers the full-clear buildup, the stamina burn from the
dump depth, the orange activation at the finisher reserve, the boosted harvest and the rope search,
with a rope exit keeping every reward whole. Its five limits — dump depth, finishers, reserve,
harvest depths and rope reserve — normalise with their own rejection messages, and every phase
change, its reason and each orange snapshot land in the result and the event log.
`idleon_spelunking::planner` holds the search scope and its
configuration count, the exhaustive configuration search (parties, policies, clears, Auto speeds,
belts, memorized subsets, manual tools and entry) with its fresh-seed finalist comparison and the
paired shorter-party advice, and the bounded Amber phase search (a diverse grid, two local
refinement rounds and an adaptive fresh-seed comparison with the rope-exit filter). Both searches
run off the UI thread through the shared task runner with progress and cancel, score the planner
objectives, keep the winner against the selected baseline and write a copyable plan; the result
shows every compared row, its depth, Amber, settled count and rope/toll/blocked split.
Details: [`docs/sections/spelunking.md`](sections/spelunking.md).

The Opal Optimizer section implements OP-01, OP-02 and OP-03. The `Current state` tab shows the
five villagers, their levels, invested opals and stars, the six overview figures, the villagers
table, the bonus terms and the shared formula tree, switchable between the total and one villager.
The `Optimizer` tab splits a total across the unlocked villagers for the total EXP or one villager's
rate: the plan comes off the shared task runner, prints the saved and planned rate per villager, sums
to the total and can be copied as tab-separated text. `idleon_opal::Opal` decodes the villager
inputs out of the save — the villager rows, the cosmo upgrades and every cross-system term through the account
decoder — and `idleon_opal::Opal::plan` searches every lowest count with each villager on that count
or the next multiple of ten, so the plan is never worse than the saved split and at least as good as
a greedy split. The `Manual simulator` tab holds one whole, non-negative opal count per unlocked
villager, a warning while the typed total leaves the save's, the lowest count and the Equal Spread it buys, and each villager's rate against
what it earns today, with a reset back to the save's own split. `idleon_opal::Simulation` carries that
arithmetic, so the simulator reads a distribution the optimizer planned exactly as the plan does;
the manual counts live in `sections.opal` in the project document, so an export carries them.
Details: [`docs/sections/opal.md`](sections/opal.md).

The Sneaking section has its loadout, inventory, formula breakdown, charm optimizer and floor
mastery workspaces (SN-01, SN-02, SN-03 and SN-04):
`idleon_sneaking::Loadout` reads the save's `Ninja` rows — the twin placements, the four
equipment rows per twin, the 39-slot inventory and the Ninja Knowledge levels — and computes the
item stats, the charm bonus map, the Gold inventory scan, action speed, KO recovery, the level,
charm, ally and flower stealth factors and the detection chance they give. Slots swap with the
inventory (two items trade places, so the editor cannot invent a charm), a repeated charm without
the Charmed emporium perk and a second Scroll of Power are flagged, and a save without `Ninja` rows
says so. The saved floor assignments line names each floor's twins, their action and the loadout's
try rate. `idleon_sneaking::account` reads the account-wide stack — Land Rank, compass,
statues, cards, achievements, gemstones, lamps, Hole upgrades, Emperor, RoG, Jelly and companions —
and `idleon_sneaking::breakdown` builds the five trees: Stealth, Jade per hour, EXP
per action, door damage per hour and the runtime action loop. The breakdowns workspace selects a
twin and a formula tab and renders each tree through the shared formula tree with its search,
expand and copy controls. `Loadout::optimize_with` is the charm optimizer: it
scores the saved placement first, then offers each untied twin every
legal ordered charm pair the owned pool can supply — the equipped and inventory charms, never a copy
the account does not own, with the Scroll of Power still account-unique — re-measures the whole
layout with the aim's own metric (`Loadout::score`: Jade per hour, EXP per hour, item claims, door
clears or untie progress per hour) and keeps a pair only when it beats what the twin had. Passes
repeat until nothing improves. The search moves charms only, so the saved floors, hats and weapons
stay put, and it runs off the UI thread through the shared task runner: it reports each twin, cancels
without touching the editor, and writes a recommendation only through the explicit Apply action,
which banks the charms it displaces so the account keeps every charm it owned. The Items objective
scores the deterministic claim rate; a Monte-Carlo claim simulation needs the flower pass this section does not build yet. The Floor Mastery workspace is
`Loadout::floor_mastery`: one row per floor at the saved mastery and its neighbours, with the floor's
flowers, funeral multiplier, detection base, Jade base, EXP base and mastery-scaled door HP, plus the
saved placement's best detection, Jade and EXP per hour, door state and ETA, and the captive's untie
ETA. A projected mastery resets the flowers and relocks the doors, the game's own what-if when the
mastery advances. `Loadout::active_door` prints the one unfinished door with the saved placement's
damage and hits per hour and the hours the door needs to clear; `Loadout::next_captive`
prints the first captive in roster order and its untie ETA. A floor with no twin says so, its
detection is a dash, and a door with no hitter prints its state instead of an endless ETA.
The Gambit point chain and the terms that the All-Talent resolver rescales stay explicit inputs, and
each such node says so in its note.
Details: [`docs/sections/sneaking.md`](sections/sneaking.md).

The Prisma section implements PR-01 and PR-02: the plan for one character and cauldron (every
bubble's saved level, its saved Prisma ownership and which of the four statuses it is in), the
save's Prisma multiplier, the streamed bubble search, the ranked per-metric rows and the coverage
table. Each candidate is the saved state with one bubble's Prisma flag toggled, and every row
compares without → with that bubble's Prisma, so a ready bubble previews the gain and an
already-Prisma bubble measures the benefit it retains. A run measures every purchased bubble's bonus
(the curve at its saved level times the Prisma multiplier, an intermediate) plus the downstream
outcome families this build owns: today `statue-retention`, the Statue EXP the STARTUE EXP bubble
buys. The rows rank by any of those metrics and filter by name, effect or changed
outcome; the coverage table lists every purchased effect in the cauldron with the native game
scripts that read it and whether this build models a downstream metric for it, so the bubble bonus
is never presented as a stat, skill or damage gain. The shared task runner reports progress and keeps the previous result
when a run is cancelled, and a save, character, cauldron or AFK-hours change marks the result stale.
Missing: the damage, stats and skills outcome families beyond the modelled one, and the bubble
icons.
Details: [`docs/sections/prisma.md`](sections/prisma.md).

The Monument section is complete: Bravery (MN-01), Justice (MN-02), Wisdom (MN-03) and bonus
sources (MN-04) cover Paying Respect training, saved Bravery stats and sword odds for the entered
run state, saved Justice and Wisdom starts, every milestone and all three permanent reward sets,
and the reward charge a monument's saved wait pays (plus its 24-hour preview), including the
Meritoc vote, sushi tier and gaming superbit behind it. The Sources tab gathers the permanent and
charge summaries: each monument's ten saved reward levels totalled, companion 135's reward-level
multiplier, every next-run charge with its 24-hour preview, the linear-window, Meritoc and Legend
factors, and the training pool as one named table whose rows sum to the additive gain card. The
character selector supplies the Summoning access the charge reads.

The Justice tab is a local worksheet, not a live-game connection: it reads the 90 cases,
fills each one's coin and popularity range, lists all three responses with every random outcome
and the share of outcomes that keep the story alive, names the delayed deals the next case settles,
and suggests a response with the survival-first heuristic's reasoning. Every worksheet step (a stat
edit, an effects toggle, a recorded response, an opened case) is undoable, the case search filters
by visitor or request wording, and the worksheet travels in a project document while its undo trail
stays a session convenience. Details: [`docs/sections/monument.md`](sections/monument.md).

The Cog Board Optimizer implements CG-01 to CG-04: the saved 12×8 board and shelf decode from
`CogO` and `CogM`, the three single-objective runs (build speed, construction EXP and Flaggy rate)
run through the shared task runner with progress and cancel, the optimized placement renders cell by
cell, and Reset restores the save board. The board insights remove each movable cog from a copy of
the board and rescore, so the weakest and strongest cogs, their share of the score and the
replacement targets match the optimizer's own score; the roll ranges show each tier's per-roll and
total bounds at the effective construction level, inferred from the board's own rolls, with the
level at which each bound rises; and the shelf lists the optimizer's pool read-only. The optimizer
runs greedy construction, simulated annealing and a greedy finish with a seeded random source; invariant tests prove it never returns a board
worse than its input and that its swap list rebuilds the result. CG-02 adds custom weights: the
three fields normalise against the save board's own totals and Optimize maximises
`Σ weight × metric / reference`, while negative, non-finite or all-zero weights are rejected with a
banner and never start a run. Perfect Board is a layered eviction: perfect T3 base
stats, T3 directionals, crystal fillers and surrounds, Excogia blocks and Yang cogs, rebuilt for
every player slot and kept only when a layer raises the score. It returns the layer list and
perfect stats that explain the board, targets the selected single goal, and reads the effective construction level the roll ranges use. The tiny-cog board is
not decoded yet, so the perfect board covers the 12×8 board only. A finished run opens the swap
walkthrough at the board it started from: Next applies the swap list in order, Previous rebuilds
the prior board and Done keeps the optimized placement. Details:
[`docs/sections/cog.md`](sections/cog.md).

The Farming section's assignment and Land Rank allocation are built (FM-02): the objective selector
re-plans which crop each of the 36 plots should grow, the table names the current and target crop,
family and change per plot and the field tiles carry the target, and the allocator spends the
account's Land Rank points off the UI thread through the shared task runner. The allocator is a
greedy marginal search over the twenty upgrade rows, with the unlock rank and fifth-column cap rules
and the per-objective candidate lists; a run reports progress, cancels without dropping the previous
plan, and an objective, mode or save edit marks the stored plan stale. FM-05 adds the Overview tab:
the priority signals, the account multipliers, the GMO thresholds, the ten Crop Scientist bonuses
and the seven growth breakpoints. The account snapshot applies only the terms the save carries, so
every figure is a lower bound. FM-03 adds the preset comparison and the named presets: the Plots tab
runs the "Preset Comparison" table through the shared task runner, so every goal reports its gain,
the points it spends and the Land Rank rows it raises from the current baseline, and the preset bar
keeps a named goal, baseline and crop target in the project document. The built-in presets are the
seven objective goals. FM-04 adds the Evolution & OG tab: `idleon_farming::evolution` plans the
evolution chain and each of its steps, so the chain from the deepest eligible plot to the target
crop prices every remaining depth — the chance any eligible plot rolls it, the per-plot range, the
cycle, and the expected, median, p90 and p95 attempts — and the tab shows the evolution cards, the
"Hardest Remaining Steps" table (the twenty slowest stages), the overgrowth stopping cards for the
selected plot and the evolution assignment. Every plot's long-run overgrowth curve, with the plot
selection switching it, sits on that tab too. Details:
[`docs/sections/farming.md`](sections/farming.md).

The Cooking Mastery section's Mastery EXP tab is built (CK-01): the mastery rank, the free points,
the current EXP rate, the EXP the next level takes and the hours that needs, the upgrade comparison
ranked by the gain one more point buys, and the EXP rate breakdown in the shared formula tree. Every
figure comes from the imported save, through `CookMaster`, `Meals[0][73]`, `Ribbon[28..]` and the
external multipliers (grid node 190, superbit 68, companion 87, menagerie 21, the Cook
Maxxing fountain upgrade, the Canteen Read vial, arcade 69 and salt lick 10). The daily distribution
(CK-02) and the yellow point allocator (CK-03) are built on the same crate. Details:
[`docs/sections/cooking-mastery.md`](sections/cooking-mastery.md).

The Ladle Optimizer implements LD-01 to LD-03: the per-character ladle rates with the claim plan
and its Post Office 19c roll and Prayer 2 cap, the Cooking EXP balloon model with overflow, two
shared formula trees and the daily reset. The Kitchens & Jellyfish tab claims the checked
characters' guaranteed ladles, donates the policy's share to the Blue Jellyfish, spends the rest on
the cheapest meal level (whole ladles first, passive kitchen time after), banks leftover hours into
the meal they were cooking, lets No Meal Left Behind raise its lowest eligible meal, and finally
cooks the reset's grant in every kitchen before buying the levels its stock covers. Each reset's
layout comes from a search over whole-kitchen assignments, with pins,
meal exclusions, the saved layout and the Dream 11 first-purchase discount, and each reset expands
into its spend order and a copy button with the same instructions as plain text. The objective
select ranks the plan and the layout by the most meal ranks or by progress toward the meal targets,
and the forecast compares the saved assignments, the optimized assignments and the policy over the
same horizon, with the extra ranks each adds and the daily-grant and cumulative-rank charts. The
meal outcomes table lists every discovered meal's current and projected level, the levels the
ladles, the grant and NMLB bought, its target input and the table's sort. The account terms that need
the full stats engine are entered instead: the ladle panel values, the kitchen speed at
Speed Lv 0 and the meal max level stand in for the Cooking speed model, and the meal cost terms
(companion 162, achievement 233, cloud 33, Dream days) are the Cooking menu's discount inputs.
A kitchen-speed objective is left out on purpose: it would rank by a meal-level kitchen speed model
this build does not have, so the select offers only the two objectives it can compute rather than a
constant ordering.
The NMLB panel splits every meal into ladleable meals (the next level fits in a day of kitchen
time) and targets, then holds the floor: the lowest target level, with the ladleable blockers that
must rise first and the rotation NMLB walks while the floor is held. The Blue Jellyfish donation
planner compares an up-front share of the owned ladles and a share of the guaranteed income until a
cutoff reset, refines the best split and shows the whole-ladle schedule; it runs through the shared
task runner, so cancelling keeps the recommendation the tab already shows, and an improved schedule
can be applied to the meal plan. Because kitchen speed is a fixed entered rate here, the search
ranks by the section objective.
Details: [`docs/sections/ladle.md`](sections/ladle.md).

The Jelly Operator section implements JO-01, JO-02, JO-03 and JO-04: the overview, the layout
editor, the operation simulator, the upgrade list, the obstruction rewards, the formula breakdowns
and the board section planner. The section decodes the save's own board (`Research[14]`), its
progress row, the cell EXP and levels, the upgrade levels and the plot tokens, then reads the
account systems the operation multiplies (research grid nodes 185-187, Spelunking palette 1, Legend
Talent 41, the Bloodclot artifact, arcade 72, Atom 15, the `ban_j` bundle, event shop 56/57, Sushi's
Ring of Glory bonus 63 and Menagerie's Upgrade Doubler row). The layout tab draws the 18×10 board
with the cell and obstruction sprites, the legend, the placement rules and a palette that places
every cell type the save unlocked; a click on a placed cell removes it and Reset restores the saved
board. The overview shows the account snapshot and runs the seeded operation estimate through the
shared task runner: the operation simulated frame for frame (projectiles, the one-second Best DPS
callback, Fever, Stronkroid, boss attacks and revives), its Monte Carlo aggregate, the cell
contribution table, the critical target timeline and the progression outlook, whose obstruction
rows come from the deterministic surrogate. The simulator tab is a sandbox copy of the same slice:
the stage and Fever selects, Stronkroid's activation second, the automatic revive policy, the seeded
single operation through the shared task runner, the palette with remove mode, Clear cells and Reset
from save, and hypothetical board sections that reflow the board through `board::normalize`. A run
shows the outcome cards, the per-cell contribution table and the critical target timeline. The
sections tab ranks every unpurchased plot: `idleon_jelly::planner` screens each section's geometry,
completes the board around it (the one-slot fill, then the new large-cell anchors the section
opens), keeps the strongest sequences with a bounded non-greedy beam, and compares the finalists
with shared-seed trials and a paired confidence score. The strategy presets, objective, EXP target,
token count, quality profile, beam width and seed are the tab's controls; the plan keeps the current
board as its baseline and shows the recommended sequence, both boards, the operation comparison and
both ranked tables. A plan is deterministic for its inputs, and the beam width changes which section
combinations the search keeps. The upgrades tab is `idleon_jelly::upgrades`: one row per slot of the
`Research[44]` purchase order with its game description filled in, the slot's own price curve, and
its Research level, prerequisite, max-level and affordability answers; the price divides by Lower
Cholesterol's quantity and the row says when Menagerie's Upgrade Doubler doubles it. The bonuses tab
is `idleon_jelly::bonuses`: all 72 `Research[45..47]` obstruction rewards with their account target,
the corrected bonus text and its normalised effect line, each marked active once the obstruction is
cleared, and the pane warns while Operational Bonuses (upgrade 39) is still locked. The breakdowns
tab is `idleon_jelly::breakdowns`: the cell damage, cell speed, cell EXP and bloodcell breakdowns as
the shared formula tree, the layout's damage and speed passives, and the per-cell output table; the
rail picks one tree at a time. A tree's factors multiply back to the multiplier its category
reports, its `Additive …` groups turn their own percentages into their factor, and a breakdown the
save cannot compute (Cell EXP without Cell Biology, Bloodcells without Bloodcell Coagulation, a board
the rules refuse) says what is missing instead of printing one. The optimizer tab is
`idleon_jelly::optimizer`, a coordinator over the bounded full-board search: independent starts each
build composition seeds from cell-type and anchor orders plus the elites the earlier starts handed
on, keep a diverse beam through their build, saved-board and refinement rounds (placements,
replacements, relocations and re-fills), and share their strongest, sufficiently distinct boards
with the later starts. The coordinator rotates explore, refine, repair and measure; refine trades
build rounds for saved-board rounds and repair sets the exact-repair budget, and payoff is scheduled
from each start's own score count, so a run stays deterministic. The strongest finalists run the
save's Fever and revive policy on shared seeds, rank by the simulated objective score and carry a
paired confidence against the board the run started from; the recommendation is never worse than
that board, and `Use this board` puts it on the layout. The search is single-process, with no worker
pooling, policy screening, exact enumeration or certification. Grid nodes 185 and 187 read the
save's shape overlay as it is. Details:
[`docs/sections/jelly-operator.md`](sections/jelly-operator.md).

The Coral God Rank section implements CR-01, CR-02 and CR-03: the planner controls with
validation and the save's current coral state, the bounded deterministic beam search through the
shared task runner, then the offering fit and the current Coral Kid levels. A run keeps the best plan
per beam, reports each step, can be cancelled and marks the stored plan stale when an input changes.
The card shows what the plan commits, the projected coral state against the current one and the
ordered Coral Kid and God Rank offering actions. The search is `idleon_coral::plan`: every
unlocked upgrade and one offering per state, candidates that do not strictly improve the goal
dropped, states deduped by level/rank signature with dominance pruning, then sorted by the goal,
spend and signature before the beam is cut. The offering fit reads the offering row at the current
God Rank: the tier's success chance, the paid cost and the expected spend for one success,
with a caption that turns the Y4 free chance and the planning confidence into the attempts and
worst-case spend the plan reserves. The current-levels table adds each upgrade's unlock status, and
both cards name the save fields they read as zero (Y4, DivCostAfter3, and the Coral Kid and
Singularity level pages) instead of hiding the gap. Details:
[`docs/sections/coral-god-rank.md`](sections/coral-god-rank.md).

## 10. Verification

- **Formula tests:** Rust tests of each section's formulas and game rules; tolerance documented per
  formula.
- **Optimizer invariants:** never worse than input, respects caps/budgets, deterministic for a
  seed, cancellable.
- **UI coverage:** harness opens every section from the dashboard, asserts the root and main controls,
  drives one core flow and saves a screenshot.
- **Import round trip:** raw save, project JSON and section config round-trip; malformed
  input leaves state untouched.
- **Gates:** `cargo test` (`-- --include-ignored` adds the long simulations),
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `node script/roadmap.mjs`, `node script/mcp-drive.mjs`, `node script/mcp-sections.mjs`.

## 11. Milestones and progress tracking

`docs/roadmap.json` is the source of truth. It defines one milestone per section and one issue per
feature, with acceptance criteria and explicit `dependsOn` edges.

Every feature issue contains:

- the feature description and its section contract (UI, inputs, outputs, assets);
- **Blocked by** and **Blocks** links resolved to issue numbers;
- the generated section spec;
- acceptance checklist and section verification steps;
- agent notes: keep core GPUI-free, reuse M0 components, close only when green.

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

Acknowledged gaps are tracked in the same file: a milestone's `gaps` list names each one
with the `blockers` entry it waits for, and [`docs/roadmap.md`](roadmap.md) renders the backlog by
blocker. When a blocker's status becomes `landed`, every gap that names it is ready to be closed;
when a section's prose here mentions something missing, it should have a gap entry. Gaps are not
synced to GitHub issues.

## 12. Risks

- **Scope:** 30 sections is a large surface. Mitigated by one milestone per section, shared
  primitives and a strict definition of done.
- **Formula drift:** game updates change formulas and tables; mitigated by per-crate data tables and
  formula tests.
- **GPUI churn:** three pinned projects; mitigated by exact pins and the smoke harness.
- **Asset weight:** 42 MB of PNGs, mostly combat-sim, now in the repository; a refresh is a single
  reviewed diff and the manifest keeps references honest.
- **Performance:** large simulations (combat, tower defense, decision tree) must run off the UI
  thread and stay cancellable.
