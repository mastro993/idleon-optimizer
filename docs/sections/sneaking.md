# Sneaking Optimizer

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M24`
- Section key: `sneaking`
- Group: `w6`
- Status: `planned`

Port loadout/inventory, stealth/jade/EXP/door/runtime breakdowns, optimizer, floor mastery and door ETA.

## Upstream sources

- sneaking-calc.html
- js/ui/sneaking-*.js
- js/stats/systems/w6/sneaking.js
- assets/sneaking

## UI

- Loadout and inventory workspaces
- Formula breakdown tabs
- Optimizer
- Floor mastery
- Saved floor assignments
- Active door and next captive

## Inputs

- Character save
- Charms and ninja kit
- Floor/objective
- Item batch hours

## Outputs

- Stealth/detection
- Jade and EXP rates
- Recommended placement
- Door ETA

## Assets

assets/sneaking charms, items, scenes and borders.

## Feature issues

### SN-01 — Loadout and inventory workspaces

Loadout/inventory editors backed by the save with saved placement output.

Acceptance:

- Edits update formulas.
- Invalid charms are flagged.
- Workspace persists.

### SN-02 — Formula breakdowns

Stealth, jade, EXP, door and runtime breakdown trees.

Acceptance:

- Every tab renders.
- Trees sum to totals.
- Search works.

### SN-03 — Sneaking optimizer

Optimize charm placement for the selected objective with progress and cancel.

Acceptance:

- Result beats baseline.
- Cancel restores state.
- Objective switch works.

### SN-04 — Floor mastery and door ETA

Floor-by-mastery table, active door, next captive and untie ETA.

Acceptance:

- Mastery values match upstream.
- ETA updates with loadout.
- Empty floor shows a state.

## Verification

- Fixture matches upstream.
- Optimizer invariant.
- Harness with assets.
