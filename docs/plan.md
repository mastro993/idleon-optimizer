# Plan: Idleon Optimizer, a native desktop suite

Status: **foundation in progress, research, statue, jar, farming, ladle, Clamworks, the Cglunko
overview and upgrade plan, the Prisma calculator, Cooking Mastery, Fountain, the Opal Optimizer's
current state, the Royal Guardian overview, the Coral God Rank planner, the Spelunking controls and
depth estimates, the Sneaking loadout, inventory and formula breakdown workspaces, the combat
simulator's loadout editor and mechanics overrides, the cog board optimizer, the Clamworks optimizer,
the Jelly Operator overview, layout editor and operation simulator, the drop-rate inputs and dungeon
total partial, sailing, shadow-ban, endgame, Fishing, Killroy and BoneJoePickle done, Monument
Bravery and Wisdom, the All Stats context and breakdowns built, 2 sections planned.** The live
breakdown is
[`docs/roadmap.md`](roadmap.md); the machine-readable source is [`docs/roadmap.json`](roadmap.json);
each section's full specification is under [`docs/sections/`](sections/).

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
- Per-section state survives tab switches and dashboard visits for the session, and persists across
  launches in the local session file with the imported save and the active section.

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
  recent imports and UI preferences. The app writes it to the platform app-data directory as
  `session.json` (override with `--state-dir` or `IDLEON_OPTIMIZER_STATE_DIR`): the project document
  plus the last applied imports. A corrupt file is renamed to
  `session.json.corrupt-<timestamp>` and the app starts clean.
- `SectionState`: serializable state owned by one section (targets, filters, custom presets, replay
  seeds, plans).

### 5.2 Accepted JSON formats

1. **Raw decoded save** — `it.json` (`{ data, serverVars, companion }`) or flat `save.json`. This is
   the primary user-data import and the format the game writes.
2. **Optimizer state string** — upstream `ROv2:` + gzip + base64, exported by the research page.
3. **Project JSON** — `{ "format": "idleon-optimizer-project", "version": 1, "save": {...}, "sections": {...} }`.
4. **Section config JSON** — `{ "format": "idleon-optimizer-section", "section": "farming", "version": 1, ... }`.

Detection order: project JSON, section config, ROv2 string, raw save. A raw save is detected by
`Research`/`Lv0_*`/`gridLevels` fields; an object with a `format` marker this build does not read is
rejected by name, and an object with no marker and no save field is rejected with the field list it
expected. `crates/core/src/import.rs` is the one pipeline every accepted format goes through.

### 5.3 Import pipeline

1. Read from file picker, paste box or drag-and-drop.
2. Parse and detect the format; never mutate state before validation passes.
3. Migrate older versions; unknown fields become warnings, not failures.
4. Build a preview: format, save name if present, characters, research level, detected sections and
   warnings.
5. Decode everything the document needs while previewing (the embedded save and each section
   config), then apply atomically: either the whole import succeeds or the previous state stays
   intact. Log every applied field change as `field: old → new`, summarise it in the status bar and
   keep the list in the dialog.
6. Persist the imported save metadata and add it to recent imports; the session file keeps the
   whole project so the next launch restores the section, save and section configs together.

Errors name the failing field and the expected shape. Examples: `not valid JSON: expected value at
line 1 column 2`, `unknown format "some-other-tool"; this build reads "idleon-optimizer-project",
"idleon-optimizer-section" or a decoded save`, `unsupported version: project version 3; this build
reads 1`, `no save fields found: expected Research, Lv0_*, gridLevels`.

### 5.4 Export

- Export project JSON with a schema version and per-section blocks.
- Copy the current research state and supplements as the upstream `ROv2:` gzip+base64 string; it
  imports back here exactly and still opens in the upstream optimizer.
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
    clamworks/                      the Clamworks overview, upgrades, claim, plan and promotion
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
script/extract-stats-data.mjs       extract the map and class tables the stats context reads
script/extract-prisma-data.mjs      extract the bubble table the Prisma calculator reads
script/prisma-oracle.mjs            regenerate the Prisma plan and bonus oracle fixture
script/extract-opal-data.mjs       extract the opal tables (cosmo, compass, statue, grimoire, lore)
script/opal-oracle.mjs             regenerate the villager EXP oracle fixture from opal-calc.html
script/extract-fountain-data.mjs    extract the fountain upgrade table and its account rows
script/extract-jar-data.mjs         extract the jar collectible, schematic and measurement tables
script/jar-oracle.mjs               regenerate the jar formula oracle fixture
script/clamworks-oracle.mjs         regenerate the Clamworks promotion and plan oracle fixture
script/clamworks-afk-oracle.mjs     regenerate the AFK claim and comparison oracle fixture
script/extract-spelunk-data.mjs     extract the cave, rock, tool, elixir, chapter and sprite tables
script/spelunking-oracle.mjs        regenerate the Spelunking inputs, profiles, runs and rejections
script/extract-royal-data.mjs       extract the Royal Guardian armory, resource and map tables
script/royal-oracle.mjs             regenerate the Royal Guardian overview oracle fixture
script/extract-farming-data.mjs     extract the farming tables, emporium encoding and gates
script/farming-signals-oracle.mjs   regenerate the farming signals and curves fixture
script/extract-sneaking-data.mjs    extract the ninja equipment, knowledge and floor tables
script/sneaking-oracle.mjs          regenerate the loadout formula oracle fixture
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

