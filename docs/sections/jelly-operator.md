# Jelly Operator Optimizer

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M29`
- Section key: `jelly-operator`
- Group: `w7`
- Status: `planned`

Port cell layouts, obstruction clears, Bloodcells, upgrades, account bonuses, section planning and the coordinator optimizer.

## Upstream sources

- jelly-operator-calc.html
- js/stats/systems/w7/jelly-operator.js
- js/optimizers/jelly-coordinator.js
- assets/jelly-operator

## UI

- Overview
- Layout editor
- Simulator
- Upgrades
- Bonuses
- Breakdowns
- Sections planner
- Optimizer

## Inputs

- Save jelly state
- Cell palette/layout
- Section strategy/objective
- Tokens/quality/beam

## Outputs

- Operation estimate
- Cell contribution
- Critical timeline
- Section plan
- Optimizer result

## Assets

assets/jelly-operator cells and board art.

## Feature issues

### JO-01 — Layout editor and overview

Cell palette, board sections, saved layout and operation estimate.

Acceptance:

- Every cell type is placeable.
- Saved layout loads.
- Estimate matches upstream.

### JO-02 — Operation simulator

Run the interactive simulator with fever, roid, revive, run and remove mode.

Acceptance:

- Simulation is deterministic.
- Timeline renders.
- Clear board works.

### JO-03 — Upgrades, bonuses and breakdowns

Upgrade list, account snapshot, bonuses and per-cell breakdowns.

Acceptance:

- Values match upstream.
- Breakdowns sum.
- Missing data warned.

### JO-04 — Sections planner

Plan board sections with strategy, objective, cell type, tokens, quality and beam size.

Acceptance:

- Planner respects section rules.
- Beam size changes search.
- Plan is reproducible.

### JO-05 — Coordinator optimizer

Optimize the full board with the coordinator and show critical target timeline.

Acceptance:

- Optimizer beats baseline.
- Cancel works.
- Result applies to layout.

## Verification

- Fixture matches upstream.
- Optimizer invariant.
- Harness with assets.
