# Statue Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M9`
- Section key: `statue`
- Group: `general`
- Status: `partial`
- Depends on: `foundation`

Port statue levels, requirements per level, STARTUE bubble, time estimates and batch apply/farming.

## Current build

- ST-01 statue table: the 32-row table with saved levels, next-level bars, availability, search and sort.
- ST-02 STARTUE bubble and time estimates: the decoded STARTUE EXP bubble, Skinny 0 Cal vial, Onyx/Zenith tier and Royal Guardian Flair drive the deposit estimate, the per-row target, statues needed and the time from a raw statues/hour rate, with the rate source shown.
- ST-03 batch actions and the farming breakdown: the upstream Set targets entry (costed for every statue before it writes the plan, with the upstream rejection message), Next level for all and a one-step undo of the batch action; the shared formula tree behind one statue's row — the STARTUE bubble and its retention, the Onyx/Zenith tier, the saved Royal Guardian Flair and the Skinny 0 Cal vial, then the statues needed, raw rate and time. The upstream "Measure statue farming" panel needs the combat simulator (CS-03), so rates stay manual entries.

## Upstream sources

- statue-calc.html
- js/stats/systems/common/statue-leveling.js
- js/stats/data/common/statues.js
- js/save/loader.js

## UI

- Character selector
- Statue table with search/sort
- Next-level and availability columns
- Farming rate and breakdown

## Inputs

- Character save
- Statue levels
- Farming method/rate

## Outputs

- Statues needed per level
- Time estimates
- Batch apply

## Assets

Statue icons if present in upstream assets.

## Feature issues

### ST-01 — Statue table and requirements

Per-statue current level, next-level requirement and availability with search and sort.

Depends on: FND-03 Shared save JSON import

Acceptance:

- All statues render.
- Search/sort work.
- Requirements match upstream.

### ST-02 — STARTUE bubble and time estimates

Apply the STARTUE EXP bubble and estimate time from the selected farming rate.

Depends on: ST-01 Statue table and requirements

Acceptance:

- Bubble changes estimates.
- Rate source is shown.
- Estimates match upstream.

### ST-03 — Set all, apply all and farming

Batch set/apply controls and the statue-farming breakdown.

Depends on: ST-01 Statue table and requirements, FND-11 Shared formula tree and breakdown component

Acceptance:

- Batch actions are undoable.
- Farming breakdown matches upstream.
- Invalid values are rejected.

## Verification

- Fixture matches upstream.
- Batch undo test.
