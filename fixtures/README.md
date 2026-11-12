# Test fixtures

`data.json` is a complete synthetic IdleOn account save: 823 top-level keys, 11 characters, the
research rows (`Research`, `Lv0_*`, `Exp0_*`, …) and 32 statue rows per character in the raw
decoded shape the game writes.

It exists for testing and debugging only. The game formulas, save decoding and optimizer behaviour
come from the upstream [`Corgan/idleon-research-optimizer`](https://github.com/Corgan/idleon-research-optimizer)
repo; do not reverse-engineer logic from this file or tune behaviour to match it.

Used by `crates/core/tests/fixture.rs`, `crates/core/tests/statue.rs` and
`crates/core/tests/sneaking.rs`. To exercise the app by
hand, open the header's Import dialog and paste the file's contents, or drop the file on it.

`endgame-stats.json` is the upstream `js/endgame-stats.js` leaderboard audit of `data.json`: all 177
metrics with their group, label and value. Generate it from an upstream checkout with
`node script/endgame-stats-fixture.mjs <upstream-checkout>`; `crates/core/tests/endgame.rs` compares
the Rust port against it and lists the 21 metrics that still need the account stats engine.

Treat it as frozen. Add small purpose-built saves to `crates/core/tests/save.rs` for edge cases
instead of editing this one; regenerating it means updating the fixture test on purpose.

`killroy.json` is the upstream planner's forecast for `data.json` wrapped as `it.json` with
`serverVars.KillroySwap = 7`. Regenerate it with `node script/killroy-oracle.mjs <upstream-repo> 2`
after changing the wrap or the horizon; `crates/core/tests/killroy.rs` compares the port field for
field. Treat it as frozen for the same reason: it is the oracle's output, not this app's.

`monument.json` is the upstream Monument Bravery calculator's own output for `data.json`: the
saved snapshot (training sources, stats, milestones and permanent reward rows), seven run states
covering boost stacks, spent rethrows and an entered last-sword HP, and the inputs it rejects with
their messages. Regenerate it with `node script/monument-oracle.mjs <upstream-repo>`;
`crates/core/tests/monument.rs` compares the port field for field. Treat it as frozen for the same
reason: it is the oracle's output, not this app's.

`coral-plan.json` is the upstream Coral God Rank planner's own output for `data.json`: the
section's defaults, four beam-search cases (the fixture plan at the default and a one-state beam,
plus a small crafted plan where a wide beam has to beat a narrow one) and the offering model at
eight God Ranks. Regenerate it with
`node script/coral-god-rank-oracle.mjs <upstream-repo>`; `crates/core/tests/coral_plan.rs` compares
the port action by action. Treat it as frozen for the same reason: it is the oracle's output, not
this app's.

`farming.json` is the upstream Farming planner's own output for one purpose-built save: the
account snapshot, the plot assignment for all seven objectives and the Land Rank allocation in both
modes, with the points, scores, levels and change list. Regenerate it with
`node script/farming-oracle.mjs <upstream-repo>`; `crates/core/tests/farming_planner.rs` compares
the port field for field. Treat it as frozen for the same reason: it is the oracle's output, not
this app's.

`farming-signals.json` is the upstream Farming Planner's own output for the signals, thresholds and
curves, and it carries the save it was evaluated on. The upstream account snapshot reads a dozen
systems this port deliberately does not apply; the fixture's purpose-built save leaves every one of
them out, so they fall back to their neutral values and the two calculators agree term for term: the
account multipliers, the priority diagnostics, the GMO counts, the seven growth breakpoints, all 36
plots' metrics, all 36 overgrowth curves and the ten Crop Scientist bonuses. Regenerate it with
`node script/farming-signals-oracle.mjs <upstream-repo>`; `crates/core/tests/farming_signals.rs`
compares the port field for field. Treat it as frozen for the same reason: it is the oracle's
output, not this app's.

`sneaking.json` is the upstream Sneaking calculator's own output for `data.json`: the saved roster
(twin placements, untie state and four equipment rows per twin), the parsed 39-slot inventory, the
Gold inventory scan with the charm it selects per bonus type, the charm bonus map and action speed
per twin, and the pure formulas (Ninja Knowledge bonuses, action speed, KO recovery, detection
bases and chances, stealth, door HP and the target-stealth inverse) over the save's own rows and
over the whole `NjEQ` catalog. Regenerate it with `node script/sneaking-oracle.mjs <upstream-repo>`;
`crates/core/tests/sneaking.rs` compares the port field for field. Treat it as frozen for the same
reason: it is the oracle's output, not this app's.

`drop-rate.json` is the upstream Drop Rate calculator's own output for `data.json`: seven dungeon
totals over the two live run values, the reason a dungeon map gives when either is blank, and the
reference total for the character's saved map, which needs the account stats engine. Regenerate it
with `node script/drop-rate-oracle.mjs <upstream-repo>`; `crates/core/tests/drop_rate.rs` compares
the port number for number. Treat it as frozen for the same reason: it is the oracle's output, not
this app's.
