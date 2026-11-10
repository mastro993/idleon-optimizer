# Idleon Optimizer

A native desktop rebuild of
[Corgan/idleon-research-optimizer](https://github.com/Corgan/idleon-research-optimizer) in Rust with
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) and
[Ely](https://elygpui.com) components. The upstream runs its simulations in browser workers; this app
runs them in a local Rust core, so a shape, magnifier and grid-point pass costs milliseconds and can
afford a real search instead of a heuristic.

The current build opens on the dashboard and implements the research section and the shadow-ban
check. The plan covers the
full upstream suite: a dashboard that opens every calculator, with one milestone per section and one
issue per feature ([`docs/roadmap.md`](docs/roadmap.md)).

![The research grid with shapes placed](docs/grid.png)

| Rates | Optimize |
| --- | --- |
| ![Rate breakdown and observations](docs/rates.png) | ![A finished run](docs/result.png) |

## What it does

- **The dashboard** — the 25 upstream calculators as a card grid grouped General through World 7,
  each with its icon, world badge and description. Search filters by title, description or world,
  Escape clears it, and every card has a stable `card-<section>` id that opens its section.
- **One section registry** — `src/sections/` declares every upstream page once: id, title, group,
  icon, description, spec link, status, dashboard card and the module that renders it. The shell
  routes to that module, keeps each section's own state alive across dashboard visits, and offers to
  resume the last section. Escape or Cmd/Ctrl+Left leaves any section; planned sections and unknown
  ids land on a named empty state instead of a panic. The five hidden upstream tools (All Stats,
  Damage, Drop Rate, Coin Multiplier, BoneJoePickle) are registered without cards and open with
  `--section <id>`.
- **One shell for every section** — the shared page header (back, title, world badge, save status,
  import), Ely tab strip, cards, tables and form controls, with the upstream dark palette. Below
  1360 px the side rails stack and tables scroll, so the 1080×680 minimum window stays usable.
- **Consistent states** — every section shows the shared no-save banner with its import action, the
  header names a partial import, long runs show a progress bar and Cancel, cancelling keeps the
  previous result, and results are marked stale instead of disappearing when an input changes.
- **The research grid** — the 20×12 board of upgradeable squares with their goal colours and levels,
  and the ten research shapes as real polygons over it: add, nudge, rotate (5° steps), remove.
  Arrow keys walk the squares, `=`/`-` level the selected one, Escape drops the selection.
- **Local simulation** — research EXP/hour with its additive and multiplicative breakdown, insight
  levelling per observation, magnifier caps, grid points earned and spent, shapes and magnifiers
  owned, and an event-driven projection to a research level or an hour budget.
- **Shared formula tree** — the Rates pane's EXP formula is one reusable, searchable breakdown:
  every node shows its inputs, multiplier, source and result, branches expand and collapse, search
  highlights matches and one copy button exports the whole tree. Every later calculator renders its
  breakdown through the same component.
- **Three optimizers, all in-process and parallel**
  - shape placement: greedy over the preset's target priorities, then a lattice search on the real
    EXP rate (moves, rotation sweeps once level 90 unlocks them, shape-order swaps);
  - magnifier assignment: two greedy orders kept best-of, then repair moves and swaps; kaleidoscopes
    weigh the neighbours that actually hold glasses, monocles go to the best insight ROI;
  - grid-point spending: marginal climb as the incumbent, a whole-layout beam search over the
    unlocked squares that wins when it can, then a refine pass that moves points; levels on the
    magnifier-grant squares are priced with the pool they unlock.
- **Sixteen presets** — Res EXP, Minehead, DR, Insight, Lumi/hr, Sushi, Gaming, Construction, … as
  placement priorities.
- **Supplements** — the save-derived multipliers that live outside the grid (`serverVarResXP`,
  all-bonus multiplier, extra account %, extra multiplier, flat grid points, extra shapes and
  magnifiers), editable in the Rates pane; the displayed rate follows each change.
- **Save import** — the header's Import button opens one dialog from the dashboard or any section:
  drop an `it.json`/`save.json` file, browse for one, or paste the JSON. It previews the detected
  format, research level, character count and warnings before Apply; the grid levels, shape
  positions, magnifiers, insight, research level/EXP, server scale, all-bonus multiplier, flat grid
  points and the decodable true multipliers are read from it. Fields the app cannot model are
  reported as warnings instead of guessed; a save that cannot be read only replaces the message and
  leaves the run untouched.
- **Shadow-ban check** — the upstream “Am I shadow banned?” page answers from `OptLacc[26]`: YES for
  any finite nonzero value, NO for zero, and UNKNOWN before a save or when the flag is missing. The
  page shows the save status and the flag it read under the verdict, so it never guesses.
- **Projection chart** — EXP/hour across the run, from an event-driven simulation that levels insight
  as it goes.

## Roadmap

The product target is a dashboard-first desktop suite with all 30 upstream pages, the same
information architecture and the same images where licensing allows. Progress is tracked as one
GitHub milestone per section and one issue per feature: 31 milestones and 123 feature issues.

- [`docs/plan.md`](docs/plan.md) — product, dashboard, JSON import, architecture and verification.
- [`docs/roadmap.md`](docs/roadmap.md) — generated milestone table and status.
- [`docs/roadmap.json`](docs/roadmap.json) — source of truth for milestones and issues.
- [`docs/sections/`](docs/sections/) — full specification per section.

```console
node script/roadmap.mjs --write   # regenerate the roadmap and section specs
node script/roadmap.mjs --apply --repo owner/name --dry-run
node script/roadmap.mjs --apply --repo owner/name
```

The sync is idempotent: milestones match by title, issues by feature-id prefix. The live tracker is
[tracker issue](https://github.com/mastro993/idleon-optimizer/issues/124),
[milestones](https://github.com/mastro993/idleon-optimizer/milestones) and
[issues](https://github.com/mastro993/idleon-optimizer/issues).

## Run

```console
git clone --recurse-submodules https://github.com/mastro993/idleon-optimizer
cd idleon-optimizer
cargo run --release
```

The first build compiles GPUI from source; expect a few minutes. `cargo run` opens the dashboard; the
Research Optimizer card opens the research section with a legal starter layout at research level
100 so every pane has something to show. `cargo run -- --section damage` opens a registry entry
directly, including the hidden tools and any unknown id (which shows the missing-section state).

## Stack pins

The app, the component kit and the automation bridge must share one GPUI build:

| Piece | Pin |
| --- | --- |
| GPUI | `gpui-pre = 0.3.9` (crates.io, patched to the bridge's vendored tree) |
| `gpui_platform` | `gpui-pre-platform = 0.3.9` with `font-kit`, `wayland`, `x11` |
| Ely | `ely-gpui-component`, rev `28e5690` |
| gpui-mcp | submodule `vendor/gpui-mcp`, branch `gpui-pre-0.3.9`, commit `ac15850` |

Newer Ely releases pin `gpui-pre 0.3.10`/`0.3.11` and the bridge patches up to `0.3.9`, so these are
the intersection. Bumping any one means moving all three together; the smoke script is the check.

## Verifying

```console
cargo test                                  # core maths, geometry and optimizer invariants
cargo build -p idleon-optimizer
node script/mcp-smoke.mjs --spawn --shot /tmp/ui.png  # own instance, tree + screenshot
node script/mcp-smoke.mjs --spawn --section damage     # a planned section's empty state
node script/mcp-smoke.mjs --spawn --section nope       # the unknown-section empty state
node script/mcp-drive.mjs                   # own instance, drives every flow end to end
node script/mcp-sections.mjs                # own instance, every section + screenshots
node script/mcp-sections.mjs --parallel 3   # three isolated lanes at once
node script/mcp-sections.mjs --baseline target/mcp-sections-baseline  # compare captures
```

All three harnesses launch their own window on a private bridge endpoint and stop it
afterwards, so several can run at once without seeing each other. Pass `--attach`
(drive) or drop `--spawn` (smoke) to drive a window you started with
`cargo run`. The app itself takes `--endpoint-dir <dir>` for that isolation, `--section <id>` to
open a registry entry at launch, and `--list-sections` to print the registry as JSON.

`script/mcp-drive.mjs` talks to the app through
[gpui-mcp](https://github.com/mastro993/gpui-mcp) over stdio: it asserts the dashboard exposes all
25 cards, that search filters and Escape clears it, that a card opens its section, that Escape and
Cmd/Ctrl+Left return to the dashboard, that Resume reopens the section with its own tab, that Back
returns, that the grid is exposed as semantic nodes, that a click selects a square, that dragging a shape
moves its squares, that the keyboard walks and levels the grid, that a run produces a result and
never loses EXP, that applying it clears the result, and that the shared import dialog previews a
save before applying it and cannot replace the run with one it cannot read. `script/mcp-sections.mjs`
reads the registry from the binary, opens every dashboard card and every hidden tool, asserts each
section root and shared header, drives one interaction and writes a screenshot plus a JSON sidecar
under `target/mcp-sections/`. `--baseline`
compares those captures with a previous run (pixels, not bytes), and `--parallel N` proves the lanes
stay isolated by checking shards in N windows at once. These scripts need the `gpui-mcp` binary
(`cargo install --git https://github.com/mastro993/gpui-mcp --branch gpui-pre-0.3.9 gpui-mcp-server`)
and a running app.

If background subagent launches fail with a missing `@earendil-works/chord`, run
`node script/install-pi-chord.mjs` (idempotent, re-run after a Pi upgrade) — see
[`docs/agent-harness.md`](docs/agent-harness.md).

To let an agent drive the window itself, the repo ships `.pi/mcp.json` registering the same server as
`gpui`; `get_ui_tree`, `click_element`, `type_text`, `screenshot` and `wait_for_element` then work
against this app. Every control in the UI carries a stable id for that purpose (`cell-129`,
`shape-add`, `shape-rotate-right`, `run-optimize`, `apply-result`, …).

## Layout

```
crates/core        game data, geometry, simulation maths, optimizers   (no GPUI)
  data/research.json  extracted from the upstream repo
crates/app         GPUI + Ely window: shell, dashboard and the section modules
  src/sections/      the registry plus one module per section (`research/`)
  src/ui/            window chrome, dashboard, shared section states, assets, bridge
  src/assets.rs      manifest-backed artwork: resolve, missing files, section previews
  src/state.rs       AppState, the route and each section's state
script/            data extraction, roadmap sync, MCP lane, smoke, drive and section harnesses
docs/plan.md       product plan, dashboard, JSON import, architecture, verification
docs/roadmap.json  milestones and feature issues (source of truth)
docs/sections/     generated full specification per section
fixtures/data.json synthetic account save for import tests
vendor/gpui-mcp    submodule pinned to the bridge's GPUI branch
```

## Data

`crates/core/data/research.json` is generated from the upstream JS by
`node script/extract-research-data.mjs <path-to-upstream-repo>`: 62 grid squares, 10 shapes with
vertices and dimensions, 43 observations, and the sixteen presets' target squares. Re-run it when
the upstream tables move.

`fixtures/data.json` is a full synthetic account save used by `crates/core/tests/fixture.rs` and for
manual debugging (open Import in the header and paste it). It is test input only: game formulas and save
decoding stay ports of the upstream repo, never inferred from the fixture. See
[`fixtures/README.md`](fixtures/README.md).

Upstream images are not committed. The upstream licence reserves all rights, so
`script/import-upstream-assets.mjs --from <checkout>` copies them into gitignored `assets/upstream/`
with a hashed manifest:

```console
node script/import-upstream-assets.mjs --from <checkout> --dry-run  # every file and total size
node script/import-upstream-assets.mjs --from <checkout>            # copy, manifest, verify
node script/import-upstream-assets.mjs --verify                     # re-check hashes on disk
```

The app reads `assets/upstream/manifest.json` and resolves every image reference through it. A
missing install or a deleted file draws a labelled placeholder in the same box; sections never
assume a sprite exists. `IDLEON_ASSETS_DIR` points the app at a different asset folder. The script
refuses to write inside a git worktree unless `.gitignore` covers the target, and never stages or
commits anything. Distributing a build with upstream assets or code needs written permission from
the upstream author.

## Scope

The current build covers the dashboard, the research grid, its optimizers and save import. The
roadmap covers all remaining upstream sections (combat sim, sailing, cooking, royal guardian, …);
their cards open planned-section placeholders until those milestones land.

The research formulas are ports of `js/stats/systems/w7/research-math.js`. Save-specific multipliers
collapse into `SimContext::{extra_pct, extra_multiplier, all_bonus_multi}`; the presets use their
static target lists rather than the save-dependent `build` functions.
