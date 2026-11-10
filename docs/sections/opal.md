# Opal Optimizer

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M23`
- Section key: `opal`
- Group: `w5`
- Status: `planned`

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

Acceptance:

- Values match upstream.
- Villager switching works.
- Breakdown sums.

### OP-02 — Opal optimizer

Optimize opal distribution for the selected target and total.

Acceptance:

- Distribution respects totals.
- Result is never worse.
- Plan is copyable.

### OP-03 — Manual opal simulator

Manual distribution inputs with simulated results.

Acceptance:

- Manual result matches optimizer when equal.
- Inputs validate.
- Reset works.

## Verification

- Fixture matches upstream.
- Optimizer invariant.
- Harness.
