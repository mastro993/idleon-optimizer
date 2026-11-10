# Killroy Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M12`
- Section key: `killroy`
- Group: `w2`
- Status: `planned`
- Depends on: `foundation`

Port weekly Killroy forecasts, monster/class records, kills, skulls and +1 upgrade comparisons.

## Upstream sources

- killroy-calc.html
- js/ui/killroy.js
- js/stats/systems/w2/killroy.js

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
