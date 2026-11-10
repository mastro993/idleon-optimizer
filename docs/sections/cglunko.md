# Cglunko Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M20`
- Section key: `cglunko`
- Group: `w5`
- Status: `planned`

Port Cglunko overview, upgrades, optimizer, stone focus and drop-rate breakdown across blue and purple pages.

## Upstream sources

- cglunko-calc.html
- js/stats/systems/w5/cglunko.js
- js/ui/cglunko-combat.js

## UI

- Overview and DR formula
- Upgrades
- Optimizer
- Stones with page tabs
- Recommended focus

## Inputs

- Save Cglunko state
- Color/page
- Upgrade levels
- Stone digits

## Outputs

- DR formula
- Upgrade priority
- Stone focus
- Recommended target

## Assets

Cglunko icons if present.

## Feature issues

### CL-01 — Overview and DR breakdown

Summary, drop-rate formula breakdown and combat context.

Acceptance:

- Values match upstream.
- Formula tree sums to the total.
- Missing data is warned.

### CL-02 — Upgrades and optimizer

Upgrade list, priority and optimizer for the selected color.

Acceptance:

- Optimizer respects costs.
- Priority updates with color.
- Result is never worse.

### CL-03 — Stone focus

Blue/purple stone pages, digit impact and recommended focus target.

Acceptance:

- Both pages render.
- Digit impact matches upstream.
- Focus target is explained.

## Verification

- Fixture matches upstream.
- Optimizer invariant.
- Harness.
