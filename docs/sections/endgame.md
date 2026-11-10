# Endgame Check

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M8`
- Section key: `endgame`
- Group: `general`
- Status: `planned`

Port the joke endgame page: always NO, with the upstream stats and leaderboard audit panels.

## Upstream sources

- endgame.html
- js/endgamers.js
- js/endgame-stats.js

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

Acceptance:

- Verdict is NO with and without a save.
- Stats render or fail with a clear error.
- Matches upstream copy.

## Verification

- Verdict test.
- Harness screenshot.
