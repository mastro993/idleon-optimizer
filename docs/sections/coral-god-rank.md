# Coral God Rank Planner

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M21`
- Section key: `coral-god-rank`
- Group: `w5`
- Status: `planned`

Port the coral kid upgrade and god-rank offering planner with beam search and projection.

## Upstream sources

- coral-god-rank-calc.html
- js/stats/systems/w5/coral-god-rank.js

## UI

- Planner controls
- Summary
- Projected coral state
- Plan actions
- Offering fit
- Current levels

## Inputs

- Goal, budget, max levels, beam size
- God rank, Y4, div cost, reef, mine floor, vault, tier
- Assume unlocked

## Outputs

- Best plan
- Projected state
- Offering fit
- Action list

## Assets

Coral icons if present.

## Feature issues

### CR-01 — Planner controls and summary

All upstream planner inputs with validation and a summary of the current state.

Acceptance:

- Every input is present.
- Validation prevents impossible plans.
- Summary matches save.

### CR-02 — Beam-search plan and projection

Run the beam planner and show projected coral state and action list.

Acceptance:

- Plan respects budget and max levels.
- Beam size changes search depth.
- Results are reproducible.

### CR-03 — Offering fit and current levels

Show offering fit and current coral kid levels.

Acceptance:

- Fit explanation is clear.
- Current levels match save.
- Missing data is warned.

## Verification

- Planner invariant.
- Fixture matches upstream.
- Harness.