## 9. Section status

Research, shadow, endgame and BoneJoePickle are implemented; research is still partial. Done: the
dashboard (summary, grid and observation previews, EXP/AFK/insight breakdowns and the grid-point
split), grid, shapes, magnifiers, local sim, shape/mag/grid optimizers, 16 presets, supplements,
save import, the ROv2 state string export, projection chart, keyboard, canvas dragging, the
shared searchable EXP formula tree, target validation and the three run toggles (assume
observations unlocked, extended insight lookahead and the daily tournament +12h).
The dashboard's AFK rate reports Gambit Milestone 15 as not modelled; its Hole measurement chain
lands with the Minehead work. Missing: the six extra tabs (Insight ROI, Observation Unlocks, Shape
Optimization, Decision Tree, Minehead, Sushi Station), target-aware simulation, strategy comparison,
complete save-derived multipliers and undo.
Details: [`docs/sections/research.md`](sections/research.md).

The Fishing section implements FI-01, FI-02 and FI-03: the character/toolkit/location toolbar, an
every-location table that switches the selected pool, the selected pool's fish split,
real-versus-panel EXP, the shared formula tree, the ranking of all 225 lure/line
combinations for the selected pool and depth by either Fishing EXP or fish quantity, with
the selected and saved rows marked, a `Use` action, the tie explanation and the
best-combination KPI, and the Trench depth projection: the saved depth's fish requirement
with its jar and Bolaia reductions, the efficiency requirements each projected depth
raises, the selected and fish-maximizing lure + line hours per depth with cumulative
totals, and the other characters' fish per hour, which sum into the team ETA. The pure
formulas, the ranking and every projected depth are pinned against oracle fixtures
evaluated from the upstream JS; the save supplies the character, rod, saved toolkit, spot
and AFK target plus the cavern rows the Trench reads (`Holes`, `Spelunk`), while the
account skill values (efficiency, EXP multiplier, AFK rate, Multi-Fish, speed sources,
prowess and equipment depths) are entered in the account card until the stats dashboard
decodes them, and the ranking and the depth search hold efficiency and the EXP multiplier
fixed across combinations because the upstream recomputes them per toolkit through the
skill systems. Details: [`docs/sections/fishing.md`](sections/fishing.md).

The Clamworks section implements CW-01, CW-02 and CW-03: the overview with the pearl-value
breakdown, every pearl upgrade, the upstream priority ranking, the AFK claim with its drop
distribution, payout and `+1` comparison, the budgeted upgrade plan (a greedy pass over the same
ranking, priced at the levels each purchase reaches and run through the shared task runner, with
progress, cancel, a stale marker and an applied projection), the promotion tracker and all twelve
worker-class rewards, plus save decoding for pearl balance, upgrade levels, unlock progress,
promotion and Scurge. `script/clamworks-oracle.mjs` regenerates
`crates/core/tests/clamworks-oracle.json` from the upstream module, so the tracker, the rewards and
the plans are compared with the source of truth, and `script/clamworks-afk-oracle.mjs` regenerates
`fixtures/clamworks-afk.json` for the claim. Multi-Scalping takes a manual multikill total, and the
claim takes the character's kills per hour and drop rate, until the combat simulator lands. The
claim does not carry the upstream reroll-for-pure-pearls policy, which needs the talent and guild
bonuses the stats context resolves. Missing: Scurge combat.
Details: [`docs/sections/clamworks.md`](sections/clamworks.md).

