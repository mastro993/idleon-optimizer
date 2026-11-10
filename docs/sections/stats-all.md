# All Stats Dashboard

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M3`
- Section key: `stats-all`
- Group: `general`
- Status: `planned`

Port the map-aware character/account stat dashboard with category breakdowns and the system formula explorer.

## Upstream sources

- stats-all.html
- js/ui/dash-breakdowns.js
- js/stats/tree-builder.js
- js/stats/stat-context.js

## UI

- Calculation context selectors
- Breakdowns grouped by category
- System formula explorer with formula ledger
- Input change list

## Inputs

- Character and map selection
- Skill and construction mode
- Save data

## Outputs

- Stat breakdowns
- Formula explanations
- Affected-formula list

## Assets

None beyond icons.

## Feature issues

### SA-01 — Calculation context

Character, map, visibility, skill and construction-mode selectors with save defaults.

Acceptance:

- Every upstream selector exists.
- Context changes refresh all breakdowns.
- Context is restorable from project JSON.

### SA-02 — Category breakdowns

Map-aware character/account breakdowns for combat, skills, carry capacity and account totals.

Acceptance:

- All upstream categories render.
- Totals agree with the formula ledger.
- Unknown systems show a warning.

### SA-03 — System formula explorer

Per-system formula cards with inputs, saved values, computed values and explanations.

Acceptance:

- Search and filtering work.
- Each formula links to its upstream source.
- Values match the breakdowns.

### SA-04 — Input change ledger

Show which inputs changed and which formulas they affect.

Acceptance:

- Changed inputs are highlighted.
- Affected formulas update live.
- Reset restores save values.

## Verification

- Fixture totals match upstream stats page.
- Explorer values match breakdowns.
- Harness covers every category.
