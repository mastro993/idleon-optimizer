# Spelunking Simulator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M28`
- Section key: `spelunking`
- Group: `w7`
- Status: `partial`
- Depends on: `foundation`

Port Normal/Manic expeditions, auto speeds, manual strategies, parties, elixirs, amber policies and the depth optimizer.

## Upstream sources

- spelunking-calc.html
- js/ui/spelunking.js
- js/stats/systems/w7/spelunk*.js
- assets/spelunking

## UI

- Party and elixir belt
- Depth estimates
- Character/cave/mode/policy controls
- Amber policy
- Cave choices and simulator
- Optimizer

## Inputs

- Character save
- Cave/mode/policy
- Party stamina
- Elixirs and amber settings

## Outputs

- Depth estimates
- Run outcomes
- Elixir effects
- Optimizer plan

## Assets

assets/spelunking cave tiles, rocks, doors, bosses, class icons and elixirs.

## Feature issues

### SP-01 — Controls, party and elixirs

Character, cave, mode, policy, full-clear, speed, party stamina and elixir belt.

Depends on: FND-03 Shared save JSON import, FND-06 Upstream asset pipeline

Acceptance:

- Every control is present.
- Party and elixirs affect estimates.
- Invalid parties are rejected.

### SP-02 — Run simulator

Cave choices, stamina, entry, tool and reaction simulation with memory.

Depends on: SP-01 Controls, party and elixirs, FND-12 Background task runner with progress and cancel

Acceptance:

- Runs are deterministic by seed.
- Memory and choices persist.
- Cancel works.

### SP-03 — Amber policies

Amber dump depth, finishers, reserve, harvest and rope reserve policies.

Depends on: SP-02 Run simulator

Acceptance:

- Policies change outcomes.
- All fields validate.
- Results explain the policy.

### SP-04 — Depth optimizer

Optimize depth limits and policies for the selected cave and party.

Depends on: SP-03 Amber policies, FND-12 Background task runner with progress and cancel

Acceptance:

- Optimizer beats baseline.
- Result is reproducible.
- Plan is copyable.

## Verification

- Fixture matches upstream.
- Simulator determinism.
- Harness with assets.
