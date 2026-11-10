# Damage Breakdown

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M4`
- Section key: `damage`
- Group: `tools`
- Status: `planned`

Port the damage calculator: min/max damage, mastery, damage layers and the full formula tree.

## Upstream sources

- damage-calc.html
- js/stats/stat-context.js
- js/stats/systems/w2/dungeon.js
- js/ui/dash-breakdowns.js

## UI

- Context selectors
- Damage summary and mastery bars
- Layer tabs
- Formula breakdown tree

## Inputs

- Character and map
- Save data
- Dungeon context

## Outputs

- Min/max damage
- Mastery value
- Damage layers
- Formula tree

## Assets

None.

## Feature issues

### DMG-01 — Damage context and summary

Character/map selectors plus game, minimum, maximum and exact damage cards.

Acceptance:

- Values match upstream.
- Missing inputs are flagged.
- Context persists.

### DMG-02 — Damage layers and mastery

Layer tabs with per-layer values and notes, plus the mastery bar and breakdown.

Acceptance:

- Every upstream layer is present.
- Layer sum matches the summary.
- Mastery matches the formula tree.

### DMG-03 — Formula tree

Expandable formula tree for damage and mastery.

Acceptance:

- Nodes show input, multiplier and source.
- Search highlights nodes.
- Values match the summary.

## Verification

- Damage fixture matches upstream.
- Layer sum test.
- Harness screenshot.
