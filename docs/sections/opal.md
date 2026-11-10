# Opal Optimizer

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M23`
- Section key: `opal`
- Group: `w5`
- Status: `planned`
- Depends on: `foundation`

Port villager EXP rates, opal distribution optimizer, manual simulator and all bonus terms.

## Upstream sources

- opal-calc.html
- js/stats/systems/w5/opal.js

## UI

- Overview and villagers
- Current state
- Optimizer
- Manual simulator
- Formula breakdown

## Inputs

- Villager levels/EXP
- Opal totals/target
- Manual distribution

## Outputs

- EXP rates
- Optimal distribution
- Manual simulation
- Bonus terms

## Assets

Villager icons if present.

## Feature issues

### OP-01 — Villagers and current state

Villager EXP rates, current opal state and formula breakdown.

Depends on: FND-03 Shared save JSON import, FND-11 Shared formula tree and breakdown component

Acceptance:

- Values match upstream.
- Villager switching works.
- Breakdown sums.

### OP-02 — Opal optimizer

Optimize opal distribution for the selected target and total.

Depends on: OP-01 Villagers and current state, FND-12 Background task runner with progress and cancel

Acceptance:

- Distribution respects totals.
- Result is never worse.
- Plan is copyable.

### OP-03 — Manual opal simulator

Manual distribution inputs with simulated results.

Depends on: OP-02 Opal optimizer

Acceptance:

- Manual result matches optimizer when equal.
- Inputs validate.
- Reset works.

## Verification

- Fixture matches upstream.
- Optimizer invariant.
- Harness.
