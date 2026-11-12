# Minehead (Depth Charge)

The research page's Minehead tab, ported from the upstream calculator. The formulas, the floor
simulation and the searches live in `crates/minehead`; the tab itself is
`crates/app/src/sections/research/minehead/`.

## Tabs

| Subtab | Upstream source | What it shows |
| --- | --- | --- |
| Dashboard | `js/ui/minehead-dashboard.js` (`renderDashboard`) | Floor, balance, grid size, lives, base damage, boss HP, depth charges, daily tries and currency/hr, the thirty upgrades with their next cost and requirement, and the first twenty-three floor rewards. |
| Optimize | `js/minehead/sim.js` (`optimizeStrategy`) + `js/ui/minehead-ui.js` | The knob search ranges, the combo count, and the top strategies by win rate and damage. |
| Rank Upgrades | `js/ui/minehead-ui.js` (`_runUpgradeRank`) | One baseline run, then one `+1` run per unlocked upgrade, sorted by damage per currency. |
| Upgrade Path | `js/ui/minehead-ui.js` (`_runUpgradePath`) | Spends the saved currency on the affordable upgrade with the best damage-per-cost each step. |
| Currency | `js/stats/defs/minehead-currency.js` | The currency/hr breakdown tree: every account factor with its source. |
| Play | `js/ui/minehead-play.js` | A playable floor. Each click is recorded; after three games the tab infers the knobs that explain the player's choices. |

## Save inputs

`idleon_minehead::inputs` reads the save once and returns the tab's inputs, warning about every
field it does not find:

- `Research[7]` (`stateR7` upstream): the current floor, the mine currency and the best hit.
- `Research[8]`: the thirty upgrade levels.
- `Research[9]`, `[10]`, `[11]`, `[19]`, `[20]` are static game tables, extracted from
  `js/stats/data/game/customlists.js` into `crates/minehead/data/minehead.json`.
- The account terms come from the shared `idleon_account::Decoder`: grid bonuses 125, 129, 146, 147,
  148, 166 and 167; companions 143 and 147; arcade 62; Atoms 13; event shop 44; the W7 currency task
  (`TaskZZ2[6][4]`); jelly rewards 8, 9, 45; Ring of Glory 1, 12 and 16; the Royal Guardian focus
  bonus; Spelunking Legend 40 and its `Spelunk[51]` Menagerie doublers; meals (`MineCurr`); and
  `Sailing[3][38]`.
- `serverVars.A_MineHP` / `A_MineCost` scale the boss health and the upgrade prices; both default to
  one, and the dashboard's server-var control edits the cost locally.

## Formulas

`crates/minehead/src/math.rs` ports `js/stats/systems/w7/minehead.js`: upgrade quantities and their
Menagerie doublers, upgrade costs and affordability, the grid size table, gold tiles, blocks, flags,
insta-reveals, daily tries, boss health, mines per floor, the base damage pipeline, per-tile damage,
blue crowns, jackpots, the seven-stage outgoing damage, currency/hr, wiggles, Glimbo costs and the
grid shape ranking. `getLOG` uses the game's truncated `ln 10`, as the shared curve module does.

## Simulation

`crates/minehead/src/sim.rs` is the upstream Monte Carlo, one game at a time:

- The seeded stream is `mulberry32`, and the calls are consumed in the upstream order, so a seeded
  run lands on the same numbers as the JS calculator, trial for trial.
- Grid generation rolls a fixed 72 tile values, places multiplier tiles (20–29), one jackpot (30) and
  currency tiles (40–49), overwrites random tiles with depth charges, then places blue crowns and
  golden tiles.
- A turn starts with the insta-reveals the strategy allows, then the strategy chooses between
  revealing a tile, clicking a golden tile and attacking. A depth charge costs a block, then a life;
  revival grants one block at one life. A cleared grid commits its damage by itself.
- Upstream's wiggle branch cannot fire: it requires `manualReveals === 0`, and the only manual reveal
  path increments the counter first. The port leaves it out and keeps no wiggle state.
- `Game` is the same state machine the Play tab drives, so the simulation and the interactive floor
  cannot drift apart.

## Searches

- `optimize` screens every combo in the search grid, re-runs the best twenty with 2,000 trials and
  ranks win rate first, damage as a fraction of boss HP second (`_optimizerScore`).
- `cost_path` is the Path tab's rule: damage per currency plus a small win-rate term, stopping when
  no unlocked upgrade is affordable.
- `rank_upgrades` is the Rank tab's rule: baseline plus one `+1` run per unlocked upgrade.
- `infer` sweeps the optimizer's grid against the recorded Play decisions, then refines ten
  human-behaviour knobs greedily for three passes, and labels the result with the upstream profile
  strings (`Daredevil`, `Crown-Chaser`, `Corner-Hugger`, …).

## Verification

```console
node script/extract-minehead-data.mjs /path/to/upstream   # rewrites data/minehead.json
node script/minehead-oracle.mjs /path/to/upstream         # rewrites tests/fixtures/upstream.json
cargo test -p idleon-minehead
```

The oracle fixture pins every formula on the fixture save's upgrade levels, the currency formula, the
shape ranking, two seeded 200-trial Monte Carlo runs (the Rust port reproduces them trial for trial),
a small strategy search, and the inferrer over forty recorded decisions. `tests/inputs.rs` pins the
fixture save's own rows and the empty-save warnings.

`docs/agent-harness.md` describes the research section check that opens every Minehead subtab and
plays one insta-reveal.

## Known limits

- The Play strategy infers from at least three finished games, as upstream; before that the Optimize
  tab shows no "Your strategy" card.
- The Knockback/weapon-power term (`wepPowDmgPCT`) is zero: the research page has no weapon context,
  and upstream's Minehead tab passes zero too.
- The `_minehead` shape preset now ranks its nodes from this crate's currency and damage terms when a
  save is loaded; with no save it keeps the static order.