The Killroy section implements KL-01 and KL-02: the weekly forecast with every character's three
encounters and records, and the `+1` upgrade comparison with the Baseline and Upgrade tabs, the
upgrade list, the selection queue preview, deterministic trials and the best-option highlight. The
player's damage belongs to the combat simulator, so a trial takes the character's kills per second
as an input until M2 lands; the arrival schedule, respawn bonus, `100 + Time` duration and the
per-kill skull, talent-point, credit and pearl rolls are the game's own formulas. Missing: the full
weekly results tables, saved buffs, unlimited inventory and the JSON export (KL-03). Details:
[`docs/sections/killroy.md`](sections/killroy.md).

The hidden BoneJoePickle page is complete: DPS and max-time-to-kill inputs, the nine Death Note
minibosses, max pickles, effective HP and kill time, ported from `pickle-calc.html` with formula
tests and a section-harness interaction. Details: [`docs/sections/pickle.md`](sections/pickle.md).

The Combat Simulator's loadout editor is built (CS-01): the character picker, the 16 armor and 7 tool
slots with their saved `EMm` upgrade deltas, the talent table from `SL_<n>`/`SM_<n>`, and named
loadout presets in the project JSON. The item catalog is extracted from the upstream `ITEMS` table by
`script/extract-combat-data.mjs`, and every edit recomputes the gear totals the header shows. Missing
or invalid items are flagged: an unknown key, the wrong slot family, a level or class requirement the
character misses, and a saved talent above its maximum. Mechanics overrides (CS-02), the encounter run
and its results (CS-03 to CS-06) and replays (CS-07) stay on the roadmap.
Details: [`docs/sections/combat-sim.md`](sections/combat-sim.md).

The shadow-ban check is complete: it reads `OptLacc[26]` from the imported save and shows YES, NO or
UNKNOWN with the save status under the verdict. Details: [`docs/sections/shadow.md`](sections/shadow.md).

The statue section has its table, its estimates and its batch controls (ST-01 to ST-03). Done: the
32-row table, the saved level and EXP progress, the next-level requirement including the
statue-1/2/3 rounding gates, per-row availability, the character selector and search/sort; the
STARTUE EXP bubble, Skinny 0 Cal vial, Onyx/Zenith tier and Royal Guardian Flair behind a per-row
target, the statues that target needs under a guaranteed or average deposit estimate, and the time
from a raw statues/hour rate with its source shown; the upstream Set targets entry (costed for every
statue first and rejected with the upstream message), Next level for all, a one-step undo of the
batch action, and the farming breakdown behind one statue's row through the shared formula tree.
Missing: the upstream "Measure statue farming" panel, which needs the combat simulator's encounter
run (CS-03), so rates are manual entries today. Formula parity against the upstream JS uses the
fixtures in `crates/core/tests/statue-requirements.json`, `statue-costs.json` and
`statue-batch.json`. Details: [`docs/sections/statue.md`](sections/statue.md).

The endgame check is complete: the joke verdict plus the 177-metric leaderboard audit, with the 156
save-backed metrics ported and the 21 account-stat metrics named instead of guessed.
Details: [`docs/sections/endgame.md`](sections/endgame.md).

The All Stats Dashboard has its calculation context (SA-01) and its category breakdowns (SA-02). The
context is the character, map, map-list, skill and construction-mode selectors with their save
defaults (character one and its current map), and the map access walk ported from
`js/stats/systems/common/map-access.js`. The breakdowns are the upstream descriptor registry
(`js/stats/registry.js`, 60 descriptors): every row keeps its id, name, scope and category, and the
nine `CATEGORY_ORDER` tabs group them. Eight carry-capacity rows are ported in full
(`js/stats/defs/carry-capacity.js` and `js/stats/systems/common/carry-capacity.js`: saved rows,
vault 11, bundles, stamps with the lab and pristine multipliers and the exalted doubler chain, star
signs with Seraph Cosmos, gem shop, guild, talents 78 and 634, companion 18, the Pantheon Shrine with
its Boss3B card and map rule, prayers 4 and 12, bribe 23, materials and the material talent), and the
two research rows (`research-exp`, `research-afk-gains`) reuse the simulator's own terms. The other
fifty descriptors resolve to a warning naming their upstream def instead of a guessed number, since
they need the shared talent/stamp/bubble/card/equipment stat pipeline. The capacity rows match the
upstream stats page exactly on the committed fixture; `crates/core/tests/stats.rs` pins the eight
values and checks every category renders. Missing: the formula explorer (SA-03) and the input ledger
(SA-04). Details: [`docs/sections/stats-all.md`](sections/stats-all.md).

