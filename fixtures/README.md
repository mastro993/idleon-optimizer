# Test fixtures

`data.json` is a complete synthetic IdleOn account save: 823 top-level keys, 11 characters, the
research rows (`Research`, `Lv0_*`, `Exp0_*`, …) and 32 statue rows per character in the raw
decoded shape the game writes.

It exists for testing and debugging only. The game formulas, save decoding and optimizer behaviour
come from the upstream [`Corgan/idleon-research-optimizer`](https://github.com/Corgan/idleon-research-optimizer)
repo; do not reverse-engineer logic from this file or tune behaviour to match it.

Used by `crates/core/tests/fixture.rs` and `crates/core/tests/statue.rs`. To exercise the app by
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
