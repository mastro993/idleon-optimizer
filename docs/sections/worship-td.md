# Worship Tower Defense

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M14`
- Section key: `worship-td`
- Group: `w3`
- Status: `planned`
- Depends on: `foundation`

Port tower layouts, live wave simulation, replays, plan/model tabs and the strategy optimizer.

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