The Drop Rate Calculator has its inputs and total (DR-01): the character and map selectors built the
way `populateMapSel` builds them, the runtime panel that only a dungeon map shows, and the total
card. A dungeon map's total ignores every saved term upstream, so it is
`1 + (Dungeon Total LUK + Dungeon Drop Rate stat) / 100` over the two live run values, and
`script/drop-rate-oracle.mjs` pins it against the upstream calculator in `fixtures/drop-rate.json`;
with either field blank the card shows the upstream reason. Every other map's total is the
descriptor's whole pool chain, which is the shared stat pipeline the All Stats breakdowns (SA-02)
still name as a gap, so the card names that gap instead of guessing a number. Missing: the formula
tree (DR-02). Details: [`docs/sections/drop-rate.md`](sections/drop-rate.md).

The Fountain section has its overview (FN-01) and its currency and upgrade tabs (FN-02): the
fountain summary, the fill and income rates per currency, lucky coins and rubber ducks, the ground
coin stacks, the eighteen cross-system bonuses and the level planner, then the currency value table
with a simulated desired currency, the per-water POW 10 multipliers, the desired-currency simulator,
the next unlock tracker and every water's upgrade list with its cap state and a log-scale cost
curve. `idleon_core::fountain::Fountain` reads the cavern rows and names every bonus source the save
does not carry; `script/fountain-oracle.mjs` regenerates the upstream figures in
`fixtures/fountain.json`, and `script/extract-fountain-data.mjs` extracts the upgrade table. Missing:
the upgrade planner, marble planner and simulator (FN-03 and FN-04).
Details: [`docs/sections/fountain.md`](sections/fountain.md).

The Jar Calculator implements JR-01 to JR-03: the overview and its rupie balances, the shared
production rate and six-tier slot table, the rupie value breakdown with its extra-chance and
per-break payouts, the opal/collectible/enchant chances and their tier scaling, the forty
collectibles grouped by effect with the enchant priority, the nineteen jar schematics and the
jar-type upgrade plan, and the next-tier tab with its pow10 milestone ranking and off-map tier
accumulation. `idleon_core::jar::Jar` reads `Holes`, `Spelunk`, `OptLacc`, `Gaming` and the account
rows; `script/extract-jar-data.mjs` extracts the collectible, schematic and measurement tables, and
`script/jar-oracle.mjs` regenerates `crates/core/tests/jar-oracle.json` from the upstream
`js/stats/systems/w5/jar.js` plus the inline `jar-calc.html` accumulation loop. The upgrade plan
marks only the next jar type affordable and never spends past the rupie balance it is paid from.
Missing: the enchant simulator and history (JR-04).
Details: [`docs/sections/jar.md`](sections/jar.md).

The Royal Guardian section implements RG-01: the overview with the counters (built outposts, worlds
unlocked, Armory levels, stationed units, militia clearing), the world progress cards, the raw
clearing values every unbuilt outpost reports (progress, requirement, remaining, militia, rate, ETA)
and the range target behind an outpost's Advanced Logistics plan. `idleon_core::royal_guardian`
reads `RoyalG` and `RoyalMaps`, `script/extract-royal-data.mjs` extracts the Armory, resource and map
tables, and `script/royal-oracle.mjs` regenerates `fixtures/royal-guardian.json` from the upstream
JS. The clearing rate reads talent 231's purchased level and the shared Jelly Operator completion
bonus; the all-talent bonus levels that raise the talent are not decoded yet, so the rate card says
so. Missing: the outpost map, resource nodes and currencies (RG-02), the upgrades, breakdown tree and
forecast (RG-03) and the optimizer, unit EXP and shelf steps (RG-04).
Details: [`docs/sections/royal-guardian.md`](sections/royal-guardian.md).

