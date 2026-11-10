# Sailing Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M19`
- Section key: `sailing`
- Group: `w5`
- Status: `planned`

Port fleet assignment, ship upgrades, captain quality, replacement matrix, chest outcomes, artifact odds and the fleet pile simulation.

## Upstream sources

- sailing-calc.html
- js/stats/systems/w5/sailing.js

## UI

- Fleet assignment
- Ship upgrade breakpoints
- Captain quality/leveling
- Replacement matrix
- Chest outcomes
- Artifacts
- Formula breakdowns
- Fleet simulation

## Inputs

- Save fleet and captains
- Island/route selection
- Assignment mode

## Outputs

- Trip time
- Loot value
- Chest tier chances
- Artifact odds
- Simulation results

## Assets

Sailing icons if present.

## Feature issues

### SL-01 — Fleet assignment

Current versus optimized fleet assignment across routes.

Acceptance:

- Optimized assignment beats current.
- Route switching works.
- Constraints are respected.

### SL-02 — Ships, captains and replacements

Ship upgrade breakpoints, captain quality/leveling and strongest-source replacement matrix.

Acceptance:

- Breakpoints match upstream.
- Replacement ranking is stable.
- Missing captains are flagged.

### SL-03 — Chest, artifact and loot outcomes

Chest outcomes, per-boat tier chances, near-full pile and selected island artifacts.

Acceptance:

- Chances sum to one.
- Loot value matches upstream.
- Artifact odds are explained.

### SL-04 — Fleet pile simulation

Simulate the fleet pile with progress and results.

Acceptance:

- Simulation is cancellable.
- Results are reproducible.
- Matches upstream fixture.

## Verification

- Fixture matches upstream.
- Chance-sum test.
- Harness.
