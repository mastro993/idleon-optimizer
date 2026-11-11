# Roadmap and progress tracking

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

Idleon Optimizer tracks 31 section milestones and 123 feature issues.
Source repository: https://github.com/Corgan/idleon-research-optimizer.

The GitHub issue tracker is the working plan. Every feature issue carries its blocked-by and
blocks links, upstream sources, section contract, acceptance criteria and verification.

## Milestones

| Milestone | Section | Group | Issues | Status | Depends on | Spec |
| --- | --- | --- | --- | --- | --- | --- |
| M0 | Foundation: dashboard, navigation, JSON import, assets | Core | 12 | in-progress | - | [foundation](sections/foundation.md) |
| M1 | Research Optimizer: full upstream parity | World 7 | 13 | partial | foundation | [research](sections/research.md) |
| M2 | Combat Simulator | General | 7 | partial | foundation | [combat-sim](sections/combat-sim.md) |
| M3 | All Stats Dashboard | General | 4 | in-progress | foundation | [stats-all](sections/stats-all.md) |
| M4 | Damage Breakdown | Tools | 3 | planned | foundation, stats-all | [damage](sections/damage.md) |
| M5 | Drop Rate Calculator | Tools | 2 | planned | foundation, stats-all | [drop-rate](sections/drop-rate.md) |
| M6 | Coin Multiplier Calculator | Tools | 2 | planned | foundation, stats-all | [coin-multi](sections/coin-multi.md) |
| M7 | Shadow Ban Check | General | 1 | done | foundation | [shadow](sections/shadow.md) |
| M8 | Endgame Check | General | 1 | done | foundation | [endgame](sections/endgame.md) |
| M9 | Statue Calculator | General | 3 | partial | foundation | [statue](sections/statue.md) |
| M10 | Fishing Calculator | World 2 | 3 | partial | foundation | [fishing](sections/fishing.md) |
| M11 | Prisma Calculator | World 2 | 2 | partial | foundation | [prisma](sections/prisma.md) |
| M12 | Killroy Calculator | World 2 | 3 | partial | foundation | [killroy](sections/killroy.md) |
| M13 | Cog Board Optimizer | World 3 | 4 | partial | foundation | [cog](sections/cog.md) |
| M14 | Worship Tower Defense | World 3 | 4 | partial | foundation | [worship-td](sections/worship-td.md) |
| M15 | Ladle Optimizer | World 4 | 4 | partial | foundation | [ladle](sections/ladle.md) |
| M16 | Cooking Mastery | World 4 | 3 | planned | foundation | [cooking-mastery](sections/cooking-mastery.md) |
| M17 | Fountain Calculator | World 5 | 4 | planned | foundation | [fountain](sections/fountain.md) |
| M18 | Jar Calculator | World 5 | 4 | partial | foundation | [jar](sections/jar.md) |
| M19 | Sailing Calculator | World 5 | 4 | partial | foundation | [sailing](sections/sailing.md) |
| M20 | Cglunko Calculator | World 5 | 3 | partial | foundation | [cglunko](sections/cglunko.md) |
| M21 | Coral God Rank Planner | World 5 | 3 | partial | foundation | [coral-god-rank](sections/coral-god-rank.md) |
| M22 | Monument Calculator | World 5 | 4 | planned | foundation | [monument](sections/monument.md) |
| M23 | Opal Optimizer | World 5 | 3 | planned | foundation | [opal](sections/opal.md) |
| M24 | Sneaking Optimizer | World 6 | 4 | planned | foundation | [sneaking](sections/sneaking.md) |
| M25 | Farming Planner | World 6 | 5 | partial | foundation | [farming](sections/farming.md) |
| M26 | Clamworks Calculator | World 7 | 4 | partial | foundation, combat-sim | [clamworks](sections/clamworks.md) |
| M27 | Royal Guardian Calculator | World 7 | 4 | planned | foundation | [royal-guardian](sections/royal-guardian.md) |
| M28 | Spelunking Simulator | World 7 | 4 | planned | foundation | [spelunking](sections/spelunking.md) |
| M29 | Jelly Operator Optimizer | World 7 | 5 | planned | foundation | [jelly-operator](sections/jelly-operator.md) |
| M30 | BoneJoePickle Calculator | Tools | 1 | done | foundation | [pickle](sections/pickle.md) |

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
