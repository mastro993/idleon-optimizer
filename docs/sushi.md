# Sushi Station

The research page's Sushi Station tab. The formulas, the save decode and the upgrade ranking live in
`crates/sushi`; the tab itself is `crates/app/src/sections/research/sushi/`.

## Tabs

| Subtab | What it shows |
| --- | --- |
| Overview | Unique sushi, active slots, highest tier, max cook tier, bucks/hr, currency multiplier, combo meter, fuel generation and capacity, the bank, sparks, Ring of Glory unlocks and upgrade slots, the 15 × 8 slot grid with its fireplace row, and the tier map. |
| Upgrades | The bank and its three daily rates, then every upgrade in unlock order with its research requirement, level, bonus, next cost, the time the bank needs and the filled-in description. The spoiler toggle shows the upgrades past the next unlock. |
| Currency/hr | The bucks tree (multiplier with every account factor, then the per-slot totals), the fuel generation tree and the fuel capacity tree, plus the time the capacity takes to fill. |
| Best Upgrade | For bucks/hr, fuel/hr and fuel capacity: the affordable upgrade path the bank buys greedily by gain per buck, and the next-level ranking with each upgrade's gain and efficiency. |
| Sushi | The knowledge category totals and every sushi tier with its perfecto odds, knowledge level and bonus and its Ring of Glory reward. The spoiler toggle shows the tiers neither discovered nor past the next Ring of Glory. |

## Save inputs

`idleon_sushi::inputs` reads the save once and returns the tab's inputs:

- `Sushi[0]` the sushi tier in each of the 120 slots (`-1` empty), `[1]` the slot type (normal, hot,
  cold, milktoast), `[2]` the upgrade levels, `[3]` the fireplace type of each of the fifteen
  columns, `[4]` the spa amount, sparks, bank and combo score, `[5]` the discovery state of each
  tier and `[7]` its knowledge level.
- The unique-sushi count is recomputed from `Sushi[5]`: the run of non-negative tiers.
- The account terms come from the shared `idleon_account::Decoder`: grid bonuses 188 and 189, arcade
  67, minehead floor 11, Atom 14, gaming super bit 67, Button slot 2, jelly reward 18, event shop 66,
  the `bon_v` bundle, and the `Spelunk[52]` upgrade doublers. `Sailing[3][39]` is the artifact.
- A field the save does not carry warns and reads as its neutral value; the tab renders empty.

## Formulas

`crates/sushi/src/math.rs` holds the formulas: the Ring of Glory array and its per-tier quantities,
the unique-sushi count, the currency-per-tier curve, fuel cost per tier, the upgrade level
requirement and cost, upgrade quantities with their Menagerie doublers, the knowledge categories and
their totals, fuel generation and capacity, the orange-fire sum, the fireplace and slot effect
bases, the per-slot and total bucks, the combo multiplier, the overtuned multiplier, the slot and
cook-tier counts, the free-shaker chance, saffron hours, perfecto odds, knowledge EXP and the
fireplace effects by type. The base-10 log uses the game's truncated `ln 10`, as the shared curve
module does.

`crates/sushi/src/lib.rs` fills the `{`, `}` and `$` markers in the upgrade descriptions and the
knowledge category text.

## Ranking

`crates/sushi/src/optimize.rs` drives the Best Upgrade subtab:

- `candidates` ranks the next level of every unlocked upgrade by its metric gain per buck.
- `path` spends the bank greedily: each step buys the affordable upgrade whose level-up raises the
  metric most per buck, up to fifty steps, then groups the purchases by upgrade.

## Verification

```console
cargo test -p idleon-sushi
```

`tests/formula.rs` checks the Ring of Glory table, the Menagerie doublers and an empty station;
`tests/inputs.rs` pins the fixture save's own rows and the empty-save warnings.

The research harness check opens every Sushi Station subtab and toggles both spoilers
(`docs/agent-harness.md`).

## Known limits

- The Ring of Glory rows feed research through the shared decoder, not through this crate: the
  research `SimContext` already carries `rog[0]` in the EXP multiplier, `rog[3]` and `rog[13]` in the
  grid-point sources, `rog[4]` and `rog[24]` in the AFK pool, `rog[8]` in the magnifier count and
  `rog[53]` in the all-bonus multiplier (`crates/import/tests/fixture.rs`).
- Upgrade descriptions keep the game text's `@` placeholder in the Fuel Capacity rows unfilled.
