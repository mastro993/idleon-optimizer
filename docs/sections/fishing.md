# Fishing Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M10`
- Section key: `fishing`
- Group: `w2`
- Status: `done`
- Depends on: `foundation`

Port AFK catches, fish split, real vs panel EXP, best lure and line per pool, depth projection and all 225 combinations.

## Current build

- FI-01 AFK catches and fish split: the character/toolkit/location toolbar, the every-location table, the selected pool split, real-versus-panel EXP and the shared formula tree, with the upstream pure formulas pinned against an oracle fixture.
- FI-02 Best lure and line per pool: all 225 lure/line combinations ranked for the selected pool and depth, switchable between Fishing EXP and fish quantity, with the selected and saved rows marked, a Use action, the tie explanation and the best-combination KPI; the ranking is pinned against an oracle fixture of every row.
- FI-03 Depth projection and other characters: the Trench card projects the next depths from the saved cavern rows, with the jar and Bolaia requirement reductions, the efficiency requirements each depth raises, the selected and fish-maximizing lure + line hours per depth, cumulative totals, and a helper table of the other characters fishing the Trench whose ticked fish per hour sum into the team ETA; the requirement and every projected row are pinned against an oracle fixture evaluated from the upstream module.
- The account skill values (efficiency, EXP multiplier, AFK rate, Multi-Fish, speed sources, prowess and equipment depths) are entered in the account card until the stats dashboard decodes them; the save supplies the character, rod, saved toolkit, spot and AFK target, and the Trench projection also reads the cavern rows (`Holes`, `Spelunk`). The ranking and the per-depth fastest search hold efficiency and the EXP multiplier fixed across combinations, because the upstream recomputes them from the toolkit's POW and EXP through the skill systems.

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
