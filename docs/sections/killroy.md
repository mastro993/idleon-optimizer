# Killroy Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M12`
- Section key: `killroy`
- Group: `w2`
- Status: `done`
- Depends on: `foundation`

Port weekly Killroy forecasts, monster/class records, kills, skulls and +1 upgrade comparisons.

## Current build

- KL-01 Weekly forecast: the week strip, each encounter's family, room and saved record, the class and unlock eligibility and the rotation statistics, pinned against an oracle fixture of the upstream planner's own output.
- KL-02 +1 upgrade comparison: the upgrade list with saved levels and availability, the selection preview, the trials through the shared task runner and the comparison table that marks the best option per baseline and flags a stale result.
- KL-03 Saved buffs and results: the save's `BuffsActive` rows with kept/dropped markers, the Keep saved buffs and Unlimited inventory options, the bag-limited skull pickups the trial models, progress/cancel and the results export document. The combat simulator is still M2, so kills per second stands in for the character's damage; the encounter, respawn and drop maths are the game's own.

## Upstream sources

- killroy-calc.html
- js/ui/killroy.js
- js/stats/systems/common/killroy-planner.js
- js/stats/data/w2/killroy.js

## UI

- Settings and forecast weeks
- Baseline and upgrades tabs
- Upgrade list and selection preview
- Trials and progress
- Results and saved buffs

## Inputs

- Character save
- Forecast weeks
- Upgrade selection
- Trials

## Outputs

- Weekly forecast
- Monster/class records
- Skull outcomes
- +1 comparisons

## Assets

Killroy icons if present.

## Feature issues

### KL-01 — Weekly forecast

Forecast encounters, kills, skulls and records over the selected weeks.

Depends on: FND-03 Shared save JSON import

Acceptance:

- Forecast matches upstream.
- Week range validates.
- Results explain assumptions.

### KL-02 — +1 upgrade comparison

Compare the next upgrade per option with selection preview and trials.

Depends on: KL-01 Weekly forecast, FND-12 Background task runner with progress and cancel

Acceptance:

- Every upgrade is comparable.
- Trials are deterministic.
- Best option is highlighted.

### KL-03 — Saved buffs and results

Render saved buffs, unlimited mode, progress/cancel and the results table.

Depends on: KL-02 +1 upgrade comparison, FND-11 Shared formula tree and breakdown component

Acceptance:

- Cancel works.
- Saved buffs match the save.
- Results export as JSON.

## Verification

- Forecast fixture.
- Trial determinism test.
- Harness.
