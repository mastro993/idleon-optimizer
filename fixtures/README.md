# Test fixtures

`data.json` is a complete synthetic IdleOn account save: 823 top-level keys, 11 characters, the
research rows (`Research`, `Lv0_*`, `Exp0_*`, …) and 32 statue rows per character in the raw
decoded shape the game writes.

It exists for testing and debugging only. The game formulas, save decoding and optimizer behaviour
come from the upstream [`Corgan/idleon-research-optimizer`](https://github.com/Corgan/idleon-research-optimizer)
repo; do not reverse-engineer logic from this file or tune behaviour to match it.

Used by `crates/import/tests/fixture.rs`, `crates/statue/tests/statue.rs` and
`crates/sneaking/tests/sneaking.rs`. To exercise the app by
hand, open the header's Import dialog and paste the file's contents, or drop the file on it.

`endgame-stats.json` is the upstream `js/endgame-stats.js` leaderboard audit of `data.json`: all 177
metrics with their group, label and value. Generate it from an upstream checkout with
`node script/endgame-stats-fixture.mjs <upstream-checkout>`; `crates/endgame/tests/endgame.rs` compares
the Rust port against it and lists the 21 metrics that still need the account stats engine.

Treat it as frozen. Add small purpose-built saves to `crates/import/tests/save.rs` for edge cases
instead of editing this one; regenerating it means updating the fixture test on purpose.

`killroy.json` is the upstream planner's forecast for `data.json` wrapped as `it.json` with
`serverVars.KillroySwap = 7`. Regenerate it with `node script/killroy-oracle.mjs <upstream-repo> 2`
after changing the wrap or the horizon; `crates/killroy/tests/killroy.rs` compares the port field for
field. Treat it as frozen for the same reason: it is the oracle's output, not this app's.

`monument.json` is the upstream Monument calculator's own output for `data.json`: the saved
Bravery and Wisdom snapshots (training sources, stats, milestones and permanent reward rows), the
reward charge for all three monuments at the saved wait, after 24 hours and after an extra hour,
seven Bravery run states covering boost stacks, spent rethrows and an entered last-sword HP, and
the inputs it rejects with their messages. Regenerate it with
`node script/monument-oracle.mjs <upstream-repo>`; `crates/monument/tests/monument.rs` compares the
port field for field. Treat it as frozen for the same reason: it is the oracle's output, not this
app's.

`coral-plan.json` is the upstream Coral God Rank planner's own output for `data.json`: the
section's defaults, four beam-search cases (the fixture plan at the default and a one-state beam,
plus a small crafted plan where a wide beam has to beat a narrow one) and the offering model at
eight God Ranks. Regenerate it with
`node script/coral-god-rank-oracle.mjs <upstream-repo>`; `crates/coral/tests/coral_plan.rs` compares
the port action by action. Treat it as frozen for the same reason: it is the oracle's output, not
this app's.

`jelly.json` is the upstream Jelly Operator calculator's own output for `data.json`: the decoded
board state and multipliers, the seeded estimate and its first trial, two single simulator operations,
the progression outlook, five board section plans (one token, three tokens at beam widths 2 and 8,
a low-risk strategy and a zero-token plan), and the three estimate cases' upgrade rows, 72 reward
rows and six breakdown trees. Regenerate it with
`node script/jelly-oracle.mjs <upstream-repo>`; `crates/jelly/tests/jelly.rs`,
`crates/jelly/tests/jelly-estimate.rs`, `crates/jelly/tests/jelly-planner.rs` and
`crates/jelly/tests/jelly-upgrades.rs` compare the port field for field. Treat it as frozen for the same reason: it is the oracle's output, not this app's.

`farming.json` is the upstream Farming planner's own output for one purpose-built save: the
account snapshot, the plot assignment for all seven objectives and the Land Rank allocation in both
modes, with the points, scores, levels and change list. Regenerate it with
`node script/farming-oracle.mjs <upstream-repo>`; `crates/farming/tests/farming_planner.rs` compares
the port field for field. Treat it as frozen for the same reason: it is the oracle's output, not
this app's.

`farming-signals.json` is the upstream Farming Planner's own output for the signals, thresholds,
curves and evolution chains, and it carries the save it was evaluated on. The upstream account
snapshot reads a dozen systems this port deliberately does not apply; the fixture's purpose-built
save leaves every one of them out, so they fall back to their neutral values and the two
calculators agree term for term: the account multipliers, the priority diagnostics, the GMO counts,
the seven growth breakpoints, all 36 plots' metrics, all 36 overgrowth curves, the ten Crop
Scientist bonuses and 22 evolution chains (a family's first crop, one step, the first eight depths
and a thirty-eight-step climb). Regenerate it with
`node script/farming-signals-oracle.mjs <upstream-repo>`; `crates/farming/tests/farming_signals.rs`
compares the port field for field. Treat it as frozen for the same reason: it is the oracle's
output, not this app's.

