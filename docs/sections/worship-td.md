# Worship Tower Defense

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M14`
- Section key: `worship-td`
- Group: `w3`
- Status: `partial`
- Depends on: `foundation`

Port tower layouts, live wave simulation, replays, plan/model tabs and the strategy optimizer.

## Current build

- The live simulator ports `createWorshipTdRun`: a seeded mulberry32 stream, 100 ms ticks, wave countdowns, the tower state machine, mob modifiers, projectiles and the totem. The Glublin fixture is parity-checked against the upstream view tick for tick.
- Tower placement spends TD points through `validatePlacement`; a click on a standing tower upgrades it one level (all-A trait recipe this build), and selling refunds 60%.
- The Plan tab ports `heuristicPlan`, the Glublin 230 guide preset and `planPlacementLayouts`: plans place towers in order through `validatePlacement`, each phase sells and rebuilds, and the pane warns about locked types, refused placements and neutral roots. A plan simulation runs the same `Run` engine off the UI thread with weight-per-point upgrades, phase conditions and the plan's skip policy; its final towers carry each tower's damage share. The Model pane lists the modelling assumptions, missing inputs and the exact save values the run reads.
- Ranges and every HUD value read the same `RunView`, so the panel cannot disagree with the field. Restart rebuilds the run from the same trial and seed.
- Every run records a replay at one frame per second. The recorder follows the upstream rules: a tower set per layout change, 4000-frame cap, and rate halving; scrub and speed read those frames through the same `RunView`.
- Save inputs decoded: `Tower` trial levels, `TotemInfo` best waves, `Dream[7]`, `SaltLick[6]`, Menagerie boost 15 and unlock 18, and super bits 6/15 for totem HP.
- Neutral this build: start points beyond the base 5, meals/Meritocracy kill points, boss-hold roots. The optimizer remains WT-04.

## Upstream sources

- worship-td-calc.html
- js/stats/systems/w3/worship-td.js
- js/ui/worship-td*.js
- assets/worship-td

## UI

- Trial selector
- Live simulator with wave/ranges/HUD
- Replay with scrub/speed
- Plan and model assumptions
- Optimizer and recommended plan

## Inputs

- Character and trial
- Tower layout
- Wave and phase config
- Optimizer seeds/mode

## Outputs

- Wave outcomes
- Final towers
- Recommended plan
- Replay timeline

## Assets

assets/worship-td towers, projectiles and map art.

## Feature issues

### WT-01 — Live tower-defense simulator

Run waves with tower placement, ranges, HUD and restart controls.

Depends on: FND-03 Shared save JSON import, FND-06 Upstream asset pipeline, FND-12 Background task runner with progress and cancel

Acceptance:

- Waves resolve deterministically.
- Ranges and HUD match state.
- Restart resets cleanly.

### WT-02 — Replay and scrubbing

Play, scrub, change speed and inspect waves from a recorded run.

Depends on: WT-01 Live tower-defense simulator

Acceptance:

- Scrub matches timeline.
- Speed controls work.
- Replay loads from a run.

### WT-03 — Plan and model assumptions

Phase plan, final towers and model-assumption panels.

Depends on: WT-01 Live tower-defense simulator

Acceptance:

- Plan matches the simulator.
- Assumptions are listed.
- Missing data is warned.

### WT-04 — Strategy optimizer

Search tower strategies with seeds, mode and recommended plan output.

Depends on: WT-03 Plan and model assumptions, FND-12 Background task runner with progress and cancel

Acceptance:

- Optimizer is cancellable.
- Recommended plan beats baseline.
- Results are reproducible by seed.

## Verification

- Wave fixture.
- Replay determinism.
- Optimizer invariant.
- Harness.
