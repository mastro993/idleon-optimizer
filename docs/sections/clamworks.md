# Clamworks Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M26`
- Section key: `clamworks`
- Group: `w7`
- Status: `planned`

Port Clamworks overview, upgrades, AFK claim, optimizer, promotion, worker rewards and Scurge combat.

## Upstream sources

- clamworks-calc.html
- js/stats/systems/w7/clamworks*.js
- js/ui/clamworks-*.js

## UI

- Overview and pearl value
- Upgrades and priority
- AFK claim and compare
- Optimizer
- Promotion
- Worker class rewards
- Scurge combat

## Inputs

- Save Clamworks state
- Character and multikill
- AFK hours
- Upgrade levels

## Outputs

- Pearl value
- Upgrade plan
- AFK claim
- Promotion progress
- Combat outcomes

## Assets

Clamworks icons if present.

## Feature issues

### CW-01 — Overview and upgrades

Summary, pearl value breakdown, all upgrades and upgrade priority.

Acceptance:

- Values match upstream.
- Priority updates with save.
- Breakdown sums.

### CW-02 — AFK claim and compare

AFK claim calculator with character, multikill and compare panel.

Acceptance:

- Claim matches upstream.
- Compare is per character.
- Invalid hours rejected.

### CW-03 — Optimizer and promotion

Upgrade optimizer plus promotion tracker and worker class rewards.

Acceptance:

- Optimizer respects costs.
- Promotion progress matches save.
- Rewards list is complete.

### CW-04 — Scurge combat

Scurge combat and upgrade panels.

Acceptance:

- Combat values match upstream.
- Upgrades affect outcomes.
- Missing data warned.

## Verification

- Fixture matches upstream.
- Optimizer invariant.
- Harness.
