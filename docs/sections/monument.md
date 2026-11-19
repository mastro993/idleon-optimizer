# Monument Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M22`
- Section key: `monument`
- Group: `w5`
- Status: `done`
- Depends on: `foundation`

Implement Bravery, Justice and Wisdom calculators, permanent rewards, reward charge, training and bonus sources.

## UI

- Bravery tab
- Justice tab with response advisor; a typed worksheet stat or deal value applies on Enter or when its field loses focus, so each edit is one undo step
- Wisdom tab
- Bonus sources
- Permanent and charge summaries

## Inputs

- Character save
- Monument levels/boosts
- Justice case/effects

## Outputs

- Wave/HP/odds
- Milestones
- Permanent rewards
- Response advice

## Assets

Monument icons if present.

## Feature issues

### MN-01 — Bravery calculator

Bravery stats, wave, boosts, rerolls, HP, odds, milestones and bonuses.

Depends on: FND-03 Shared save JSON import

Acceptance:

- Boost changes update odds.
- Invalid inputs are rejected.

### MN-02 — Justice calculator and advisor

Justice stats, reset/undo, effects editor, search, case and response advisor.

Depends on: MN-01 Bravery calculator

Acceptance:

- Effects editor is undoable.
- Advisor explains its choice.
- Search works.

### MN-03 — Wisdom calculator

Wisdom stats and training/reward-charge effects.

Depends on: MN-01 Bravery calculator

Acceptance:

- Charges update rewards.
- Missing data is warned.

### MN-04 — Permanent rewards and bonus sources

Permanent summary, next-run charge and training/reward-charge source list.

Depends on: MN-03 Wisdom calculator, FND-11 Shared formula tree and breakdown component

Acceptance:

- Sources sum to the summary.
- Each source is named.
- State persists.

## Verification

- Formula tests cover the section's game rules.
- Justice undo test.
- Harness.
