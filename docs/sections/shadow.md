# Shadow Ban Check

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M7`
- Section key: `shadow`
- Group: `general`
- Status: `done`
- Depends on: `foundation`

Implement the shadow-ban answer page with the YES/NO/UNKNOWN semantics of OptLacc[26].

## UI

- Large verdict
- Load/save status

## Inputs

- OptLacc[26] from the save

## Outputs

- YES, NO or UNKNOWN

## Assets

None.

## Feature issues

### SH-01 — Shadow-ban verdict

Show YES for nonzero, NO for zero and UNKNOWN before a valid save or when the flag is missing.

Depends on: FND-03 Shared save JSON import

Acceptance:

- All three states are reachable in tests.
- Verdict never guesses.
- Save status is visible.

## Verification

- Three-state test.
- Harness screenshot.
