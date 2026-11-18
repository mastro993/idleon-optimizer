# UI performance: harness, baseline and budget

The UI kit migration (#335) is held to measured frame times, not impressions. `script/mcp-perf.mjs`
drives a private app window through gpui-mcp, records the draw time of every frame each scenario
causes, and checks the result against the budget below. `baseline.json` next to this page holds the
raw numbers for the Ely UI before the migration; every later PR reports its delta against it.

## Running it

```console
cargo build --release -p idleon-optimizer
node script/mcp-perf.mjs --release                                   # every scenario, ~2 minutes
node script/mcp-perf.mjs --release --only research,sidebar           # a subset
node script/mcp-perf.mjs --release --compare docs/ui-kit/baseline.json
node script/mcp-perf.mjs --release --check                           # exit 1 when over budget
node script/mcp-perf.mjs --release --json docs/ui-kit/baseline.json  # refresh the baseline
```

| Flag | Effect |
|---|---|
| `--release` | launch `target/release/idleon-optimizer` (default `target/debug`; a debug build is several times slower and says nothing about the budget) |
| `--app-bin PATH` | launch another binary |
| `--only A,B` | run these scenarios: `idle`, `home-scroll`, `search`, `hover`, `sections` (the default section list), `switch`, `sidebar`, `resize`, `run-away`, or any section id. Startup always runs |
| `--json FILE` | write the raw results (the `baseline.json` format) |
| `--compare FILE` | print each scenario's delta against a previous `--json` |
| `--check` | exit non-zero when a scenario is over budget. #362 turns this on as the gate |
| `--keep` | leave the window running afterwards |

It needs macOS, `gpui-mcp` on `PATH` and Accessibility access for the terminal (System Events
resizes the window and reads which app is frontmost). Like the other harnesses it owns a private
`--endpoint-dir` and `--state-dir`, stops only the window it started, and can run next to other lanes.
A scenario error, such as an element that never appeared, always exits non-zero.

## What it measures

Each scenario runs its setup unmeasured, then calls `mark_frames`, performs the action, waits for the
UI to settle and reads `get_frame_report`. The numbers are GPUI's per-frame draw time (`draw_ms`:
layout, prepaint and paint of the window; the bridge's own share is under 0.2 ms), so a frame that
was never drawn does not count. p50/p95/max are over the frames of that scenario, and `>16.7` counts
frames that missed a 60 Hz vsync.

| Scenario | Action |
|---|---|
| `startup` | Import the fixture in a first launch so a session exists, then launch three more times restoring it (a user's launch). Reports the median spawn → first frame and spawn → Home with the decoded session |
| `home-idle-3s` | Home, no input for 3 s |
| `home-scroll` | 30 wheel events of 80 px on `dashboard-scroll` (15 down, 15 up) |
| `home-search-typing` | Focus `dashboard-search`, type `research` one key at a time |
| `home-hover` | Pointer across 7 dashboard cards (`pointer_move`) |
| `<section>-open` | Click `nav-<section>` until `section-<section>` shows, 1.5 s settle. The first frame still draws the page being left |
| `<section>-scroll` | 30 wheel events on the section's page scroll |
| `switch-10x` | research → royal-guardian → research, 10 times, through the sidebar |
| `sidebar-fold`, `sidebar-unfold` | Toggle the rail on Home, 1 s settle (the animation) |
| `window-resize` | 20 width steps 1180 → 1920 px and back at 900 px high. macOS clamps the window to its screen; `widest_px` in the JSON says how far it got |
| `run-away-home`, `run-away-killroy` | Start Worship TD's strategy search (about 3.5 s), open Home or Killroy, and watch 1 s of its progress. Over budget when the place on screen (`Page` or `Pane<HomeReads>`) renders at all: only the sidebar mark and the status line may follow a run (#363) |

The sections are `stats-all`, `research`, `spelunking`, `royal-guardian`, `jelly-operator`,
`worship-td`, `combat-sim`, `farming` and `killroy`: the heaviest ones in the first measurement plus
those #337 names. `worship-td` and `killroy` do not scroll at 1440 × 900, and Killroy shows its empty
state with the fixture, so their scroll rows have no frames.

The first frame is read once the bridge answers, so it is an upper bound: the window may have drawn
earlier.

### Focus

macOS stops drawing an occluded window, and every lane activates itself when it opens. The harness
activates its own window before each scenario and samples the frontmost app while it runs. If another
app took focus, the scenario is retried up to twice; a result still taken without focus is printed
with `(focus lost)` and carries `focus_lost: true` in the JSON.

## Budget

Acknowledged by the owner on 2026-11-18 (in the #337 work thread):

- **Every interaction** (scroll, typing, navigation, section switching, hover, the sidebar animation
  and window resize), release build: **p95 ≤ 8.3 ms and max ≤ 16.7 ms** per frame, so 95% of frames
  fit one 120 Hz vsync and none misses two.
- **Idle: zero frames.** An idle UI must not repaint.
- **Cold start: first frame ≤ 500 ms.** The time until Home shows the decoded session is reported and
  compared but not budgeted, because it grows with the save.

`--check` applies exactly these rules (`BUDGET` in the script).

## Baseline

`baseline.json`: 2026-11-18, commit `c0179b4e` (Ely UI, before #336), release build, Apple M4 Pro,
macOS 26.7.1, window 1440 × 900 on the built-in 120 Hz display, fixture save. Times in ms.

| Scenario | Frames | p50 | p95 | max | >16.7 |
|---|---|---|---|---|---|
| startup: first frame ≤ 437, Home with the session 1,877 | 33 | 45.4 | 47.7 | 56.0 | 33 |
| home-idle-3s | 0 | – | – | – | 0 |
| home-scroll | 31 | 43.2 | 45.1 | 45.3 | 31 |
| home-search-typing | 15 | 19.5 | 42.9 | 42.9 | 8 |
| home-hover | 3 | 43.1 | 43.4 | 43.4 | 3 |
| stats-all-open (first visit) | 20 | 11.7 | 43.0 | **735.6** | 2 |
| stats-all-scroll | 31 | 12.5 | 12.9 | 13.3 | 0 |
| research-open | 24 | 30.2 | 43.6 | 50.4 | 24 |
| research-scroll | 31 | 30.4 | 31.9 | 32.2 | 31 |
| spelunking-open | 3 | 42.6 | 43.6 | 43.6 | 3 |
| spelunking-scroll | 31 | 30.7 | 31.8 | 31.8 | 31 |
| royal-guardian-open | 78 | 7.9 | 8.3 | 45.1 | 2 |
| royal-guardian-scroll | 31 | 8.2 | 8.4 | 8.5 | 0 |
| jelly-operator-open | 76 | 4.8 | 5.1 | 43.6 | 1 |
| jelly-operator-scroll | 31 | 4.5 | 4.9 | 4.9 | 0 |
| worship-td-open | 26 | 5.0 | 10.4 | 43.2 | 1 |
| combat-sim-open | 27 | 5.4 | 9.4 | 44.9 | 1 |
| combat-sim-scroll | 31 | 5.4 | 5.5 | 5.7 | 0 |
| farming-open | 27 | 5.0 | 12.4 | 43.2 | 1 |
| farming-scroll | 31 | 5.1 | 5.4 | 5.5 | 0 |
| killroy-open | 40 | 1.3 | 1.5 | 43.6 | 1 |
| switch-10x | 101 | 28.7 | 34.6 | 35.5 | 62 |
| sidebar-fold | 8 | 41.8 | 43.4 | 43.4 | 8 |
| sidebar-unfold | 8 | 42.2 | 44.0 | 44.0 | 8 |
| window-resize (to 1728 px, the screen width) | 60 | 43.6 | 45.7 | 46.6 | 60 |

20 of 27 scenarios are over budget. Idle and the cold-start first frame pass. The ~44 ms max on
every `-open` row is the click frame, which still draws Home. Runs on the same machine vary by about
±2 ms per frame and ±100 ms at startup.

## Where the time goes

Profiled with `sample` on the release build while the harness drove each scenario:

1. **Home: ~45 ms per frame, 97% of it Taffy layout.** `AppState` is the only view, so every notify
   re-renders and re-lays-out the whole window and no subtree is ever cached; render and paint are
   about 1% each, text measuring about 10%. It drags down scroll, typing, hover, the sidebar animation
   (about 22 fps), resize and the loader. The move to `gpui-pre` 0.3.12 (#366) brought Home to about
   2 ms per frame. #363 then split the window into cached views
   ([ADR 0004](../adr/0004-the-window-renders-cached-views-that-follow-what-they-read.md)), so a
   notify redraws only the views that read what changed, and a run elsewhere leaves the place on
   screen alone (`run-away`).
2. **stats-all, first visit: a 736 ms frame.** `stats_all::view` calls `State::rebuild`, which runs
   `idleon_stats::breakdowns::Sheet::build` inside render; damage `evaluate` is about 75% of it. Warm
   frames are 12 ms with ~11 `TextInput`s re-rendered each. #359.
3. **research: ~30 ms per scroll frame, 86% Taffy layout.** Four `FormulaTree`s and four
   `TextInput`s re-render every frame; render itself is about 5%. #348, #343.
4. **spelunking: ~31 ms per scroll frame, 88% Taffy layout.** About 24 `TextInput`s re-render every
   frame. #352.
5. **Cold start.** The loader veils a full shell that re-lays-out every spinner frame (33 frames of
   ~45 ms while the session decodes). #347. A launch with no session file still applies `--import`
   before the first frame (~1.3 s with the fixture), on purpose since #334; that is why `startup`
   measures a restoring launch.

No image decode showed up, and paint is cheap: under 1.5 ms at p50 in every scenario, 5.8 ms at
worst.

## Harness notes

Problems hit while building the harness, and how the scripts handle them:

- **Socket path length.** The bridge listens on `<endpoint dir>/mcp-<16 hex>.sock`, which must fit
  `sun_path` (104 bytes on macOS). A long `TMPDIR` failed with `local socket name length exceeds
  capacity of sun_path of sockaddr_un`, so `script/mcp-lane.mjs` falls back to `/tmp` for the endpoint
  directory when `TMPDIR` is too long.
- **Screenshots.** The bridge's `screenshot` timed out (`native screenshot capture exceeded the 15
  second deadline`) on the release build. The perf harness takes none; `screencapture -l<window id>`
  works when one is needed.
- **Hover.** `hover_element` answers `does not support Hover` on dashboard cards; `pointer_move` works.
- **Resize.** The bridge has no resize tool; the harness resizes through System Events.
- **Focus.** Other lanes and apps steal focus; see [Focus](#focus).
