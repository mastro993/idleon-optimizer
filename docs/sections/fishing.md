# Fishing Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M10`
- Section key: `fishing`
- Group: `w2`
- Status: `planned`
- Depends on: `foundation`

Port AFK catches, fish split, real vs panel EXP, best lure and line per pool, depth projection and all 225 combinations.

## Upstream sources

- fishing-calc.html
- js/stats/systems/w2/fishing.js
- js/stats/data/w2/fishing.js
- js/stats/tree-builder.js

## UI

- Toolbar and character context
- Depth projection
- Pool cards with best lure/line
- All-combination table
- Other characters

## Inputs

- Character save
- Fishing level/toolkit
- Pool/depth selection

## Outputs

- AFK catches
- Fish split
- Real vs panel EXP
- Best lure and line
- Depth projection

## Assets

Fishing icons if present.

## Feature issues

### FI-01 — AFK catches and fish split

Catch rate, fish distribution and real-versus-panel EXP per pool.

Depends on: FND-03 Shared save JSON import, FND-11 Shared formula tree and breakdown component

Acceptance:

- Values match upstream.
- Pool switching updates results.
- Missing toolkit is flagged.

### FI-02 — Best lure and line per pool

Rank every lure/line combination for the selected pool and depth.

Depends on: FI-01 AFK catches and fish split

Acceptance:

- All 225 combinations are evaluated.
- Best combination is highlighted.
- Ties are explained.

### FI-03 — Depth projection and other characters

Project fishing depth and show other characters fishing the Trench.

Depends on: FI-01 AFK catches and fish split

Acceptance:

- Projection matches upstream.
- Character contributions sum correctly.
- Empty roster shows a state.

## Verification

- 225-combination fixture.
- Depth projection test.
- Harness.
