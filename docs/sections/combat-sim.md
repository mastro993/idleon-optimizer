# Combat Simulator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M2`
- Section key: `combat-sim`
- Group: `general`
- Status: `partial`
- Depends on: `foundation`

Port the active combat simulator: loadout editor, mechanics overrides, component terms, encounter simulation, comparisons, Killroy, obols, cards, kills-over-time and replays.

## Current build

- CS-01 ships the loadout editor: the character picker, the 16 armor and 7 tool slots with their saved `EMm` upgrade deltas, and the talent table read from `SL_<n>`/`SM_<n>` (`readEquipment`, `readTalents` in `combat-sim-setup.js`).
- Item pickers read the extracted `crates/combat/data/combat.json` catalog (871 wearable items) and filter by the upstream `ARMOR_TYPES`/`TOOL_TYPEGEN` slot rules; `script/extract-combat-data.mjs` regenerates it from the upstream `ITEMS` table.
- Every edit recomputes the gear totals the header shows: weapon power, defence, the class main stat and the summed talent points, so the summary always matches the editor. It is the simulator's input preview; the encounter run reports monster damage and kill outcomes, while character DPS waits for the auto-mode player.
- Missing or invalid equipment is flagged: an item key outside the catalog, an item in the wrong slot family, a level or class requirement the character misses, and a saved talent level above its saved maximum.
- Named loadouts are presets of item keys and talent levels. They live in the project JSON under `sections.combat-sim` and survive an export/import round trip; applying one replaces the working loadout like equipping each slot by hand.
- CS-02 ships the mechanics overrides: the upstream `COMBAT_TOGGLES` catalog (68 mechanics in 11 groups, `combat-sim-toggles.js`) is extracted into `crates/combat/data/combat-overrides.json` by `script/combat-overrides-oracle.mjs`, and `crates/combat/src/overrides.rs` keeps one rule per mechanic id, normalizes it the way `normalizeToggleState` does and resolves it against a run's saved inputs (`off` writes the neutral value, `set` the typed value, `scale` saved × value, the flags with their own keys, the per-monster object, and `raw:<input>` ids for any other input).
- The Mechanics tab lists the catalog with a search field, a group filter and an only-changed filter; each row picks Saved, Off, On, Set or Scale × and a value, the active rules show as removable chips above the tabs, and Reset all puts every input back to the save. The overrides live in `sections.combat-sim` under `toggles`, so a project export carries them.
- Every override case is compared with the upstream module's own output through `fixtures/combat-overrides.json`; the catalog and that fixture are written from one upstream checkout, so they cannot drift apart.
- CS-03 ships the encounter run: `crates/combat/src/encounter.rs` replays the game's fixed 10 ms step, its render-frame batch and its timed-task loop (`combat-sim-core.js`) over one map's spawn roster, then the spawn, one-second respawn tick, `whenDrawing` respawn and kill handlers (`combat-sim-world.js`). The two players upstream ships without its AI module are ported exactly: the fixed kill interval stand-in (`scriptedPlayer`) and the idle player.
- `crates/combat/data/encounters.json` holds every map whose scene the run supports (124 spawn rosters with their base monster HP and respawn time), and `script/combat-encounter-oracle.mjs` writes it with `fixtures/combat-encounter.json` from one upstream checkout. The fixture runs the upstream engine's own stand-in and idle players at neutral monster tuning; the Rust test compares kills, damage, kills by monster, respawn counters, steps and frames, so the port cannot drift from its source.
- The Encounter tab picks a map, the stand-in or idle player, the kill interval and the 1-minute to 24-hour duration, then runs through the shared task runner: progress, Cancel, the cancelled notice and the stale marker are the shared ones, and a cancelled run keeps the previous result. The result card reports kills, damage, damage per second, kills per hour, the mean kill interval, and the kills by monster.
- The mechanics the run cannot model are listed under the options instead of being dropped: the auto-mode player AI (so no character DPS or survival yet), the reward engine (EXP, drops, cards, obols, coins, inventory), crystal/giant/Plunder spawns, the account monster tuning, Killroy, The Den, the Rift, caverns, Clam Work and dungeons, and replays.
- CS-04 ships the comparison: `crates/combat/src/compare.rs` plans one row per character, loadout and map (`characterMapComparePlan`), runs the same encounter on every row, reuses a map's outcome because the stand-in run reads no loadout, and gives every metric its paired delta against the first row (`setupCompareCell`). `script/combat-compare-oracle.mjs` writes `fixtures/combat-compare.json` from the upstream module, so the delta maths cannot drift from its source.
- The Compare tab picks characters, their imported save, working edits and named loadouts, and the maps to run. The result table shows the run's kills per hour, damage per second and mean kill interval beside the setup's weapon power, defence, main stat and talent points, each with its delta against the baseline row; Export copies the comparison document as JSON.
- Neutral this build: the auto-mode player AI and its component-term trace, the saved values Scale multiplies, the project-to-run wiring of the mechanics overrides, the result tabs (CS-05 and CS-06) and replays (CS-07).

