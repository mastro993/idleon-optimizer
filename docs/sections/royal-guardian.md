# Royal Guardian Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M27`
- Section key: `royal-guardian`
- Group: `w7`
- Status: `done`
- Depends on: `foundation`

Port outposts, resource nodes, currencies, upgrades, breakdowns, forecasts, optimizer, unit EXP and shelf layout.

## Upstream sources

- royal-guardian-calc.html
- js/stats/systems/w7/royal-guardian*.js
- assets/royal-guardian

## UI

- Overview with counters
- Outposts and resource nodes
- Currencies
- Upgrades
- Breakdowns
- Forecast
- Optimizer
- Unit EXP and shelf

## Inputs

- Save royal state
- Outpost/resource selection
- Drain staffing
- Forecast days

## Outputs

- Clearing values
- Resource/currency rates
- Upgrade plan
- Forecast
- Unit EXP moves

## Assets

assets/royal-guardian maps, resources, units and lines.

## Feature issues

### RG-01 — Overview and counters

Raw clearing values, range targets, world progress and counters.

Depends on: FND-03 Shared save JSON import, FND-06 Upstream asset pipeline

Acceptance:

- Counters match save.
- Range target is actionable.
- World progress renders.

### RG-02 — Outposts, resources and currencies

Outpost map, resource nodes, currencies, drain staffing and banked hours.

Depends on: RG-01 Overview and counters

Acceptance:

- Every outpost renders.
- Drain projection works.
- Resource totals match upstream.

### RG-03 — Upgrades, breakdowns and forecast

Upgrade list, global breakdown tree and forecast tab.

Depends on: RG-02 Outposts, resources and currencies, FND-11 Shared formula tree and breakdown component

Acceptance:

- Breakdown sums.
- Forecast matches upstream.
- Upgrade plan respects costs.

### RG-04 — Optimizer, unit EXP and shelf

Outpost/unit optimizer, unit EXP moves and shelf layout steps.

Depends on: RG-03 Upgrades, breakdowns and forecast, FND-12 Background task runner with progress and cancel

Acceptance:

- Optimizer is cancellable.
- Unit EXP moves are legal.
- Shelf steps apply in order.

## Verification

- Fixture matches upstream.
- Optimizer invariant.
- Harness with assets.
