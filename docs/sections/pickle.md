# BoneJoePickle Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M30`
- Section key: `pickle`
- Group: `tools`
- Status: `planned`
- Depends on: `foundation`

Port the hidden DPS/TTK calculator.

## Upstream sources

- pickle-calc.html

## UI

- Inputs
- DPS and TTK results

## Inputs

- Pickle combat values

## Outputs

- DPS
- TTK

## Assets

None.

## Feature issues

### PK-01 — DPS and TTK

Compute DPS and time-to-kill from the upstream inputs.

Depends on: FND-03 Shared save JSON import

Acceptance:

- Values match upstream.
- Inputs validate.
- Empty state is clear.

## Verification

- Fixture matches upstream.
- Harness.
