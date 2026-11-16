# Wiring landed providers into their consumers

Several sections still name a gap that a shared provider has since closed: the All-Talent resolver
(`idleon_account::talent::bonus_levels`, a port of upstream `resolveAllTalentLVz`), the shared stat
engine (`crates/stats-engine`) and the Gambit point chain (`crates/account/src/hole.rs`). This page
is the backlog for wiring them in. Each item is a small PR that follows its recipe below, and none of
them ports a new upstream formula.

Verified on 2026-11-15 against the live code and upstream
`Corgan/idleon-research-optimizer@cf9fe34`.

## 1. Inventory

| Consumer | Where the gap is named | Upstream term it needs | Result |
|---|---|---|---|
| All Stats `damage` row | `crates/stats/src/breakdowns/mod.rs` (`Sheet::build` fall-through) | `js/stats/defs/damage.js`: the row `val` is `maxDmg = DDL0 × DDL1 × DDL2`; `partial` is the missing companion/vote metadata | **Confirmed 1:1.** `damage_layers` matches every layer and `_debug` term for all eleven characters (`crates/damage/tests/damage.rs`). **Wired** (section 7). |
| All Stats capacity, talent 78 | `crates/stats/src/breakdowns/capacity.rs` (`TALENT_LEVELS`) | `js/stats/systems/common/carry-capacity.js:86`: `_resolved(talent, 78, ctx)`, which goes through `talent.js` and so `resolveAllTalentLVz` | **Confirmed 1:1** for 78. Talent 634 is above 614, which `resolveAllTalentLVz` excludes, so its half of the caption was never true. |
| Royal Guardian rates | `crates/royal-guardian/src/rates.rs` (`talent_value`) | `js/stats/systems/w7/royal-guardian.js:69`: `maxTalentBonusDetail(225/226/230/231/232)`, the max over characters of the bonus-levelled talent | **Adjacent.** `bonus_levels` gives one character's bonus levels; the consumer needs the per-character max of the curve at `purchased + bonus`. |
| Sneaking, talent 207 | `crates/sneaking/src/breakdown.rs` (Land Rank Stealth note) | `js/stats/systems/w6/farmRank.js:22`: `rval(talent, 207, …, { mode: 'max' })`, `getbonus2(1, 207, -1)` | **Adjacent.** Same max-over-characters shape as Royal Guardian. |
| Coin Multiplier "Other Additive Sources" | `crates/coin-multi/src/lib.rs` (`Spec::reason`) | `js/stats/defs/coin-multi.js` group g22 | **Caption already fixed.** `idleon_stats_engine::other_sources` is public and `crates/coin-multi/src/save.rs` sums it, flurbo shop included. Only the static `Spec::reason` text is stale. |
| Research AFK rate, Gambit Milestone 15 | `crates/research/src/sim.rs` (`AfkTerm "Gambit Milestone 15"`), `crates/research/src/breakdown.rs` | `js/stats/systems/w5/hole.js:194` `gambitBonus15`: `rawPts × gambitPTSmulti >= req ? 3 : 0` | **Confirmed 1:1** with `Decoder::gambit_points` (private). Needs `gambit_bonus15` beside `gambit_bonus7`. |

## 2. Wiring paths

The manifests decide the path. `stats-engine` sits above `account`; `stats` and `coin-multi` sit above
the engine; `royal-guardian`, `sneaking` and `research` sit below `account` or beside it, and
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

## 3. Public surface for the stat engine

Coin Multiplier needed one function per group, not raw providers, and already has it
(`cash_bubbles`, `other_sources`). Keep that rule for the next consumers:

- `talent_value(save, simulation, character, talent) -> Option<f64>`: the engine's talent provider
  (purchased + All-Talent levels through the curve), for capacity talent 78. Returns `None` unless
  the save carries the character.

Both follow `cash_bubbles`: return `None` unless every term is modelled. No `pub(crate)` provider
becomes public wholesale. The flurbo shop is no longer a blocker: `Ctx::flurbo_shop` is ported and
`other_sources` reads it.

## 4. Staying honest

- A caption is deleted only when the term it names is wired and pinned by an oracle.
- A value that still omits a term keeps a "Lower bound: <term> not modelled" partial reason. The
  `damage` row joins the descriptor's own missing-metadata reason with the engine's missing list, so
  neither hides the other.

## 5. Oracles and pins

| Consumer | Oracle script → fixture | Test that pins it |
|---|---|---|
| All Stats `damage` | `script/damage-oracle.mjs` → `fixtures/damage.json` (unchanged) | `crates/stats/tests/stats.rs` `the_damage_row_matches_the_upstream_descriptor` |
| Capacity talent 78 | none yet: `FIXTURE_CAPACITY` is pinned by hand; extend `script/stats-oracle.mjs` (today it writes the engine totals to `fixtures/stats.json`) to record the capacity rows first | `the_fixture_capacity_matches_the_upstream_stats_page` |
| Royal Guardian | `script/royal-oracle.mjs` → `fixtures/royal-guardian.json` | `crates/royal-guardian/tests/` |
| Sneaking | `script/sneaking-oracle.mjs` → `fixtures/sneaking.json` | `crates/sneaking/tests/` |
| Research Gambit 15 | `script/research-roi-oracle.mjs` → `fixtures/research-roi.json` | `crates/research/tests/` AFK rate |
| Coin Multiplier | `script/coin-multi-oracle.mjs` → `fixtures/coin-multi.json` | `crates/coin-multi/tests/coin_multi.rs` |

Regenerate a fixture only from a checkout whose other numbers are unchanged; if unrelated rows move,
that is upstream drift and belongs to its own PR.

## 6. Order

1. All Stats `damage` row: S. **Done.**
2. Coin Multiplier stale `Spec::reason`: S, text only.
3. Research Gambit Milestone 15: S. Public `gambit_bonus15` plus one `SimContext` field.
4. Capacity talent 78 through a public `talent_value`: S–M.
5. Royal Guardian and Sneaking talent levels through `import`: M. One shared "max bonus-levelled
   talent over characters" helper in `idleon-account` serves both.

## 7. The first wiring: All Stats `damage`

`crates/stats/src/breakdowns/damage.rs` answers the `damage` id. Its value is
`damage_layers(…).max` and its tree's three children are the base, percentage and multiplicative
layers, whose product is the row. A dungeon map resolves to the Damage section's own `DUNGEON` reason.
The descriptor's missing-metadata rule (`Metadata`, ported from `js/save/loader.js`) moved from
`idleon-damage` down into this module so the row and the Damage section share it; `idleon-damage`
re-exports it unchanged. The explorer ledger reads `15 of 60`.
