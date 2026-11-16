# Wiring landed providers into their consumers

Several sections still name a gap that a shared provider has since closed: the All-Talent resolver
(`idleon_account::talent::bonus_levels`), the shared stat engine (`crates/stats-engine`) and the
Gambit point chain (`crates/account/src/hole.rs`). This page is the backlog for wiring them in. Each
item is a small PR that follows its recipe below, and none of them models a new formula.

Verified on 2026-11-15 against the live code.

## 1. Inventory

| Consumer | Where the gap is named | Term it needs | Result |
|---|---|---|---|
| All Stats `damage` row | `crates/stats/src/breakdowns/mod.rs` (`Sheet::build` fall-through) | The damage descriptor: the row value is the max damage, `DDL0 × DDL1 × DDL2`; `partial` is the missing companion/vote metadata | **Confirmed.** `damage_layers` supplies every layer and recorded term for all eleven characters. **Wired** (section 7). |
| All Stats capacity, talent 78 | `crates/stats/src/breakdowns/capacity.rs` (`TALENT_LEVELS`) | Carry capacity reads talent 78 at its resolved level, purchased plus All-Talent bonus levels | **Confirmed** for 78. Talent 634 is above 614, which the All-Talent bonus excludes, so its half of the caption was never true. |
| Royal Guardian rates | `crates/royal-guardian/src/rates.rs` (`talent_value`) | Talents 225/226/230/231/232 at the max over characters of the bonus-levelled talent | **Adjacent.** `bonus_levels` gives one character's bonus levels; the consumer needs the per-character max of the curve at `purchased + bonus`. |
| Sneaking, talent 207 | `crates/sneaking/src/breakdown.rs` (Land Rank Stealth note) | The Land Rank Stealth bonus reads talent 207 at its max over characters | **Adjacent.** Same max-over-characters shape as Royal Guardian. |
| Coin Multiplier "Other Additive Sources" | `crates/coin-multi/src/lib.rs` (`Spec::reason`) | The coin multiplier's group 22 | **Caption already fixed.** `idleon_stats_engine::other_sources` is public and `crates/coin-multi/src/save.rs` sums it, flurbo shop included. Only the static `Spec::reason` text is stale. |
| Research AFK rate, Gambit Milestone 15 | `crates/research/src/sim.rs` (`AfkTerm "Gambit Milestone 15"`), `crates/research/src/breakdown.rs` | Gambit milestone 15: `raw points × Gambit points multiplier >= requirement ? 3 : 0` | **Confirmed** with `Decoder::gambit_points` (private). Needs `gambit_bonus15` beside `gambit_bonus7`. |
| Endgame `strength`/`agility`/`wisdom`/`luck` | `crates/endgame` (the not-modelled metrics) | The best character's `total-str` (and siblings) descriptor value | **Confirmed** with `total_stat` per character. |
| Endgame `cashMulti`, `dropRate`, `defence`, … | same | The best character's `coin-multi`, `drop-rate`, … value | **Adjacent.** Each needs its own descriptor modelled first; not a wiring. |
| Endgame `highestDamage` | same | The saved `Tasks[0][1][0]` | **Not an engine value.** It is a saved number and can be read without the engine. |

## 2. Wiring paths

The manifests decide the path. `stats-engine` sits above `account`; `stats` and `coin-multi` sit above
the engine; `royal-guardian`, `sneaking`, `research` and `endgame` sit below `account` or beside it, and
`account` depends on `sneaking` and `research`, so those two can never pull from it.

- **Direct call** (the consumer already depends on the provider):
  - All Stats `damage`: `idleon_stats_engine::damage_layers` from a new `breakdowns::damage` resolver.
    Done.
  - All Stats capacity talent 78: `capacity.rs` calls `talent(save, character, 78)` today. Route it
    through the engine's talent provider instead (needs the public surface in section 3), and drop
    634 from the caption.
  - Coin Multiplier: already direct; delete the stale `Spec::reason`.
