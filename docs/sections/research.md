# Research Optimizer: full upstream parity

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

- Milestone: `M1`
- Section key: `research`
- Group: `w7`
- Status: `partial`

Finish the Research section so all eight upstream tabs and the shared save/supplement tools are present, including the Decision Tree, Insight ROI, Observation Unlocks and full save-derived multipliers.

## Upstream sources

- research-optimizer-v2.html
- js/app.js
- js/ui/optimizer-ui.js
- js/ui/dashboard.js
- js/ui/state-io.js
- js/dt/decision-tree.js
- js/stats/systems/w7/research-math.js
- js/optimizers/shapes.js
- js/optimizers/mags.js
- js/optimizers/grid-spend.js
- js/workers/worker-pool.js
- js/sim-engine.js

## UI

- Tabs: Dashboard, Optimizer, Insight ROI, Observation Unlocks, Shape Optimization, Decision Tree, Minehead, Sushi Station.
- Dashboard: summary, research grid, observations, EXP rate breakdown, AFK rate, insight multiplier.
- Optimizer: target level/hours, assume observations, extended insight lookahead, tournament +12h, strategy comparison, timeline, EXP/hr chart, step actions and Open in Decision Tree.
- Decision Tree: pan/zoom tree, branch/save/delete nodes, upgrade and shape modes, observation editor, auto insight, compare branches.

## Inputs

- Research rows from the save
- Grid levels
- Shape placements
- Magnifier pool
- Observation found/insight state
- Supplements and account multipliers

## Outputs

- EXP/hour breakdown
- Projection and strategy timeline
- Optimized layout
- Insight ROI and unlock priorities
- Decision-tree branches

## Assets

Research sprites under assets/combat-sim/ui/research (ResShape*, ResSq*, ResearchBG, magnifier and observation icons).

## Feature issues

### RES-01 — Research dashboard tab

Port the upstream dashboard: summary, research grid preview, observation board, EXP rate breakdown, AFK rate and insight multiplier.

Acceptance:

- All six dashboard blocks render from the loaded save.
- Values match the Optimizer tab and the Rust simulation.
- Grid and observation previews link to the editors.

### RES-02 — Optimizer target and options

Target level or hours plus the three upstream toggles: assume observations unlocked, extended insight lookahead and tournament +12h.

Acceptance:

- Each toggle changes the simulated projection in a testable way.
- Targets are validated and remembered per session.
- The run can be cancelled without losing the previous result.

### RES-03 — Target-aware optimizer engine

Simulate to the target, spend grid points earned on the way and re-optimize shapes and magnifiers on research and insight level-ups, matching upstream sim-engine and worker-pool behavior.

Acceptance:

- A level target optimizes time-to-level, not only immediate EXP/hr.
- Future grid points and observation unlocks are applied at the correct level-ups.
- Results are deterministic for the same save and options.

### RES-04 — Strategy comparison, timeline and actions

Port the upstream result view: best/worst strategy comparison, interactive timeline, EXP/hr chart and expandable step-by-step actions.

Acceptance:

- The comparison reports best and worst tested strategies.
- Timeline and chart agree with the projection samples.
- Actions expand to their detail and highlight the timeline phase.

### RES-05 — Insight ROI tab

Per-observation insight levelling analysis: time to level, research EXP lost during the grind, permanent rate gain and break-even time.

Acceptance:

- Table covers every usable observation with the upstream columns.
- Sorting and summary totals work.
- Values match the simulation's insight formulas.

### RES-06 — Observation Unlocks tab

Rank undiscovered observations by expected EXP/hr gain weighted by unlock probability per day.

Acceptance:

- Priority table matches upstream ordering for the fixture save.
- Filters and summary are present.
- Missing roll data is reported, not guessed.

### RES-07 — Shape Optimization tab

Standalone shape workspace: priority tiers above/below the Res EXP divider, drag to reorder, built-in and custom presets, optimized placement preview and coverage breakdown.

Acceptance:

- Tier editing detaches a built-in preset without mutating it.
- Custom presets persist in the project state.
- Placement preview and breakdown use the real optimizer.

### RES-08 — Decision Tree sandbox

Interactive tree of research events: branch, save, delete, compare, zoom, level-ups-only, upgrade/shape modes and observation editing.

Acceptance:

- A run can be imported from the Optimizer tab.
- Branch comparison shows deltas for rate, level and time.
- Auto Insight and shape/magnifier optimize actions work per node.

### RES-09 — Minehead tab

Port the research page's Minehead tab and its save-derived inputs.

Acceptance:

- All upstream Minehead controls and results are present.
- Values update with the loaded save.
- Formula tests match upstream fixtures.

### RES-10 — Sushi Station tab

Port the research page's Sushi Station tab and its account bonuses.

Acceptance:

- All upstream Sushi controls and results are present.
- RoG bonuses feed research grid points and multipliers.
- Formula tests match upstream fixtures.

### RES-11 — Full save-derived multiplier tree

Decode every account term the upstream derives: sticker tree, arcade 63, Clam Works 9, Menagerie B3, equipment, Button, royal research, jelly, RoG, mastery, event shop and minehead.

Acceptance:

- Imported save rate matches upstream for the fixture and a full-account save.
- Every omitted term is warned with its field name.
- No term is guessed.

### RES-12 — State export/import and supplements

Copy/Paste State string plus the full supplements panel, matching upstream state-io and supplement behavior.

Acceptance:

- Exported ROv2 string imports back to the same state.
- Supplements apply to the simulation and persist.
- Invalid state strings leave the run untouched.

### RES-13 — Grid, shape and magnifier editors

Complete the manual editing surface: grid levels, shape drag/rotate/add/remove, magnifier assignment, undo/redo and canvas rotation handle.

Acceptance:

- Every edit is undoable and redoable.
- Shapes can be rotated on the canvas.
- Keyboard and pointer edits agree.

## Verification

- Every research tab opens in the harness.
- Formula tests compare against upstream Node fixtures.
- Full save fixture matches upstream EXP/hr within tolerance.
