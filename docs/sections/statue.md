# Statue Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M9`
- Section key: `statue`
- Group: `general`
- Status: `planned`

Port statue levels, requirements per level, STARTUE bubble, time estimates and batch apply/farming.

## Upstream sources

- statue-calc.html
- js/stats/systems/w1/stamp.js
- js/stats/systems/w1/statue.js

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

Acceptance:

- All statues render.
- Search/sort work.
- Requirements match upstream.

### ST-02 — STARTUE bubble and time estimates

Apply the STARTUE EXP bubble and estimate time from the selected farming rate.

Acceptance:

- Bubble changes estimates.
- Rate source is shown.
- Estimates match upstream.

### ST-03 — Set all, apply all and farming

Batch set/apply controls and the statue-farming breakdown.

Acceptance:

- Batch actions are undoable.
- Farming breakdown matches upstream.
- Invalid values are rejected.

## Verification

- Fixture matches upstream.
- Batch undo test.
