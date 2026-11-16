# All Stats Dashboard

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M3`
- Section key: `stats-all`
- Group: `general`
- Status: `in-progress`
- Depends on: `foundation`

Implement the map-aware character/account stat dashboard with category breakdowns and the system formula explorer.

## Current build

- SA-01 to SA-03 ship the calculation context (character, map, map list, skill and construction mode with their save defaults), the 60-descriptor registry grouped by nine categories, and the formula explorer whose ledger counts every category. Eight carry-capacity rows, the two research rows, the four base-stat totals and the damage row are computed; the other forty-five report unavailable instead of guessing.
- SA-04 ships the game-input sandbox: the vault, gem-shop, talent, guild and stamp levels the computed capacity rows read are editable, each edit is spliced into a clone of the save (`crates/stats/src/edits.rs`), and both the baseline and the hypothetical save resolve, so a row never shows a stale value.
- The change ledger marks an edited input with the saved value it replaced and resets one input or every input; the affected-formula list reports each descriptor the edit moved with its saved and hypothetical value, the delta and the percentage, and clicking a row selects its descriptor. A descriptor row and its detail card keep the saved value beside the hypothetical one.
- Missing: a per-node editor inside the formula inspector, which needs a level-table catalogue and a combat-setup save writer, so equipment, obol, companion and bubble/prisma cells are not editable yet. The sandbox offers the saved cells the computed formulas actually read, and an edit to a cell the current character no longer lists is dropped so the ledger and the applied save always agree.

## Known gaps

- `SA-G1` Descriptors not modelled: 45 of 60 report unavailable (15 computed) — blocked by `stats:descriptors` (partial)
- `SA-G3` Per-node formula editor for equipment, obol, companion and bubble/prisma cells — blocked by `stats:level-tables` (absent)

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

Depends on: FND-01 Dashboard card grid, FND-03 Shared save JSON import, FND-08 Shared layout, theme and responsive rules, FND-09 Empty, loading, error and stale states

Acceptance:

- Every context selector exists.
- Context changes refresh all breakdowns.
- Context is restorable from project JSON.

### SA-02 — Category breakdowns

Map-aware character/account breakdowns for combat, skills, carry capacity and account totals.

Depends on: SA-01 Calculation context

Acceptance:

- All nine categories render.
- Totals agree with the formula ledger.
- Unknown systems show a warning.

### SA-03 — System formula explorer

Per-system formula cards with inputs, saved values, computed values and explanations.

Depends on: SA-02 Category breakdowns, FND-11 Shared formula tree and breakdown component

Acceptance:

- Search and filtering work.
- Values match the breakdowns.

### SA-04 — Input change ledger

Show which inputs changed and which formulas they affect.

Depends on: SA-02 Category breakdowns

Acceptance:

- Changed inputs are highlighted.
- Affected formulas update live.
- Reset restores save values.

## Verification

- Formula tests cover the computed totals for the fixture save.
- Explorer values match breakdowns.
- Harness covers every category.