## Upstream sources

- combat-sim-calc.html
- js/stats/systems/common/combat-sim-*.js
- js/ui/combat-sim-*.js
- assets/combat-sim

## UI

- Loadout editor and mechanics overrides
- Component terms and simulator options
- Encounter simulation workspace
- Results tabs: comparison, Killroy, obols, kills by monster, cards, kills over time, The Den, combat-driven progress, smelting

## Inputs

- Character save data
- Loadout and equipment
- Mechanics overrides
- Encounter and map selection
- Simulator options

## Outputs

- DPS/TTK and kill outcomes
- Comparison tables
- Killroy and card progress
- Replay data
- Warnings for unsupported mechanics

## Assets

assets/combat-sim (maps, mobs, player, bullets, items, UI); 38 MB of committed artwork.

## Feature issues

### CS-01 — Loadout editor

Edit character, equipment, talents and loadout presets with save-backed defaults.

Depends on: FND-01 Dashboard card grid, FND-03 Shared save JSON import, FND-08 Shared layout, theme and responsive rules, FND-09 Empty, loading, error and stale states

Acceptance:

- Loadout changes update the simulation immediately.
- Invalid or missing items are flagged.
- Presets save in project JSON.

### CS-02 — Mechanics overrides

Override combat mechanics and component terms with searchable groups and reset-to-save behavior.

Depends on: CS-01 Loadout editor

Acceptance:

- Every upstream override is present.
- Overrides are visible as active chips.
- Reset restores save values.

### CS-03 — Encounter simulator

Run active combat against the selected encounter with the upstream options and progress reporting.

Depends on: CS-02 Mechanics overrides, FND-12 Background task runner with progress and cancel

Acceptance:

- Runs can be cancelled.
- Results match upstream fixture outcomes.
- Unsupported mechanics are listed, not silently ignored.

### CS-04 — Character, loadout and map comparison

Side-by-side comparison across characters, loadouts and maps.

Depends on: CS-03 Encounter simulator

Acceptance:

- Comparison selects two or more setups.
- Deltas are shown per metric.
- Export copies the comparison as JSON.

### CS-05 — Killroy, obols, cards and kills

Killroy encounter results, obol fragments, kills by monster, card drops and kills-over-time chart.

Depends on: CS-03 Encounter simulator, FND-11 Shared formula tree and breakdown component

Acceptance:

- Each result tab renders from the same run.
- Charts match the simulated timeline.
- Empty runs show a clear state.

### CS-06 — The Den, progress and smelting

Combat-driven progress, The Den outcomes and smelting output panels.

Depends on: CS-03 Encounter simulator

Acceptance:

- Panels match upstream outputs.
- Missing inputs are warned.
- Values feed the comparison export.

### CS-07 — Replays and combat assets

Play, scrub and speed-control combat replays using the upstream sprites and maps.

Depends on: CS-03 Encounter simulator, FND-06 Upstream asset pipeline

Acceptance:

- Replay loads from a run or Killroy replay.
- Scrub, speed and wave controls work.
- Assets resolve through the manifest with placeholders.

## Verification

- The loadout readers match upstream `readEquipment`/`readTalents` on the committed synthetic save.
- The item catalog covers every equipped item in that save and every slot has picker options.
- Presets round-trip through the project JSON and refuse another character's loadout.
- The section harness imports a save, edits an item and a talent, flags an unknown item and saves a preset.
- The override catalog and every resolve case match the upstream module through `fixtures/combat-overrides.json`.
- The section harness sets a mechanic off, sees its chip and resets every override.
- The encounter run matches the upstream engine's kills, damage, kills by monster and respawn counters through `fixtures/combat-encounter.json`.
- A cancelled encounter run stores nothing, and the section harness runs a map from the Encounter tab and reads the result.
- Every comparison cell matches the upstream `setupCompareCell` through `fixtures/combat-compare.json`, and the section harness compares two characters and copies the comparison JSON.
