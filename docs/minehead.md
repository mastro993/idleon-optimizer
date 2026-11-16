# Minehead (Depth Charge)

The research page's Minehead tab. The formulas, the floor simulation and the searches live in
`crates/minehead`; the tab itself is `crates/app/src/sections/research/minehead/`.

## Tabs

| Subtab | What it shows |
| --- | --- |
| Dashboard | Floor, balance, grid size, lives, base damage, boss HP, depth charges, daily tries and currency/hr, the thirty upgrades with their next cost and requirement, and the first twenty-three floor rewards. |
| Optimize | The knob search ranges, the combo count, and the top strategies by win rate and damage. |
| Rank Upgrades | One baseline run, then one `+1` run per unlocked upgrade, sorted by damage per currency. |
| Upgrade Path | Spends the saved currency on the affordable upgrade with the best damage-per-cost each step. |
| Currency | The currency/hr breakdown tree: every account factor with its source. |
| Play | A playable floor. Each click is recorded; after three games the tab infers the knobs that explain the player's choices. |

## Save inputs

`idleon_minehead::inputs` reads the save once and returns the tab's inputs, warning about every
field it does not find:

- `Research[7]`: the current floor, the mine currency and the best hit.
- `Research[8]`: the thirty upgrade levels.
- `Research[9]`, `[10]`, `[11]`, `[19]`, `[20]` are static game tables, kept in
  `crates/minehead/data/minehead.json`.
- The account terms come from the shared `idleon_account::Decoder`: grid bonuses 125, 129, 146, 147,
  148, 166 and 167; companions 143 and 147; arcade 62; Atoms 13; event shop 44; the W7 currency task
  (`TaskZZ2[6][4]`); jelly rewards 8, 9, 45; Ring of Glory 1, 12 and 16; the Royal Guardian focus
  bonus; Spelunking Legend 40 and its `Spelunk[51]` Menagerie doublers; meals (`MineCurr`); and
  `Sailing[3][38]`.
- `serverVars.A_MineHP` / `A_MineCost` scale the boss health and the upgrade prices; both default to
  one, and the dashboard's server-var control edits the cost locally.

## Formulas

`crates/minehead/src/math.rs` holds the formulas: upgrade quantities and their Menagerie doublers,
upgrade costs and affordability, the grid size table, gold tiles, blocks, flags, insta-reveals,
daily tries, boss health, mines per floor, the base damage pipeline, per-tile damage, blue crowns,
jackpots, the seven-stage outgoing damage, currency/hr, wiggles, Glimbo costs and the grid shape
ranking. The base-10 log uses the game's truncated `ln 10`, as the shared curve module does.

## Simulation

`crates/minehead/src/sim.rs` is the Monte Carlo, one game at a time:

- The seeded stream is `mulberry32`, so a seeded run is reproducible trial for trial.
- Grid generation rolls a fixed 72 tile values, places multiplier tiles (20–29), one jackpot (30) and
  currency tiles (40–49), overwrites random tiles with depth charges, then places blue crowns and
  golden tiles.
- A turn starts with the insta-reveals the strategy allows, then the strategy chooses between
  revealing a tile, clicking a golden tile and attacking. A depth charge costs a block, then a life;
  revival grants one block at one life. A cleared grid commits its damage by itself.
- The simulation has no wiggle branch and keeps no wiggle state: a wiggle would need a game with no
  manual reveals, and the only manual reveal path increments the counter first.
- `Game` is the same state machine the Play tab drives, so the simulation and the interactive floor
  cannot drift apart.

## Searches

- `optimize` screens every combo in the search grid, re-runs the best twenty with 2,000 trials and
  ranks win rate first, damage as a fraction of boss HP second (`optimize::score`).
- `cost_path` is the Path tab's rule: damage per currency plus a small win-rate term, stopping when
  no unlocked upgrade is affordable.
- `rank_upgrades` is the Rank tab's rule: baseline plus one `+1` run per unlocked upgrade.
- `infer` sweeps the optimizer's grid against the recorded Play decisions, then refines ten
  human-behaviour knobs greedily for three passes, and labels the result with profile strings
  (`Daredevil`, `Crown-Chaser`, `Corner-Hugger`, …).

## Verification

```console
cargo test -p idleon-minehead
```

`tests/formula.rs` checks the cost path and the seeded `mulberry32` stream on the fixture save;
`tests/inputs.rs` pins the fixture save's own rows and the empty-save warnings.

`docs/agent-harness.md` describes the research section check that opens every Minehead subtab and
plays one insta-reveal.

## Known limits

- The Play strategy infers from at least three finished games; before that the Optimize tab shows no
  "Your strategy" card.
- The Knockback/weapon-power term (`wep_pow_dmg_pct`) is zero: the research page has no weapon
  context.
- The `_minehead` shape preset now ranks its nodes from this crate's currency and damage terms when a
  save is loaded; with no save it keeps the static order.
