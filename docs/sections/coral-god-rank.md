# Coral God Rank Planner

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M21`
- Section key: `coral-god-rank`
- Group: `w5`
- Status: `done`
- Depends on: `foundation`

Port the coral kid upgrade and god-rank offering planner with beam search and projection.

## Current build

- CR-01 Planner controls and summary: every upstream input with validation, the save's budget, rank, tier, Coral Kid and Singularity levels, and the current coral metrics, with the fields the save lacks named.
- CR-02 Beam-search plan and projection: `idleon_core::coral::plan` over unlocked upgrades and one offering per state, run through the shared task runner with progress, cancel and a stale marker; the card shows spend, remaining, whole-plan confidence, the projected coral state and the ordered actions.
- CR-03 Offering fit and current levels: the upstream `offeringStats` row at the current rank (success per attempt, expected spend per success, tier) with a caption for the Y4 free chance and planning confidence, the Coral Kid levels table with each upgrade's unlock status, and warnings for the save fields read as zero.

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

Depends on: FND-03 Shared save JSON import

Acceptance:

- Every input is present.
- Validation prevents impossible plans.
- Summary matches save.

### CR-02 — Beam-search plan and projection

Run the beam planner and show projected coral state and action list.

Depends on: CR-01 Planner controls and summary, FND-12 Background task runner with progress and cancel

Acceptance:

- Plan respects budget and max levels.
- Beam size changes search depth.
- Results are reproducible.

### CR-03 — Offering fit and current levels

Show offering fit and current coral kid levels.

Depends on: CR-02 Beam-search plan and projection

Acceptance:

- Fit explanation is clear.
- Current levels match save.
- Missing data is warned.

## Verification

- Planner invariant.
- Fixture matches upstream.
- Harness.
