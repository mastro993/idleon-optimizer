# Roadmap and progress tracking

<!-- Generated from docs/roadmap.json. Edit the JSON, then run node script/roadmap.mjs --write. -->

Idleon Optimizer tracks 31 section milestones and 121 feature issues.
Source repository: https://github.com/Corgan/idleon-research-optimizer.

## Milestones

| Milestone | Section | Group | Issues | Status | Spec |
| --- | --- | --- | --- | --- | --- |
| M0 | Foundation: dashboard, navigation, JSON import, assets | Core | 10 | in-progress | [foundation](sections/foundation.md) |
| M1 | Research Optimizer: full upstream parity | World 7 | 13 | partial | [research](sections/research.md) |
| M2 | Combat Simulator | General | 7 | planned | [combat-sim](sections/combat-sim.md) |
| M3 | All Stats Dashboard | General | 4 | planned | [stats-all](sections/stats-all.md) |
| M4 | Damage Breakdown | Tools | 3 | planned | [damage](sections/damage.md) |
| M5 | Drop Rate Calculator | Tools | 2 | planned | [drop-rate](sections/drop-rate.md) |
| M6 | Coin Multiplier Calculator | Tools | 2 | planned | [coin-multi](sections/coin-multi.md) |
| M7 | Shadow Ban Check | General | 1 | planned | [shadow](sections/shadow.md) |
| M8 | Endgame Check | General | 1 | planned | [endgame](sections/endgame.md) |
| M9 | Statue Calculator | General | 3 | planned | [statue](sections/statue.md) |
| M10 | Fishing Calculator | World 2 | 3 | planned | [fishing](sections/fishing.md) |
| M11 | Prisma Calculator | World 2 | 2 | planned | [prisma](sections/prisma.md) |
| M12 | Killroy Calculator | World 2 | 3 | planned | [killroy](sections/killroy.md) |
| M13 | Cog Board Optimizer | World 3 | 4 | planned | [cog](sections/cog.md) |
| M14 | Worship Tower Defense | World 3 | 4 | planned | [worship-td](sections/worship-td.md) |
| M15 | Ladle Optimizer | World 4 | 4 | planned | [ladle](sections/ladle.md) |
| M16 | Cooking Mastery | World 4 | 3 | planned | [cooking-mastery](sections/cooking-mastery.md) |
| M17 | Fountain Calculator | World 5 | 4 | planned | [fountain](sections/fountain.md) |
| M18 | Jar Calculator | World 5 | 4 | planned | [jar](sections/jar.md) |
| M19 | Sailing Calculator | World 5 | 4 | planned | [sailing](sections/sailing.md) |
| M20 | Cglunko Calculator | World 5 | 3 | planned | [cglunko](sections/cglunko.md) |
| M21 | Coral God Rank Planner | World 5 | 3 | planned | [coral-god-rank](sections/coral-god-rank.md) |
| M22 | Monument Calculator | World 5 | 4 | planned | [monument](sections/monument.md) |
| M23 | Opal Optimizer | World 5 | 3 | planned | [opal](sections/opal.md) |
| M24 | Sneaking Optimizer | World 6 | 4 | planned | [sneaking](sections/sneaking.md) |
| M25 | Farming Planner | World 6 | 5 | planned | [farming](sections/farming.md) |
| M26 | Clamworks Calculator | World 7 | 4 | planned | [clamworks](sections/clamworks.md) |
| M27 | Royal Guardian Calculator | World 7 | 4 | planned | [royal-guardian](sections/royal-guardian.md) |
| M28 | Spelunking Simulator | World 7 | 4 | planned | [spelunking](sections/spelunking.md) |
| M29 | Jelly Operator Optimizer | World 7 | 5 | planned | [jelly-operator](sections/jelly-operator.md) |
| M30 | BoneJoePickle Calculator | Tools | 1 | planned | [pickle](sections/pickle.md) |

## Tracking

`docs/roadmap.json` is the source of truth: one milestone per section and one issue per feature.
Validate and render with:

```console
node script/roadmap.mjs --write
```

Create the GitHub milestones and issues once the repository has a remote:

```console
node script/roadmap.mjs --apply --repo owner/name --dry-run
node script/roadmap.mjs --apply --repo owner/name
```

The sync is idempotent: milestones match by title and issues by their feature id prefix.
Close an issue to update GitHub progress; update the milestone status in `roadmap.json` when a
section is complete, then regenerate this page.