The Spelunking section has its controls, party, elixir belt, depth estimates and expedition run
(SP-01, SP-02). `idleon_core::spelunking` reads the tunnel rows, saved stamina, shop upgrades, elixir
unlocks and depth records from the save, normalises the party, belt, mode and policy with the
upstream `normalizeSpelunkConfig` rules and its rejection messages, and computes the Depth limits
profile from `spelunk-depth.js`. The four account multipliers upstream derives from its whole stat
context — Spelunking Power, the amber multiplier, stamina regeneration and the Grand Discovery
chance — stay explicit inputs here; the page carries all four, the extra-exit bonus and each
character's maximum stamina. The section renders the character/cave/mode/policy/full-clear/speed
toolbar, the party with the half-stamina rule, the equipped and memorized belt with the built-in
presets, and the two depth charts with a per-depth inspector. `idleon_core::spelunking::sim` ports
the whole expedition from `spelunk-expedition.js` — the seeded 10 ms tick, floor generation, digging,
the elixir belt with its refills, party switching, exits, the Delve toll and the run log — so the same
inputs and seed reproduce the run; the page runs it through the shared task runner with progress and
cancel, and shows the outcome, rewards, stamina, per-member contributions and the event log. The
phased Amber policy is not ported yet, so `amber-farm` is rejected instead of behaving like another
policy. `fixtures/spelunking.json` is the upstream calculator's own output for the committed save:
four configurations, three whole runs and nineteen rejected inputs, compared field for field by
`crates/core/tests/spelunking.rs`; regenerate it with `script/spelunking-oracle.mjs`, and
`script/extract-spelunk-data.mjs` extracts the cave, rock, tool, elixir, chapter and sprite tables.
Missing: the Amber policies and the optimizer (SP-03, SP-04).
Details: [`docs/sections/spelunking.md`](sections/spelunking.md).

