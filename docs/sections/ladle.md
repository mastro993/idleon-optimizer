# Ladle Optimizer

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M15`
- Section key: `ladle`
- Group: `w4`
- Status: `done`
- Depends on: `foundation`

Port ladles/day, Cooking EXP balloons, meal costs, kitchen reset planning, forecast comparison and the NMLB planner.

## Upstream sources

- ladle-calc.html
- js/stats/systems/w4/cooking-ladle.js
- js/stats/tree-builder.js

## UI

- Summary and character selectors
- Breakdowns
- Kitchens and Jellyfish reset layout
- Ladle farmers
- Forecast comparison
- Meal outcomes
- NMLB planner

## Inputs

- Character save
- Ladle ownership
- Days/resets
- Balloon target
- Kitchen objective/policy

## Outputs

- Ladles/day
- Cooking EXP
- Meal costs
- Reset instructions
- NMLB targets

## Assets

Cooking icons if present.

## Feature issues

### LD-01 — Ladle summary and breakdowns

Per-character ladle uses, rates and Cooking EXP balloon breakdown.

Depends on: FND-03 Shared save JSON import, FND-11 Shared formula tree and breakdown component

Acceptance:

- Values match upstream.
- Character switching works.
- Balloon overflow is handled.

### LD-02 — Kitchens and Jellyfish reset

Daily kitchen layout, reset investment and expandable reset instructions.

Depends on: LD-01 Ladle summary and breakdowns

Acceptance:

- Reset plan matches inputs.
- Instructions are copyable.
- Days range validates.

### LD-03 — Forecast comparison and meal outcomes

Compare objectives across reset days and show meal targets/outcomes.

Depends on: LD-01 Ladle summary and breakdowns

Acceptance:

- Objective switch changes ranking.
- Meal costs use the discount.
- Results match upstream.

### LD-04 — NMLB donation planner

Blue Jellyfish planner for kitchen speed after N resets, including predicted rotation.

Depends on: LD-03 Forecast comparison and meal outcomes, FND-12 Background task runner with progress and cancel

Acceptance:

- Planner finds the best target.
- Rotation is shown.
- Cancel restores state.

## Verification

- Fixture matches upstream.
- Planner determinism.
- Harness.