`sneaking.json` is the upstream Sneaking calculator's own output for `data.json`: the saved roster
(twin placements, untie state and four equipment rows per twin), the parsed 39-slot inventory, the
Gold inventory scan with the charm it selects per bonus type, the charm bonus map and action speed
per twin, and the pure formulas (Ninja Knowledge bonuses, action speed, KO recovery, detection
bases and chances, stealth, door HP and the target-stealth inverse) over the save's own rows and
over the whole `NjEQ` catalog. Regenerate it with `node script/sneaking-oracle.mjs <upstream-repo>`;
`crates/sneaking/tests/sneaking.rs` compares the port field for field. Treat it as frozen for the same
reason: it is the oracle's output, not this app's.

`drop-rate.json` is the upstream Drop Rate calculator's own output for `data.json`: seven dungeon
totals over the two live run values, the reason a dungeon map gives when either is blank, and the
reference total for the character's saved map, which needs the account stats engine. Regenerate it
with `node script/drop-rate-oracle.mjs <upstream-repo>`; `crates/drop-rate/tests/drop_rate.rs` compares
the port number for number. Treat it as frozen for the same reason: it is the oracle's output, not
this app's.

`sailing-simulation.json` is the upstream sailing pile simulator's own output for a purpose-built
three-boat save: two cases (an optimized fleet over a day of arrivals and a current-save fleet with a
tier-three filter), each with the configuration the calculator builds and the full result — event
counts, pile summaries, slot statistics, tier composition and the three currency distributions.
`script/sailing-simulation-oracle.mjs` replaces `Math.random` with the same seeded xorshift stream the
Rust port uses, so the upstream output is reproducible; regenerate it with
`node script/sailing-simulation-oracle.mjs <upstream-repo>`. `crates/sailing/tests/sailing_simulation.rs`
compares the port field for field.

`fountain.json` is the upstream Fountain calculator's own output for `data.json`: the summary,
income rows, lucky coins and ducks, cross bonuses, ground stacks, level plan, currency values,
per-water multipliers, unlock tracker, upgrade rows and cost curves, plus the planner's ranked
candidates for all thirteen goals and four optimizer plans (currency and royal affordable, fill
ranked by hours, and optimal with waits for unearned currency). Regenerate it with
`node script/fountain-oracle.mjs <upstream-repo>`; `crates/fountain/tests/fountain.rs` compares the
port field for field. Treat it as frozen for the same reason: it is the oracle's output, not this
app's.

`coin-multi.json` is the upstream Coin Multiplier calculator's own output, from
`js/stats/defs/coin-multi.js` run over `data.json` and over one purpose-built save whose unmodelled
sources are all neutral. It carries every character's map, AFK target, monster HP, computed damage,
overkill tier and the 23 multiplicative groups, so the port's own overkill tier and every resolved
group are compared against upstream, and the simple save's total is compared end to end. Regenerate
it with `node script/coin-multi-oracle.mjs <upstream-repo>`; `crates/coin-multi/tests/coin_multi.rs`
compares the port field for field. Four groups still need systems this build does not carry (the
shared stat pipeline's total STR/AGI/WIS, talents and card/golden-food bonuses, and Divinity's bubble
chain); the port reports them as not modelled instead of guessing.
Treat it as frozen for the same reason: it is the oracle's output, not this app's.

`damage.json` is the upstream Damage Breakdown's own output for `data.json`, from
`js/stats/defs/damage.js` run over every character through `createStatContext`: the character's map
and class, the maximum and minimum damage, the mastery ratio, the three damage layers, and the
descriptor's `partial` flag and reason for the companion roster and server vote this save does not
carry, followed by the same save with both fields present, the named `MapDispName` rows the map
selector lists, and the descriptor's reason for all six dungeon maps. Regenerate it with
`node script/damage-oracle.mjs <upstream-repo>`; `crates/damage/tests/damage.rs` recombines the
upstream layers through the port's own summary arithmetic for every character, matches the
metadata flags and the map rows, and reports the missing stat pipeline until the three layers can
be resolved. Treat it as frozen for the same reason: it is the oracle's output, not this app's.

`clamworks-scurge.json` is the upstream Clamworks Scurge planner's own output for `data.json`, from
`js/stats/systems/w7/clamworks-scurge.js` run over the save with each case's `Menagerie` row: nine
cases across the Self Affliction comparison and the optimized EXP allocation, with the locked
Self Affliction tier, the three unlocked EXP upgrades, both refusals and every row's levels, level
changes, unspent points and EXP per kill. Regenerate it with
`node script/clamworks-scurge-oracle.mjs <upstream-repo>`; `crates/clamworks/tests/clamworks_scurge.rs`
compares the port field for field. Treat it as frozen for the same reason: it is the oracle's output,
not this app's.