The Opal Optimizer section implements OP-01 and OP-02. The `Current state` tab shows the five
villagers, their levels, invested opals and stars, the six overview figures, the villagers table, the
bonus terms and the shared formula tree, switchable between the total and one villager. The
`Optimizer` tab splits a total across the unlocked villagers for the total EXP or one villager's rate:
the plan comes off the shared task runner, prints the saved and planned rate per villager, sums to the
total and can be copied as tab-separated text. `idleon_core::opal::Opal` decodes `buildExt` out of the
save — the villager rows, the cosmo upgrades and every cross-system term through the account decoder —
and `idleon_core::opal::Opal::plan` searches every lowest count with each villager on that count or the
next multiple of ten, so the plan is never worse than the saved split and at least as good as the
page's own greedy shapes. `script/opal-oracle.mjs` evaluates `opal-calc.html`'s own functions to
regenerate `fixtures/opal.json` (the page's rates and optimizer plans), which
`crates/core/tests/opal.rs` pins. Missing: the manual simulator (OP-03).
Details: [`docs/sections/opal.md`](sections/opal.md).

The Sneaking section has its loadout, inventory and formula breakdown workspaces (SN-01 and SN-02):
`idleon_core::sneaking::Loadout` reads the save's `Ninja` rows — the twin placements, the four
equipment rows per twin, the 39-slot inventory and the Ninja Knowledge levels — and computes the
item stats, the charm bonus map, the Gold inventory scan, action speed, KO recovery, the level,
charm, ally and flower stealth factors and the detection chance they give. Slots swap with the
inventory (two items trade places, so the editor cannot invent a charm), a repeated charm without
the Charmed emporium perk and a second Scroll of Power are flagged, and a save without `Ninja` rows
says so. The saved floor assignments line names each floor's twins, their action and the loadout's
try rate. `idleon_core::sneaking::account` reads the account-wide stack — Land Rank, compass,
statues, cards, achievements, gemstones, lamps, Hole upgrades, Emperor, RoG, Jelly and companions —
and `idleon_core::sneaking::breakdown` builds the five upstream trees: Stealth, Jade per hour, EXP
per action, door damage per hour and the runtime action loop. The breakdowns workspace selects a
twin and a formula tab and renders each tree through the shared formula tree with its search,
expand and copy controls. `script/extract-sneaking-data.mjs` extracts the catalog, the floor tables
and the account tables, and `script/sneaking-oracle.mjs` regenerates `fixtures/sneaking.json` from
the upstream formulas, including the breakdown factor map the account decoder is compared against.
The Gambit point chain and the terms that the All-Talent resolver rescales stay explicit inputs, and
each such node says so in its note. Missing: the placement optimizer (SN-03) and the floor × mastery
table with the door ETA (SN-04).
Details: [`docs/sections/sneaking.md`](sections/sneaking.md).

The Prisma section implements PR-01 and PR-02: the plan for one character and cauldron (every
bubble's saved level, its saved Prisma ownership and the four statuses upstream distinguishes), the
save's Prisma multiplier, the streamed bubble search, the ranked per-metric rows and the coverage
table. Each candidate is the saved state with one bubble's Prisma flag toggled, and every row
compares without → with that bubble's Prisma, so a ready bubble previews the gain and an
already-Prisma bubble measures the benefit it retains. A run measures every purchased bubble's bonus
(the curve at its saved level times the Prisma multiplier, an intermediate) plus the downstream
outcome families this build owns: today `statue-retention`, the Statue EXP the STARTUE EXP bubble
buys, pinned field for field to upstream's `statueLevelingInputs`, and its per-row change is pinned
to `prismaChanges`. The rows rank by any of those metrics and filter by name, effect or changed
outcome; the coverage table lists every purchased effect in the cauldron with the native game
scripts upstream's source audit (`js/stats/data/prisma-source-coverage.js`) traced for it and
whether this build models a downstream metric for it, so the bubble bonus is never presented as a
stat, skill or damage gain. The shared task runner reports progress and keeps the previous result
when a run is cancelled, and a save, character, cauldron or AFK-hours change marks the result stale. Formula parity is pinned by
`crates/core/tests/prisma-oracle.json`, generated by `script/prisma-oracle.mjs` from the upstream
plan, multiplier, bubble bonus and comparison; `script/extract-prisma-data.mjs` extracts the bubble
table and the consumer audit. Missing: the damage, stats and skills outcome families the upstream
evaluator covers beyond the modelled one, and the bubble icons.
Details: [`docs/sections/prisma.md`](sections/prisma.md).

The Monument section's Bravery (MN-01), Justice (MN-02) and Wisdom (MN-03) tabs are built: Paying
Respect training and sources, saved Bravery stats and sword odds for the entered run state, saved
Justice and Wisdom starts, Justice and Wisdom milestones and all three permanent reward sets, and
the reward charge a monument's saved wait pays (plus its 24-hour preview), including the Meritoc
vote, sushi tier and gaming superbit behind it. The character selector supplies the Summoning
access the charge reads.

The Justice tab is a local worksheet, not a live-game connection: it reads the 90 upstream cases,
fills each one's coin and popularity range, lists all three responses with every random outcome
and the share of outcomes that keep the story alive, names the delayed deals the next case settles,
and suggests a response with the survival-first heuristic's reasoning. Every worksheet step (a stat
edit, an effects toggle, a recorded response, an opened case) is undoable, the case search filters
by visitor or request wording, and the worksheet travels in a project document while its undo trail
stays a session convenience. The full permanent/charge summary stays on the roadmap as MN-04.

`fixtures/monument.json` is the upstream calculator's own output for the committed save, including
seven Bravery run states (boost stacks, spent rethrows, an entered last-sword HP), the Wisdom
snapshot, all three reward charges, the Justice snapshot, its fresh-story start, a report for every
one of the 90 cases, the worked states with the deals and promises active, each timer settlement and
the rejected inputs, so `crates/core/tests/monument.rs` compares the port field for field;
regenerate it with `script/monument-oracle.mjs`, and `script/extract-monument-data.mjs` extracts the
tables the tabs read. Details: [`docs/sections/monument.md`](sections/monument.md).

The Cog Board Optimizer implements CG-01 to CG-04: the saved 12×8 board and shelf decode from
`CogO` and `CogM`, the three single-objective runs (build speed, construction EXP and Flaggy rate)
run through the shared task runner with progress and cancel, the optimized placement renders cell by
cell, and Reset restores the save board. The board insights remove each movable cog from a copy of
the board and rescore, so the weakest and strongest cogs, their share of the score and the
replacement targets match the optimizer's own score; the roll ranges show each tier's per-roll and
total bounds at the effective construction level, inferred from the board's own rolls as upstream's
`findMaxConLevel` does, with the level at which each bound rises; and the shelf lists the
optimizer's pool read-only. The optimizer is the upstream greedy-construction, simulated-annealing
and greedy-finish port, with a seeded random source; invariant tests prove it never returns a board
worse than its input and that its swap list rebuilds the result. CG-02 adds custom weights: the
three fields normalise against the save board's own totals and Optimize maximises
`Σ weight × metric / reference`, while negative, non-finite or all-zero weights are rejected with a
banner and never start a run. Perfect Board is the upstream layered-eviction port: perfect T3 base
stats, T3 directionals, crystal fillers and surrounds, Excogia blocks and Yang cogs, rebuilt for
every player slot and kept only when a layer raises the score. It returns the layer list and
perfect stats that explain the board, targets the selected single goal as the upstream active stat
overlay does, and reads the effective construction level the roll ranges use. The tiny-cog board is
not decoded yet, so the perfect board covers the 12×8 board only. A finished run opens the swap
walkthrough at the board it started from: Next applies the swap list in order, Previous rebuilds
the prior board and Done keeps the optimized placement. `fixtures/cog-insights.json` is the
upstream page's own output for the committed save, regenerated by
`script/cog-insights-oracle.mjs`, so the contributions, the roll-target wording and the ranges are
compared field for field. Details:
[`docs/sections/cog.md`](sections/cog.md).

The Farming section's assignment and Land Rank allocation are built (FM-02): the objective selector
re-plans which crop each of the 36 plots should grow, the table names the current and target crop,
family and change per plot and the field tiles carry the target, and the allocator spends the
account's Land Rank points off the UI thread through the shared task runner. The allocator is the
upstream greedy marginal search over the twenty upgrade rows, with the unlock rank and fifth-column
cap rules and the per-objective candidate lists; a run reports progress, cancels without dropping
the previous plan, and an objective, mode or save edit marks the stored plan stale. `fixtures/farming.json`
is the upstream planner's own output for one purpose-built save, including all seven objectives and both
allocation modes, so `crates/core/tests/farming_planner.rs` compares the assignment rows, the
allocator's levels, points and scores, and the account snapshot field for field; regenerate it with
`script/farming-oracle.mjs`. FM-05 adds the upstream Overview tab: the priority signals, the account
multipliers, the GMO thresholds, the ten Crop Scientist bonuses, the seven growth breakpoints and
every plot's long-run overgrowth curve, with the plot selection switching the curve. The account
snapshot applies only the terms the save carries, so every figure is a lower bound;
`script/farming-signals-oracle.mjs` regenerates `fixtures/farming-signals.json` from the upstream
`farming-planner.js` and `farming.js` on a purpose-built save whose unmodelled systems are all
absent, and `crates/core/tests/farming_signals.rs` compares the snapshot, the diagnostics, the
thresholds, the breakpoints, all 36 plots' metrics, all 36 curves and the Crop Scientist table
against it. FM-03 adds the preset comparison and the named presets: the Plots tab runs the
upstream "Preset Comparison" table through the shared task runner, so every goal reports its gain,
the points it spends and the Land Rank rows it raises from the current baseline, and the preset bar
keeps a named goal, baseline and crop target in the project document. The built-in presets are the
page's own seven `OBJECTIVES` goals: `fixtures/farming.json` carries that list and both comparison
tables, so `crates/core/tests/farming_planner.rs` proves every upstream preset exists and each
comparison row matches upstream field for field. The evolution chain with overgrowth advice (FM-04)
stays on the roadmap. Details: [`docs/sections/farming.md`](sections/farming.md).
The Cooking Mastery section's Mastery EXP tab is built (CK-01): the mastery rank, the free points,
the current EXP rate, the EXP the next level takes and the hours that needs, the upgrade comparison
ranked by the gain one more point buys, and the EXP rate breakdown in the shared formula tree. Every
figure comes from the imported save, through `CookMaster`, `Meals[0][73]`, `Ribbon[28..]` and the
upstream external multipliers (grid node 190, superbit 68, companion 87, menagerie 21, the Cook
Maxxing fountain upgrade, the Canteen Read vial, arcade 69 and salt lick 10). The daily distribution
(CK-02) and the yellow point allocator (CK-03) stay on the roadmap. Formula parity against the
upstream page uses `fixtures/cooking-mastery.json`, regenerated by
`script/cooking-mastery-oracle.mjs`. Details:
[`docs/sections/cooking-mastery.md`](sections/cooking-mastery.md).

The Ladle Optimizer implements LD-01 and LD-02: the per-character ladle rates with the claim plan
and its Post Office 19c roll and Prayer 2 cap, the Cooking EXP balloon model with overflow, two
shared formula trees and the daily reset. The Kitchens & Jellyfish tab claims the checked
characters' guaranteed ladles, donates the policy's share to the Blue Jellyfish, spends the rest on
the cheapest meal level (whole ladles first, passive kitchen time after), banks leftover hours into
the meal they were cooking, lets No Meal Left Behind raise its lowest eligible meal, and finally
cooks the reset's grant in every kitchen before buying the levels its stock covers. Each reset's
layout is the upstream `optimizeKitchenReset` search over whole-kitchen assignments, with pins,
meal exclusions, the saved layout and the Dream 11 first-purchase discount, and each reset expands
into its spend order and a copy button with the same instructions as plain text. The account terms
the upstream stats engine resolves are entered instead: the ladle panel values, the kitchen speed
at Speed Lv 0 and the meal max level stand in for the Cooking speed model, and the meal cost terms
(companion 162, achievement 233, cloud 33, Dream days) are the Cooking menu's discount inputs.
`script/ladle-kitchen-oracle.mjs` regenerates `crates/core/tests/ladle-kitchen-oracle.json` from the
upstream module, with a model whose account multipliers are neutral so every rate is the port's
`base × (1 + Speed Lv / 10) × (1 + gem doubler)`; `script/extract-cooking-data.mjs` extracts
`MealINFO` and the `N2L` encoding. The forecast comparison and the meal outcome tables (LD-03) and
the Blue Jellyfish donation planner with its task runner (LD-04) stay on the roadmap.
Details: [`docs/sections/ladle.md`](sections/ladle.md).

The Jelly Operator section implements JO-01 and JO-02: the overview, the layout editor and the
operation simulator. The section decodes the save's own board (`Research[14]`), its progress row,
the cell EXP and levels, the upgrade levels and the plot tokens, then reads the account systems the
page multiplies (research grid nodes 185-187, Spelunking palette 1, Legend Talent 41, the Bloodclot
artifact, arcade 72, Atom 15, the `ban_j` bundle, event shop 56/57, Sushi's `rogBonusQty(63)` and
Menagerie's Upgrade Doubler row). The layout tab draws the 18×10 board with the upstream cell and
obstruction sprites, the legend, the placement rules (`jellyValidateLayout`, `jellyPlaceCell`,
`jellyRemoveCell`) and a palette that places every cell type the save unlocked; a click on a placed
cell removes it and Reset restores the saved board. The overview shows the account snapshot and runs
the seeded operation estimate through the shared task runner: `simulateJellyOperation` ported frame
for frame (projectiles, the one-second Best DPS callback, Fever, Stronkroid, boss attacks and
revives), its Monte Carlo aggregate, the cell contribution table, the critical target timeline and
the progression outlook, whose obstruction rows come from the deterministic surrogate.
`script/extract-jelly-data.mjs` writes the cell, plot and upgrade tables; `script/jelly-oracle.mjs`
regenerates `fixtures/jelly.json` from the upstream JS for the committed save and three documented
overrides, and `crates/core/tests/jelly.rs` compares the state, the multipliers, the layout metrics,
the estimate and the outlook field for field. The simulator tab is a sandbox copy of the same slice:
the stage and Fever selects, Stronkroid's activation second, the automatic revive policy, the seeded
single operation through the shared task runner, the palette with remove mode, Clear cells and Reset
from save, and hypothetical board sections that reflow the board through `board::normalize` (the
page's `normalizeSimulatorLayout`). A run shows the outcome cards, the per-cell contribution table
and the critical target timeline. `script/jelly-oracle.mjs` also writes two seeded single operations
(Stronkroid at 5s with `dps-first` revives, and an empty sandbox board), and
`crates/core/tests/jelly-estimate.rs` compares every trial field. The upgrades, bonuses and
breakdowns (JO-03), the section planner (JO-04) and the coordinator optimizer (JO-05) stay on the
roadmap. The upstream page's shape overrides for grid nodes 185 and 187 are not ported: they correct
a save-export timing bug from the page's own local storage, and this app reads the save's shape
overlay. Details:
[`docs/sections/jelly-operator.md`](sections/jelly-operator.md).

The Coral God Rank section implements CR-01, CR-02 and CR-03: the upstream planner controls with
validation and the save's current coral state, the bounded deterministic beam search through the
shared task runner, then the offering fit and the current Coral Kid levels. A run keeps the best plan
per beam, reports each step, can be cancelled and marks the stored plan stale when an input changes.
The card shows what the plan commits, the projected coral state against the current one and the
ordered Coral Kid and God Rank offering actions. The search is `idleon_core::coral::plan`: every
unlocked upgrade and one offering per state, candidates that do not strictly improve the goal
dropped, states deduped by level/rank signature with dominance pruning, then sorted by the goal,
spend and signature before the beam is cut. The offering fit reads the upstream `offeringStats` row
at the current God Rank: the tier's success chance, the paid cost and `expectedSpendForOneSuccess`,
with a caption that turns the Y4 free chance and the planning confidence into the attempts and
worst-case spend the plan reserves. The current-levels table adds each upgrade's unlock status, and
both cards name the save fields they read as zero (Y4, DivCostAfter3, and the Coral Kid and
Singularity level pages) instead of hiding the gap.
`script/coral-god-rank-oracle.mjs` regenerates the upstream plans in `fixtures/coral-plan.json`,
including the beam-dependent case and offering rows with and without a Y4 free chance, and
`crates/core/tests/coral_plan.rs` compares the plan action by action and the fit figures. Details:
[`docs/sections/coral-god-rank.md`](sections/coral-god-rank.md).

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
