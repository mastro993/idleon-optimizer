# Endgame Check

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M8`
- Section key: `endgame`
- Group: `general`
- Status: `done`
- Depends on: `foundation`

Implement the joke endgame page: always NO, with the stats and leaderboard audit panels.

## Known gaps

- `EG-G1` 21 leaderboard metrics have no value — blocked by `wire:stats-engine` (landed), `stats:descriptors` (partial)

## UI

- Large verdict
- Stats table

## Inputs

- Optional save

## Outputs

- NO verdict
- Stats summary

## Assets

None.

## Feature issues

### EG-01 — Endgame verdict and stats

Always display NO before and after loading, with the stats and error panels.

Depends on: FND-03 Shared save JSON import

Acceptance:

- Verdict is NO with and without a save.
- Stats render or fail with a clear error.

## Verification

- Verdict test.
- Harness screenshot.
