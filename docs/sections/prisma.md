# Prisma Calculator

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M11`
- Section key: `prisma`
- Group: `w2`
- Status: `partial`
- Depends on: `foundation`

Port bubble Prisma impact search across damage, stats, skills and account totals with streamed results.

## Upstream sources

- prisma-calc.html
- js/ui/prisma.js

## UI

- Character and AFK-hours controls
- Metric/search filters
- Progress and cancel
- Result rows
- Coverage table

## Inputs

- Character save
- AFK hours
- Metric filter
- Search

## Outputs

- Bubble impact rows
- Coverage
- Stale/summary state

## Assets

Bubble icons if present.

## Feature issues

### PR-01 — Prisma search and run

Run the bubble impact search with metric filters, progress and cancel.

Depends on: FND-03 Shared save JSON import, FND-12 Background task runner with progress and cancel

Acceptance:

- Cancel restores the previous result.
- Progress is accurate.
- Stale results are marked.

### PR-02 — Result rows and coverage

Ranked bubble impacts with per-metric deltas and a coverage table.

Depends on: PR-01 Prisma search and run

Acceptance:

- Rows sort by the selected metric.
- Coverage lists missing systems.
- Values match upstream.

## Verification

- Fixture ranking test.
- Cancel test.
- Harness.
