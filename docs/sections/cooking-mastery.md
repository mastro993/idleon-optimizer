# Cooking Mastery

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M16`
- Section key: `cooking-mastery`
- Group: `w4`
- Status: `done`
- Depends on: `foundation`

Port Mastery EXP/hr, upgrade comparison, optimal daily distribution and the yellow point allocator.

## Current build

- CK-01 Mastery EXP and upgrade comparison: the mastery rank, free points, EXP rate and EXP to the next level with the hours it takes, the five EXP upgrades ranked by what one more point buys, and the whole rate as a formula tree, all decoded from the save and pinned against the upstream page's own numbers.
- CK-02 Optimal daily distribution: `optimizeDistribution` re-plans the daily budget one point at a time onto the upgrade with the biggest marginal multiplier, for the rate at the current rank or the rate the same points buy at the rank that unlocks the next upgrade; the plan is copyable as text.
- CK-03 Yellow point allocator: `optimizeForGoal` spreads the account's yellow points across the meals of the selected goal — meal level, per-level bonus, the meal multiplier and each meal's ribbon factor — with the page's 44 goals, the character context the stat goals scale with, the kitchen aggregate, and a clamped budget that is rejected rather than overspent.

## Upstream sources

- cooking-mastery-calc.html
- js/stats/systems/common/cooking.js
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

Depends on: FND-03 Shared save JSON import, FND-11 Shared formula tree and breakdown component

Acceptance:

- Values match upstream.
- Upgrade ranking is stable.
- Breakdown sums to the total.

### CK-02 — Optimal daily distribution

Allocate the daily reset budget across upgrades for the selected goal: the EXP rate at the current mastery rank, or the rate the same points buy at the rank that unlocks the next upgrade.

Depends on: CK-01 Mastery EXP and upgrade comparison

Acceptance:

- Allocation respects the budget.
- Goal switch changes the plan.
- Plan is copyable.

### CK-03 — Yellow point allocator

Spend yellow points across characters/meals with a goal selector.

Depends on: CK-01 Mastery EXP and upgrade comparison

Acceptance:

- Budget is respected.
- Per-goal results are shown.
- Invalid budgets are rejected.

## Verification

- Fixture matches upstream.
- Allocator invariant.
- Harness.
