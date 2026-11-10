# Combat Simulator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M2`
- Section key: `combat-sim`
- Group: `general`
- Status: `planned`
- Depends on: `foundation`

Port the active combat simulator: loadout editor, mechanics overrides, component terms, encounter simulation, comparisons, Killroy, obols, cards, kills-over-time and replays.

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

assets/combat-sim (maps, mobs, player, bullets, items, UI); 38 MB, local-only until licensed.

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

- Combat fixtures match upstream damage and kill counts.
- Replay harness takes a screenshot.
- Asset manifest covers every referenced file.
