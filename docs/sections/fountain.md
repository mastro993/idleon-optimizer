# Fountain Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M17`
- Section key: `fountain`
- Group: `w5`
- Status: `planned`
- Depends on: `foundation`

Port fountain overview, currency values, upgrades, planner, marble planner, simulation, lucky coins/ducks and cost curves.

## Upstream sources

- fountain-calc.html
- js/stats/systems/w5/fountain.js

## UI

- Overview and income rates
- Currency and multipliers
- Upgrades and cost curve
- Planner/optimizer
- Marble planner
- Simulation

## Inputs

- Save fountain state
- Water/currency selection
- Planner goal and budget

## Outputs

- Income rates
- Currency values
- Upgrade plan
- Marble plan
- Simulation results

## Assets

Fountain icons if present.

## Feature issues

### FN-01 — Overview and income rates

Fountain summary, income rates, lucky coins/ducks, coin stacks and cross-system bonuses.

Depends on: FND-03 Shared save JSON import, FND-11 Shared formula tree and breakdown component

Acceptance:

- Values match upstream.
- Missing bonuses are warned.
- Rates update with the save.

### FN-02 — Currency values and upgrades

Per-water multipliers, currency values, upgrade list and cost curve.

Depends on: FN-01 Overview and income rates

Acceptance:

- Currency switching works.
- Cost curve matches upgrades.
- Max-level state is clear.

### FN-03 — Planner and optimizer

Plan upgrades for a goal with affordability and per-hour normalization.

Depends on: FN-02 Currency values and upgrades, FND-12 Background task runner with progress and cancel

Acceptance:

- Plan respects budget.
- Goal switch changes ranking.
- Steps are listed.

### FN-04 — Marble planner and simulation

Marble upgrade planner and the fountain simulation tab.

Depends on: FN-03 Planner and optimizer

Acceptance:

- Marble plan is explainable.
- Simulation is deterministic.
- Cancel restores state.

## Verification

- Fixture matches upstream.
- Planner invariant.
- Harness.
