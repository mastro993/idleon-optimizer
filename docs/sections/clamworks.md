# Clamworks Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M26`
- Section key: `clamworks`
- Group: `w7`
- Status: `done`
- Depends on: `foundation`, `combat-sim`

Implement Clamworks overview, upgrades, AFK claim, optimizer, promotion, worker rewards and Scurge combat.

## Known gaps

- `CW-G1` Kills per hour, drop rate and multikill are typed stand-ins — blocked by `combat:auto-player` (absent)

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

Depends on: FND-03 Shared save JSON import

Acceptance:

- Priority updates with save.
- Breakdown sums.

### CW-02 — AFK claim and compare

AFK claim calculator with character, multikill and compare panel.

Depends on: CW-01 Overview and upgrades

Acceptance:

- Compare is per character.
- Invalid hours rejected.

### CW-03 — Optimizer and promotion

Upgrade optimizer plus promotion tracker and worker class rewards.

Depends on: CW-01 Overview and upgrades, FND-12 Background task runner with progress and cancel

Acceptance:

- Optimizer respects costs.
- Promotion progress matches save.
- Rewards list is complete.

### CW-04 — Scurge combat

Scurge combat and upgrade panels.

Depends on: CW-01 Overview and upgrades, CS-03 Encounter simulator

Acceptance:

- Upgrades affect outcomes.
- Missing data warned.

## Verification

- Formula tests cover the section's game rules.
- Optimizer invariant.
- Harness.
