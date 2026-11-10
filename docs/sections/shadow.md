# Shadow Ban Check

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M7`
- Section key: `shadow`
- Group: `general`
- Status: `planned`

Port the shadow-ban answer page with the upstream YES/NO/UNKNOWN semantics from OptLacc[26].

## Upstream sources

- shadow.html
- js/save/loader.js

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

Acceptance:

- All three states are reachable in tests.
- Verdict never guesses.
- Save status is visible.

## Verification

- Three-state fixture test.
- Harness screenshot.
