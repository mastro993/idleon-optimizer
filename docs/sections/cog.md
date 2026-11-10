# Cog Board Optimizer

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M13`
- Section key: `cog`
- Group: `w3`
- Status: `planned`

Port cog placement optimization for construction bonuses with weighted objectives, insights, shelf and roll targets.

## Upstream sources

- cog-optimizer.html
- js/cogs/cog-board.js
- js/cogs/cog-judge.js
- js/cogs/cog-optimizer.js

## UI

- Summary stats
- Weighted objective inputs
- Optimize/perfect/reset
- Swap list and steps
- Board insights, shelf and roll ranges

## Inputs

- Cog board and inventory
- Construction level
- Objective weights

## Outputs

- Optimized placement
- Swap steps
- Board grade
- Roll targets

## Assets

Cog icons if present.

## Feature issues

### CG-01 — Cog board optimizer

Optimize cog placement for build, construction EXP and Flaggy rate objectives.

Acceptance:

- Optimizer respects board rules.
- Result is never worse than input.
- Reset restores the save board.

### CG-02 — Weighted objectives and perfect board

Weighted objective inputs plus perfect-board and reset actions.

Acceptance:

- Weights change the result.
- Perfect board is explainable.
- Invalid weights are rejected.

### CG-03 — Insights, shelf and roll targets

Board insights, shelf contents, roll ranges and what to roll for.

Acceptance:

- Insights match the optimizer.
- Roll ranges use the construction level.
- Shelf is read-only.

### CG-04 — Swap walkthrough

Step-by-step swap list with previous/next/done controls.

Acceptance:

- Steps apply in order.
- Previous restores the prior board.
- Done persists the final board.

## Verification

- Optimizer invariant tests.
- Swap undo test.
- Harness.
