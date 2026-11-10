# Coin Multiplier Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M6`
- Section key: `coin-multi`
- Group: `tools`
- Status: `planned`
- Depends on: `foundation`, `stats-all`

Port the coin multiplier calculator with map, target, HP, damage and overkill inputs and the formula breakdown.

## Upstream sources

- coin-multi-calc.html
- js/stats/systems/common/overkill.js
- js/stats/tree-builder.js

## UI

- Context selectors
- Current calculation cards
- Formula breakdown tree

## Inputs

- Character
- Map value
- Target, HP, damage and overkill values

## Outputs

- Total coin multiplier
- Partial-badge state
- Formula tree

## Assets

None.

## Feature issues

### CM-01 — Coin multiplier inputs and total

Character/map and combat inputs with the total multiplier card and partial badge.

Depends on: FND-03 Shared save JSON import, FND-08 Shared layout, theme and responsive rules, SA-01 Calculation context

Acceptance:

- Total matches upstream.
- Partial state is explained.
- Inputs validate.

### CM-02 — Coin multiplier formula tree

Expandable breakdown of every coin multiplier term.

Depends on: CM-01 Coin multiplier inputs and total, FND-11 Shared formula tree and breakdown component

Acceptance:

- Tree sums to the total.
- Sources are named.
- Search works.

## Verification

- Fixture matches upstream.
- Tree-sum test.