- **Inject through the import layer** (`crates/import/src/lib.rs` builds every calculator's state and
  hands account values in):
  - Royal Guardian: `CollectionInputs` (`crates/royal-guardian/src/rates.rs`, re-exported from
    `lib.rs`) gains `talent_levels: BTreeMap<usize, f64>`, the max over characters of
    `purchased + bonus_levels(…)` for 225/226/230/231/232. `import` computes it with the account
    `Decoder`; `talent_value` reads it before the purchased level.
  - Sneaking: the account struct `breakdown.rs` reads gains `talent_207`, the same max; `import`
    fills it.
  - Research Gambit Milestone 15: `SimContext` gains `gambit_bonus15: f64`, filled in `import` from a
    new `Decoder::gambit_bonus15` (a public sibling of `gambit_bonus7` over the existing private
    `gambit_points`), exactly as `minehead` floors already travel.
  - Endgame stat metrics: the endgame input gains `best_totals: [Option<f64>; 4]`, the max over
    characters of `total_stat`, computed where `import` already builds the endgame state.

## 3. Public surface for the stat engine

Coin Multiplier needed one function per group, not raw providers, and already has it
(`cash_bubbles`, `other_sources`). Keep that rule for the next consumers:

- `talent_value(save, simulation, character, talent) -> Option<f64>`: the engine's talent provider
  (purchased + All-Talent levels through the curve), for capacity talent 78. Returns `None` unless
  the save carries the character.
- `best_total(save, simulation, stat) -> Option<f64>`: the max of `total_stat` over characters,
  `None` if any character's total is incomplete, for Endgame.

Both follow `cash_bubbles`: return `None` unless every term is modelled. No `pub(crate)` provider
becomes public wholesale. The flurbo shop is no longer a blocker: `Ctx::flurbo_shop` is modelled and
`other_sources` reads it.

## 4. Staying honest

- A caption is deleted only when the term it names is wired and pinned by a test.
- A value that still omits a term keeps a "Lower bound: <term> not modelled" partial reason. The
  `damage` row joins the descriptor's own missing-metadata reason with the engine's missing list, so
  neither hides the other.
- Endgame's `Computed`/`Stat` is "value or not modelled" today. The smallest extension is a third
  state, `Partial { value, reason }`, rendered with the same warning the All Stats rows use; a
  best-total that is a lower bound uses it instead of claiming the metric.

## 5. Tests

Each wiring lands with a test in its consumer's crate that pins the wired value on
`fixtures/data.json` or a purpose-built save:

| Consumer | Tests |
|---|---|
| All Stats `damage` and capacity talent 78 | `crates/stats/tests/stats.rs` |
| Royal Guardian | `crates/royal-guardian/tests/` |
| Sneaking | `crates/sneaking/tests/` |
| Research Gambit 15 | `crates/research/tests/` AFK rate |
| Endgame stat metrics | `crates/endgame/tests/endgame.rs` |
| Coin Multiplier | `crates/coin-multi/tests/coin_multi.rs` |

## 6. Order

1. All Stats `damage` row: S. **Done.**
2. Coin Multiplier stale `Spec::reason`: S, text only.
3. Endgame `highestDamage` from the saved Tasks row: S, no engine.
4. Research Gambit Milestone 15: S. Public `gambit_bonus15` plus one `SimContext` field.
5. Capacity talent 78 through a public `talent_value`: S–M.
6. Royal Guardian and Sneaking talent levels through `import`: M. One shared "max bonus-levelled
   talent over characters" helper in `idleon-account` serves both.
7. Endgame `strength`/`agility`/`wisdom`/`luck` with a `Partial` state: M.

## 7. The first wiring: All Stats `damage`

`crates/stats/src/breakdowns/damage.rs` answers the `damage` id. Its value is
`damage_layers(…).max` and its tree's three children are the base, percentage and multiplicative
layers, whose product is the row. A dungeon map resolves to the Damage section's own `DUNGEON` reason.
The descriptor's missing-metadata rule (`Metadata`) moved from `idleon-damage` down into this module
so the row and the Damage section share it; `idleon-damage` re-exports it unchanged. The explorer
ledger reads `15 of 60`.
