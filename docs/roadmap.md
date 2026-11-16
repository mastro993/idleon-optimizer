# Roadmap and progress tracking

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

Idleon Optimizer tracks 29 section milestones and 121 feature issues.

The GitHub issue tracker is the working plan. Every feature issue carries its blocked-by and
blocks links, section contract, acceptance criteria and verification.

## Milestones

| Milestone | Section | Group | Issues | Status | Depends on | Spec |
| --- | --- | --- | --- | --- | --- | --- |
| M0 | Foundation: dashboard, navigation, JSON import, assets | Core | 12 | in-progress | - | [foundation](sections/foundation.md) |
| M1 | Research Optimizer: complete section | World 7 | 13 | partial | foundation | [research](sections/research.md) |
| M2 | Combat Simulator | General | 7 | partial | foundation | [combat-sim](sections/combat-sim.md) |
| M3 | All Stats Dashboard | General | 4 | in-progress | foundation | [stats-all](sections/stats-all.md) |
| M4 | Damage Breakdown | Tools | 3 | done | foundation, stats-all | [damage](sections/damage.md) |
| M5 | Drop Rate Calculator | Tools | 2 | partial | foundation, stats-all | [drop-rate](sections/drop-rate.md) |
| M6 | Coin Multiplier Calculator | Tools | 2 | done | foundation, stats-all | [coin-multi](sections/coin-multi.md) |
| M9 | Statue Calculator | General | 3 | partial | foundation | [statue](sections/statue.md) |
| M10 | Fishing Calculator | World 2 | 3 | done | foundation | [fishing](sections/fishing.md) |
| M11 | Prisma Calculator | World 2 | 2 | partial | foundation | [prisma](sections/prisma.md) |
| M12 | Killroy Calculator | World 2 | 3 | done | foundation | [killroy](sections/killroy.md) |
| M13 | Cog Board Optimizer | World 3 | 4 | partial | foundation | [cog](sections/cog.md) |
| M14 | Worship Tower Defense | World 3 | 4 | partial | foundation | [worship-td](sections/worship-td.md) |
| M15 | Ladle Optimizer | World 4 | 4 | done | foundation | [ladle](sections/ladle.md) |
| M16 | Cooking Mastery | World 4 | 3 | done | foundation | [cooking-mastery](sections/cooking-mastery.md) |
| M17 | Fountain Calculator | World 5 | 4 | done | foundation | [fountain](sections/fountain.md) |
| M18 | Jar Calculator | World 5 | 4 | done | foundation | [jar](sections/jar.md) |
| M19 | Sailing Calculator | World 5 | 4 | done | foundation | [sailing](sections/sailing.md) |
| M20 | Cglunko Calculator | World 5 | 3 | done | foundation | [cglunko](sections/cglunko.md) |
| M21 | Coral God Rank Planner | World 5 | 3 | done | foundation | [coral-god-rank](sections/coral-god-rank.md) |
| M22 | Monument Calculator | World 5 | 4 | done | foundation | [monument](sections/monument.md) |
| M23 | Opal Optimizer | World 5 | 3 | done | foundation | [opal](sections/opal.md) |
| M24 | Sneaking Optimizer | World 6 | 4 | done | foundation | [sneaking](sections/sneaking.md) |
| M25 | Farming Planner | World 6 | 5 | partial | foundation | [farming](sections/farming.md) |
| M26 | Clamworks Calculator | World 7 | 4 | done | foundation, combat-sim | [clamworks](sections/clamworks.md) |
| M27 | Royal Guardian Calculator | World 7 | 4 | done | foundation | [royal-guardian](sections/royal-guardian.md) |
| M28 | Spelunking Simulator | World 7 | 4 | done | foundation | [spelunking](sections/spelunking.md) |
| M29 | Jelly Operator Optimizer | World 7 | 5 | done | foundation | [jelly-operator](sections/jelly-operator.md) |
| M30 | BoneJoePickle Calculator | Tools | 1 | done | foundation | [pickle](sections/pickle.md) |

## Blocker backlog

| Blocker | Status | Gaps it blocks |
| --- | --- | --- |
| `wire:all-talent` All-Talent bonus levels (idleon_account::talent::bonus_levels) passed to the sections that read talents | landed | SN-G1, RG-G1 |
| `wire:gambit` Gambit point chain from the account decoder (idleon_account hole.rs) passed to Sneaking and Research | landed | RES-G1, SN-G1 |
| `wire:stats-engine` Shared stat engine results (total stats, damage layers) consumed by the sections that name them as gaps | landed | - |
| `stats:descriptors` Remaining All Stats descriptors (15 of 60 computed) | partial | SA-G1, PR-G1 |
| `combat:auto-player` Combat simulator auto-mode player (movement, attacks, skills), the source of a character's kill rate and DPS | absent | CS-G1, ST-G1, KL-G1, CW-G1 |
| `stats:level-tables` Level-table catalogue and the combat-setup save writer behind the per-node formula editor | absent | SA-G3 |
| `stats:drop-rate-descriptor` The drop-rate pool chain descriptor in the shared stat engine | absent | DR-G1 |
| `stats:skill-descriptors` Fishing skill descriptors (efficiency, EXP multiplier, AFK rate, Multi-Fish, speed, prowess, equipment depths) | absent | FI-G1 |
| `stats:spelunking-descriptors` Spelunking stat context: Spelunking Power, amber multiplier, stamina regeneration, Grand Discovery chance | absent | SP-G1 |
| `model:royal-objectives` Royal Guardian optimizer objectives beyond the three the kingdom metrics carry (Armory target, currency portfolio, reset collection) | absent | RG-G2 |
| `model:sneaking-flower-pass` Sneaking flower pass behind the Monte-Carlo claim simulation | absent | SN-G2 |
| `ui:jar-sparkline` Sparkline chart behind each Jar history tile | absent | JR-G1 |

A `landed` blocker means its gaps can be closed now.

## Tracking

`docs/roadmap.json` is the source of truth: one milestone per section and one issue per feature.
Validate and render with:

```console
node script/roadmap.mjs --write
```

Sync the GitHub milestones and issues:

```console
node script/roadmap.mjs --apply --repo owner/name --dry-run
node script/roadmap.mjs --apply --repo owner/name
```

The sync is idempotent: milestones match by title and issues by feature id prefix. Existing
issues are updated in place, so dependency links and acceptance criteria stay current.
Close an issue to update GitHub progress; update the milestone status in `roadmap.json` when a
section is complete, then regenerate this page.
