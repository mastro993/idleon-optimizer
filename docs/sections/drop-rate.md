# Drop Rate Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M5`
- Section key: `drop-rate`
- Group: `tools`
- Status: `partial`
- Depends on: `foundation`, `stats-all`

Port the drop-rate calculator with dungeon runtime/luck/drop-rate inputs and the formula breakdown.

## Upstream sources

- drop-rate-calc.html
- js/stats/systems/w2/dungeon.js
- js/stats/tree-builder.js

## UI

- Context selectors
- Dungeon inputs
- Total card
- Formula tree

## Inputs

- Character and map
- Dungeon runtime, luck and drop rate

## Outputs

- Total drop rate
- Formula breakdown

## Assets

None.

## Feature issues

### DR-01 — Drop-rate inputs and total

Character/map selectors, dungeon runtime/luck/DR fields and the total drop-rate card.

Depends on: FND-03 Shared save JSON import, FND-08 Shared layout, theme and responsive rules, SA-01 Calculation context

Acceptance:

- Total matches upstream.
- Inputs validate ranges.
- Save values prefill.

### DR-02 — Drop-rate formula tree

Expandable breakdown of every drop-rate term.

Depends on: DR-01 Drop-rate inputs and total, FND-11 Shared formula tree and breakdown component

Acceptance:

- Tree sums to the total.
- Each term names its source.
- Search works.

## Verification

- Fixture matches upstream.
- Tree-sum test.
