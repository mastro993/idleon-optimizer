# Plan: Idleon Research Optimizer, Rust + GPUI desktop app

Reimplementation of [Corgan/idleon-research-optimizer](https://github.com/Corgan/idleon-research-optimizer) as a
native desktop application. The upstream is a static-site suite of calculators that runs its
optimizers in web workers; this app keeps the same maths and moves the simulations and optimizers
into a local Rust core, so a run costs milliseconds of CPU instead of minutes of tab time.

## 1. Scope

**In scope (v1)**

1. **Research grid workspace** — the 20×12 grid of upgradeable squares with their level, name, goal
   and bonus, the ten research shapes drawn as polygons over it, and drag/rotate placement.
2. **Simulation** — research EXP/hour, the additive/multiplicative breakdown, insight levelling,
   magnifier caps, grid points earned, shapes and magnifiers owned, and a forward projection to a
   target level or hour budget (event-driven: research level-ups and insight level-ups cascade).
3. **Optimizers**, all local and parallel:
   - shape placement: greedy over the preset's target priorities, then hill-climbing on the real
     EXP rate (rotations unlock at research level 90, as in game);
   - magnifier assignment: kaleidoscopes → monocles → magnifying glasses, respecting per-observation
     caps and the observation grid's adjacency;
   - grid point spending: marginal-value climb over unlocked squares, with a refine pass that moves
     points one at a time;
   - one `Optimize` run that chains them and projects the result.
4. **Presets** — the sixteen upstream activity presets (Res EXP, Minehead, DR, Insight, Lumi/hr, …)
   as placement priorities.
5. **Agent-drivable UI** — every control carries a stable id, label, role and disabled state for the
   gpui-mcp bridge, so an agent can read and drive the window.

**Out of scope (v1)** — explicitly deferred, not forgotten:

- Save-file import (upstream reads the game save through Google sign-in; this app takes the
  save-derived multipliers as manual supplement inputs instead).
- The other calculators in the upstream repo (combat sim, sailing, cooking, royal guardian, …).
- Cloud sync, multi-account, mobile.

## 2. Stack and the compatibility matrix

The three libraries have to agree on exactly one GPUI snapshot, and the newest of each do not:

| Piece | Pin | Why |
| --- | --- | --- |
| GPUI | `gpui-pre = 0.3.9` (crates.io), patched to the bridge's vendored tree | the snapshot Ely and the bridge both build on |
| `gpui_platform` | `gpui-pre-platform = 0.3.9`, features `font-kit`, `wayland`, `x11` | opens windows and rasterises text |
| Ely components | `ely-gpui-component`, rev `28e5690` | newest rev pinning `gpui-pre = 0.3.9`, MSRV 1.95 |
| gpui-mcp | submodule `vendor/gpui-mcp`, branch `gpui-pre-0.3.9`, commit `ac15850` | vendors the accessibility additions the bridge reads |

Newer Ely pins `0.3.10`/`0.3.11` and the bridge's branch only patches up to `0.3.9`, so the pins
above are the intersection, not a preference. `[patch.crates-io] gpui-pre` points at
`vendor/gpui-mcp/vendor/gpui-pre`, so the app, the platform crate and the bridge compile against one
GPUI. `vendor/gpui-mcp` is excluded from this workspace so it keeps its own package metadata.

Bumping GPUI later means moving all three together; the smoke script is the check.

## 3. Architecture

```
crates/core                      no UI, no GPUI, fast to test
  data.rs                        game data (serde), parsed once
  model.rs                       GridState, Placement, Mag, Simulation, SimContext, requests
  shape.rs                       polygon geometry, 30 px coverage lattice, overlays
  sim.rs                         EXP rate, curves, insight, grid points, projection, cell values
  optimize/{shapes,mags,grid}.rs placement, assignment, point spending
  optimize/mod.rs                the run that chains them

crates/app                       GPUI + Ely
  main.rs                        application, window, theme
  state.rs                       AppState entity: simulation, context, run task, selection
  ui/shell.rs                    title bar, tab strip, status bar, pane layout
  ui/grid_view.rs                the 20×12 canvas, shapes, hit testing, keyboard
  ui/inspector.rs                selected square, shape list, rotation
  ui/dashboard.rs                rate breakdown, observation board
  ui/optimizer.rs                target, effort, run, results, projection chart
  ui/bridge.rs                   gpui-mcp installation

script/
  extract-research-data.mjs      upstream JS → crates/core/data/research.json
  mcp-smoke.mjs                  stdio MCP client: list apps, read the tree, screenshot
```

Rules: core never depends on GPUI, so it stays unit-testable and can be driven from a test harness;
UI files stay under ~500 lines and read state only through `AppState` methods.

## 4. Domain maths (ports, with sources)

From `js/stats/systems/w7/research-math.js`:

- observation base EXP: `(4 + t/2 + ⌊t/4⌋)·(1 + t^(1+0.4t/15)/10) + t^1.5 + 1.5t`
- insight requirement: `(2 + 0.7t)·(1.75 + t/200)^level·(1 + t²/100) + level`
- research requirement: `10·max(0,lv−1) + 10·(1 + lv^(1+0.4lv/10)/10)·1.1^lv·svrxp^max(0,lv−20)`
- grid bonus: `bonusPerLv · level · (1 + shapeBonus/100) · allBonusMulti`
- EXP/hour: `Σ observations (counts · base · (1 + gd93·insight/100) · kaleidoMulti) × (1 + additive/100) × (1 + takinNotes/100) × account multipliers`
- insight/hour: `3 · monocles · (1 + (gb91+gb92)/100) · kaleidoMulti`
- caps: magnifiers `min(4, round(1 + ⌊lv/40⌋₁ + ⌊lv/70⌋₁ + ⌊lv/120⌋₁))`, observations found
  `5⌊(lv+10)/10⌋ − ⌊lv/20⌋ − ⌊lv/30⌋ − ⌊lv/50⌋`, shapes: one per 20 levels.

From `js/optimizers/shapes-geo.js`: coverage is a point-in-polygon test on the four corners of each
30×30 cell, so it is periodic in `(x mod 30, y mod 30)`. The Rust port precomputes the same lattice
at a 5 px phase step (36 phases per shape/rotation instead of 900): the optimizer only proposes
lattice-aligned placements, and a test pins LUT coverage against exact geometry.

Deliberate simplifications, documented in code: account multipliers (companions, ribbons, event
shop) collapse into `SimContext::{extra_pct, extra_multiplier, all_bonus_multi}`, entered by hand.
Preset priorities use the presets' static target lists, not the save-dependent `build` functions.

## 5. UI

Window: Ely `TitleBar`, a tab strip (Grid / Rates / Optimize), the active pane, and a `StatusBar`
carrying EXP/hour, grid points, shape and magnifier counts.

- **Grid** — the canvas plus a right rail: the selected square (name, goal, description, level
  stepper, max level), and the shape list with bonus, covered cells and rotation nudge. Shapes draw
  as translucent polygons over a goal-coloured grid, with level badges.
- **Rates** — KPI cards (EXP/hour, multiplier, observation EXP, grid points, shapes, magnifiers),
  the observation board with insight level and insight/hour per slot, and the magnifier pool.
- **Optimize** — target (level or hours), effort (quick / balanced / thorough), run button with
  progress, before/after comparison, projection summary and the step list, apply/dismiss.

Every element that an agent might touch gets `.id(...)`; icon-only buttons get `.aria_label(...)`;
disabled controls get `.aria_disabled(true)`; read-only fields get `.aria_read_only(true)`.

## 6. Verification

1. `cargo test` — curves, geometry/LUT agreement, optimizer invariants (never worse than the input,
   respects caps and unlock rules, deterministic) and a timing guard on the thorough run.
2. `cargo build` then launch: `./target/debug/idleon-optimizer`.
3. `node script/mcp-smoke.mjs --shot /tmp/ui.png` — lists the running app through gpui-mcp, reads
   the semantic tree and saves a screenshot.
4. `node script/mcp-drive.mjs` — clicks the grid, a square, adds a shape, opens the optimize pane,
   runs it, asserts the gain is not negative, applies the result and screenshots the layout.

## 7. Work plan

1. **Scaffold and pin** — workspace, submodule, patches, minimal window, bridge. Done.
2. **Core model** — data extraction, geometry, simulation maths, baseline optimizers, tests. Done.
3. **UI** — three panes on Ely components, agent-ready ids, grid canvas with shape polygons. Done.
4. **Integration** — magnifier legality, apply/dismiss, status bar, optimizer gains proven
   non-negative by test. Done.
5. **Verification** — `cargo test`, `clippy`, gpui-mcp smoke and drive harness, screenshots. Done.

Still open, in priority order:

1. Save-file import: parse the upstream's decoded save JSON into `Simulation` and `SimContext`
   instead of hand-entered multipliers.
2. Shape dragging on the canvas (today: nudge and rotate from the inspector).
3. A projection chart on the optimize pane (the samples are already computed).
4. Further optimizer work: beam search over grid spending, two-shape swap moves, and a wider local
   search on rotations once profiles show where the time goes.

## 8. Risks

- **GPUI churn**: the pins are fragile by design; a bump touches three repos. Mitigated by exact
  pins and a smoke test that fails loudly.
- **Ely API drift**: components are early; the agent-facing ids and the compile check are the guard.
- **Formula drift from upstream**: extracted data is committed, and the cross-check tests evaluate
  the same formulas in Node.
- **Optimizer scope creep**: v1 optimises the research grid only; the rest of the upstream suite is
  explicitly out.
