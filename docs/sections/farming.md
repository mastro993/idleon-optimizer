# Farming Planner

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M25`
- Section key: `farming`
- Group: `w6`
- Status: `planned`

Port the 9x4 field, plot assignment, Land Rank allocation, evolution/overgrowth, presets, markets and growth curves.

## Upstream sources

- farming-calc.html
- js/stats/data/w5/farming.js
- js/stats/systems/w6/farming.js
- js/stats/systems/w6/farming-planner.js

## UI

- Priority signals
- Account multipliers
- GMO thresholds
- Crop Scientist bonuses
- Growth breakpoints
- Full field and plot detail
- Allocation and presets
- Evolution chain
- Overgrowth
- All plots curves

## Inputs

- Save farming state
- Crop/plot selection
- Objective
- Land Rank allocation

## Outputs

- Plot assignment
- Recommended allocation
- Preset comparison
- Evolution chain
- Growth curves

## Assets

Crop icons if present.

## Feature issues

### FM-01 — Field and plot detail

Full 9x4 field, per-plot detail and crop selection.

Acceptance:

- Every plot renders.
- Plot detail matches save.
- Selection persists.

### FM-02 — Plot assignment and Land Rank

Assign crops to plots and allocate Land Rank for the selected objective.

Acceptance:

- Assignment respects plot rules.
- Allocation respects points.
- Result beats baseline.

### FM-03 — Presets and comparison

Preset objectives, recommended allocation and preset comparison.

Acceptance:

- Every upstream preset exists.
- Comparison is per objective.
- Custom preset persists.

### FM-04 — Evolution and overgrowth

Evolution chain to a target crop, hardest remaining steps and overgrowth stopping.

Acceptance:

- Chain is complete.
- Steps are actionable.
- Overgrowth advice matches inputs.

### FM-05 — Signals, thresholds and curves

Priority signals, account multipliers, GMO thresholds, Crop Scientist bonuses, growth breakpoints and all-plot curves.

Acceptance:

- Signals update with state.
- Thresholds match upstream.
- Curves render for every plot.

## Verification

- Fixture matches upstream.
- Allocator invariant.
- Harness.
