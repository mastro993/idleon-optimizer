# Jar Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M18`
- Section key: `jar`
- Group: `w5`
- Status: `planned`

Port jar production, rupie value, break chances, collectibles, upgrades, next tier and the enchant simulator/history.

## Upstream sources

- jar-calc.html
- js/stats/systems/w5/jar.js

## UI

- Overview
- Production
- Rupie value
- Chances
- Collectibles
- Upgrades
- Next tier
- Enchant simulator
- Enchant history

## Inputs

- Save jar state
- Jar type/tier
- Enchant target/count/strategy

## Outputs

- Production rates
- Rupie income
- Break chances
- Upgrade plan
- Enchant outcomes

## Assets

Jar icons if present.

## Feature issues

### JR-01 — Overview and production

Jar summary, rupie balances, production slots and multipliers.

Acceptance:

- Values match upstream.
- Slot details render.
- Missing jars are flagged.

### JR-02 — Rupie value and chances

Rupie value breakdown, extra chance, break chances and chance-by-tier.

Acceptance:

- Values match upstream.
- Tier switching works.
- Breakdown sums to the total.

### JR-03 — Collectibles, upgrades and next tier

Collectible list, upgrade costs and next-tier requirements.

Acceptance:

- Upgrade plan respects budget.
- Next tier is actionable.
- Max tier is clear.

### JR-04 — Enchant simulator and history

Simulate enchants with target, count, strategy and focus, and keep a history.

Acceptance:

- Simulation is seeded/reproducible.
- History persists in project JSON.
- Clear history works.

## Verification

- Fixture matches upstream.
- Enchant determinism.
- Harness.
