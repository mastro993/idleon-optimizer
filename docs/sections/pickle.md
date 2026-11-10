# BoneJoePickle Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M30`
- Section key: `pickle`
- Group: `tools`
- Status: `done`
- Depends on: `foundation`

Port the hidden DPS/TTK calculator.

## Upstream sources

- pickle-calc.html

## UI

- DPS and kill-time inputs
- Miniboss pickle table

## Inputs

- DPS
- Max time to kill

## Outputs

- Max pickles
- Effective HP
- Kill time

## Assets

None.

## Feature issues

### PK-01 — DPS and TTK

Compute each miniboss's maximum pickles, effective HP and time-to-kill from DPS and a kill-time budget.

Depends on: FND-03 Shared save JSON import

Acceptance:

- Values match upstream.
- Inputs validate.
- Empty state is clear.

## Verification

- Fixture matches upstream.
- Harness.
