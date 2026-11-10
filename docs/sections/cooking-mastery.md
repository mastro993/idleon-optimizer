# Cooking Mastery

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M16`
- Section key: `cooking-mastery`
- Group: `w4`
- Status: `planned`

Port Mastery EXP/hr, upgrade comparison, optimal daily distribution and the yellow point allocator.

## Upstream sources

- cooking-mastery-calc.html
- js/stats/systems/w4/cooking.js
- js/stats/systems/common/meal-impact.js

## UI

- Mastery EXP tab
- Upgrade comparison
- Optimal distribution
- EXP rate breakdown
- Yellow point allocator

## Inputs

- Character save
- Meal levels and upgrades
- Yellow point budget
- Goal

## Outputs

- EXP/hr
- Upgrade ranking
- Point allocation
- Breakdown tree

## Assets

Meal icons if present.

## Feature issues

### CK-01 — Mastery EXP and upgrade comparison

Current Mastery EXP/hr, upgrade comparison and EXP rate breakdown.

Acceptance:

- Values match upstream.
- Upgrade ranking is stable.
- Breakdown sums to the total.

### CK-02 — Optimal daily distribution

Allocate the daily reset budget across upgrades for the selected goal.

Acceptance:

- Allocation respects the budget.
- Goal switch changes the plan.
- Plan is copyable.

### CK-03 — Yellow point allocator

Spend yellow points across characters/meals with a goal selector.

Acceptance:

- Budget is respected.
- Per-goal results are shown.
- Invalid budgets are rejected.

## Verification

- Fixture matches upstream.
- Allocator invariant.
- Harness.
