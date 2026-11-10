# Killroy Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M12`
- Section key: `killroy`
- Group: `w2`
- Status: `planned`

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

Acceptance:

- Forecast matches upstream.
- Week range validates.
- Results explain assumptions.

### KL-02 — +1 upgrade comparison

Compare the next upgrade per option with selection preview and trials.

Acceptance:

- Every upgrade is comparable.
- Trials are deterministic.
- Best option is highlighted.

### KL-03 — Saved buffs and results

Render saved buffs, unlimited mode, progress/cancel and the results table.

Acceptance:

- Cancel works.
- Saved buffs match the save.
- Results export as JSON.

## Verification

- Forecast fixture.
- Trial determinism test.
- Harness.
