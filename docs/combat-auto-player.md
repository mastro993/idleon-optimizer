# Combat auto-mode player: sizing and design

A research spike (plan 023, issue #278). It sizes the upstream **auto-mode player**: the AI that
moves, auto-attacks, casts skills and fires projectiles. That player is what would turn a save into
the kill rates Clamworks, Killroy, the Statue calculator and BoneJoePickle still ask the user to type.
In `docs/roadmap.json` this player is the blocker `combat:auto-player`. It gates four tracked gaps:
combat DPS and survival, Statue's "Measure statue farming", Killroy's kills per second, and
Clamworks' kills per hour, drop rate and multikill. The missing stat formulas listed in §2 sit
under `stats:descriptors`. This document changes no code. It records what the port would take, whether an oracle can pin it,
the smallest useful slice, and a recommendation.

Written against upstream `Corgan/idleon-research-optimizer` at
**`cf9fe34561f46b4628d15d6f80636d771a558189`** (committed 2026-11-13). The upstream `LICENSE` still
reads "All rights reserved. This repository is provided solely for educational review…", the text
`docs/plan.md` §2 quotes, so any port stays a rewrite with source references. Every path below
is relative to the upstream checkout unless it starts with `crates/`, `script/`, `fixtures/` or
`docs/`.

All measurements come from scratch scripts run outside this repository. They load
`fixtures/data.json` through `js/save/loader.js`, build `createCombatSim` with
`autoPlayer()`, and use Node 24.14.1 on an Apple M4 Pro.

## 1. Size

### Files

The auto player is one module, `js/stats/systems/common/combat-sim-player.js`. The worker picks it
whenever a job asks for neither the idle nor the scripted player (`js/workers/combat-sim-worker.js:60-66`,
`makePlayer`). It plugs into the same engine core this repository already ports.

| File | Lines | Role for the auto player | Ported here |
| --- | ---: | --- | --- |
| `combat-sim-player.js` | 3,255 | the player itself (everything below) | no |
| `combat-sim-world.js` | 937 | monster movement, contact damage, `createMonster`, statuses, `damageMonster` | spawn/respawn/kill only (`crates/combat/src/encounter.rs`) |
| `combat-sim-core.js` | 801 | 10 ms step, timed tasks, frame batch, RNG, player contract | yes, minus the auto player (`encounter.rs`) |
| `combat-sim-inputs.js` | 1,041 | every save-derived scalar the player reads (`_playerInputs`, lines 851-1003) | no (see §2) |
| `combat-sim-rewards.js` | 1,632 | kill rewards; shares the RNG stream with the player | cards and obols only (`crates/combat/src/drops.rs`) |
| `combat-sim-special.js` | 849 | class/map kill mechanics (form drops, Clam Work pearls, golems) | no |
| `combat-forms.js` | 250 | Wraith / Tempest / Arcanist form stats | no |
| `combat-outcomes.js` | 660 | `computeHitChance`, `actionWaitTimeAfk`, `autoRespawnTime` | no |
| `combat-sim-telemetry.js` | 374 | optional decision trace (`options.telemetry`) | no; useful for the oracle |
| `combat-sim-replay.js` | 393 | replay codec | yes (`crates/combat/src/replay/`) |

Static data it reads: `js/stats/data/game/attacks.js` (`ATTACKS`, 93 skills),
`scenes.js` (`SCENE_GEOMETRY`, 152 scenes), `combat-sprites.js` (`BULLET_ANIMS`,
`SPRITE_DEFAULTS`, `CODE_IMAGES`, `PLAYER_ANIMS`), `items.js`, `monsters.js` and `customlists.js`.

### Entry points

`autoPlayer(opts)` (`combat-sim-player.js:3177`) returns the core's player module (`init`, `step`,
`createDmgPixel`; the contract is at `combat-sim-core.js:38-60`). `init` builds `sim.player` and
registers its timers. Registration order is firing order inside one 10 ms step:

| Timer | Interval | Function | Line |
| --- | ---: | --- | ---: |
| reset node-change count | 10,000 ms | inline | 3220 |
| target selection | 150 ms | `_findNewNode` | 2098 |
| movement (path, jump, ladder, drop) | 50 ms | `_movement` | 2162 |
| auto-attack trigger | 50 ms | `_actionsAttacks` | 2318 |
| attack resolution | 50 ms | `_results` | 2361 |
| stop moving | 150 ms | `_stopMoving` | 2308 |
| contact damage | 150 ms | `_takeDamage` → `world.contactDamage` | 2887 |
| attack-queue decrement | 50 ms | `_every005` | 2710 |
| cooldowns, MP regen, buff tick, respawn countdown | 1,000 ms | `_every1sec` | 2717 |
| skill auto-cast | 1,000 ms | `_autoCast` | 2753 |
| health food | 150 ms | `_consumeFoodNew` | 2655 |
| movement food | 1,000 ms | `_consumeFoodMove` | 2676 |
| all-mobs-dead respawn (buff 167) | 1,000 ms | `_allMobDead` | 2819 |
| spawn-skill pixel helper | 330 ms | `event025sec` | 1953 |
| attack queue (scene updater, every step) | 10 ms | `_attackQueueLoop` | 2536 |

`step(sim)` runs once per step, after the core integrates position: `_jumping` (2273), then
`_die` (2910).

### Call graph, one level deep

| Area | Functions (line) | Approx. lines |
| --- | --- | ---: |
| Helpers: distance, BFS pathfinder, weapon wait time, damage roll | `distanceEqn` 73, `pathfinder` 85, `actionWaitTime` 115, `rollDamage` 131 | 140 |
| Target selection | `_findNewNode` 2098, `_eligibleTarget` 2084, `portalDestination` 2140 | 60 |
| Movement | `_movement` 2162, `_landing` 2150, `_jumping` 2273, `_stopMoving` 2308 | 160 |
| Auto attack (fist, spear, bow, wand) | `_actionsAttacks` 2318, `_results` 2361, `_bowSwing` 2483, `_bowArrow` 2506, `_arrow` 460, `playerReach` 2057, `_awt` 2066 | 270 |
| Damage application | `createDmgPixel` 246 (crit, super crit, mastery roll, boss multiplier, accuracy miss, status marks), `isNonDamageable` 229 | 70 |
| Skill pipeline | `_autoCast` 2753, `castAttackofID1` 1996, `castAttack` 2026, `_attackQueueLoop` 2536, `_every005` / `_every1sec` | 230 |
| Skill effects | `SKILL_HANDLERS` (58 skills, 1367), bullets and animations `_meleeArea` … `_wt14` (339-1300) | 1,300 |
| Summons and pets | `_petInit` 1301, `_summonPet` 1348, `_summonWithExtras` 1351 | 70 |
| Forms and teleports | `_applyFormOverrides` 203, `_wraithForm` 1626, `_formReload` 1640, `_teleport` 1658, `_disciple` 1680 | 160 |
| Spawn skills | Graveyard 1807, Reindeer 1829, Wormhole 1845, Arcane Crystal 1907 | 190 |
| Buffs | `addBuff` 182, `_mirrorBuff` 191 | 20 |
| Survival | `_takeDamage` 2887, `_die` 2910, invulnerability timer in `_actionsAttacks`; world `contactDamage` / `monsterDamage` / `dealDmgToPlayer` (`combat-sim-world.js:816-860`) | 110 |
| Food and potions | `consumeFoods` 2640, `_consumeFoodNew` 2655, `_consumeFoodMove` 2676, `_manaPotions` 2696, `_foodRecomputeLive` 2588 | 170 |
| Manual casts (not auto mode) | `normalizeManualCasts` 2960 … `_manualCasts` 3066 | 220 |

The world side matters too. With the stand-in player, monster positions never mattered, so the
Rust encounter never ported monster movement (`combat-sim-world.js:596`, `_movement`, every 2 s,
which draws `randomInt` per monster) or `_nodeBoundary` (587). The auto player chases monsters by
position, so both become part of the port.

## 2. Inputs

The player reads 66 distinct keys from `sim.inputs.values`, plus monster and scene data. Most are
resolved in `combat-sim-inputs.js` `_playerInputs` (851-1003) and `_foodInputs`. Status:

- **decoded**: already computed in this repository;
- **private**: computed inside `idleon-stats-engine` as part of the damage layers, but not exported;
- **not ported**: computed upstream by a system this repository does not carry;
- **static**: game data, with or without an extractor here.

| Input | Upstream source | Status here |
| --- | --- | --- |
| `damageMax`, `damageMin`, `damageMastery` | `damage` descriptor (`js/stats/defs/damage.js`) | **decoded**: `idleon_stats_engine::damage_layers` (`max`, `min`, `mastery`; matches upstream since #291) |
| `playerMaxHp` | `max-hp` descriptor | **private**: `Ctx::max_hp` (`crates/stats-engine/src/extra.rs:767`) |
| `playerMaxMp` | `max-mp` descriptor | **private**: `Ctx::max_mp` (`extra.rs:800`) |
| `playerSpeedBonus`, `playerSpeed` | `movement-speed` descriptor; `16 × min(2.19, bonus)` | **private**: `Ctx::movement_speed` (`extra.rs:815`) |
| `critChance` | `js/stats/defs/crit-chance.js` | **not ported** (registered only in `crates/stats/src/breakdowns/mod.rs`) |
| `critDamage` | `js/stats/defs/crit-damage.js` | **not ported** (registered only) |
| `accuracy`, `hitChancePct` | `js/stats/defs/accuracy.js`; `computeHitChance` (`combat-outcomes.js:94`) | **not ported** |
| `playerDefence` | `js/stats/defs/defence.js` (`TotalStats("Defence")`) | **not ported**: `crates/combat/src/loadout.rs` sums gear Defence only |
| `attackSpeedPct` | `actionWaitTimeAfk` (`combat-outcomes.js:184`): etc bonus 56, meals, chip, bubble, box | **not ported** |
| `weaponType`, `weaponSpeed`, `weaponReach` | `ITEMS[weapon].Type/Speed/Reach` | **static, partial**: `crates/combat/data/combat.json` items keep `type` and `stats`, not `Speed` or `Reach` |
| `level`, `charClass`, `skillLevels`, `attackLoadout`, `savedCooldowns` | `Lv0_X`, `CharacterClass_X`, `SL_X`, `AttackLoadout_X`, `AtkCD_X` | **decoded** as raw save keys (`crates/stats/src/snapshot.rs` reads `AttackLoadout`); trivial to expose |
| `talentN` (98 talent ids, both columns) | `GetTalentNumber` via `talent.js` | **private**: `Ctx::talent` / `talent_tab` (`crates/stats-engine/src/systems.rs`); the loadout editor also has the talent table |
| `initialBuffs` | `BuffsActive_X` | **decoded**: `crates/killroy/src/buffs.rs` |
| `atkCooldownDec` | `1 + (TN(1,289) + BoxRewards.cooldown)/100` | **private** parts (talent, box reward); composition not ported |
| `pctMpPerSec`, `foodManaRegen`, `sigil5`, `card51` | `PctMPperSec` (src 65953) | **private** parts (food, sigil, card); composition not ported |
| `invulnSeconds`, `autoRespawnSec`, `selfHeal` | `2.65 + TN(1,484)`; `autoRespawnTime` (`combat-outcomes.js:363`) | **not ported** (small) |
| `bossMulti` | WIS, etc bonus 5, star sign, stamp, vault 22 | **private** parts; composition not ported |
| `vaultUpg7/8/9`, `arcane13/20/26`, `compassRange` | `vaultUpgBonus`, `arcaneUpgBonus`, `computeCompassBonus` | **decoded** (`crates/account/src/bonuses.rs`, compass in `extra.rs`) |
| `formStats`, `grimoire21` | `wraithStats` / `tempestStats` / `arcanistStats` (`combat-forms.js`), fishing evaluator for Wraith | **not ported** |
| `riChance`, `riMobs` | `orbletBonus`, `jellyRewardBonus`, `rogBonusQTY`, `outpostIsPurified` | **not ported** as a composition; parts live in `crates/royal-guardian` and `crates/jelly` |
| `talentEnh165/362/497`, `alchemyActive` | `getbonus2`, bubble `AlchemyACTIVE` | **not ported** (small) |
| `foodSlots`, `foodStorage`, `healthFoodEffect`, `foodNotConsume`, `cFoodCap`, `labFoodRefill` | `_foodInputs`, `healthFoodHealingPerHour`, `computeMaxCapacity('cFood')` | **partial**: food amounts in `extra.rs`, carry capacity in `crates/stats/src/breakdowns/capacity.rs`; the rest not ported |
| `fishingSpeed` | never set by `combat-sim-inputs.js` (reads as 0) | upstream gap; nothing to port |
| `ola25`, `initialHp`, `initialMp`, `gi81`, `playerWidth/Height` | runtime-only defaults (`unavailable(...)`) | constants |
| Scene graph: `neighbors`, `neighborType`, `connectorX`, `portals`, `idlePlatforms` | `SCENE_GEOMETRY` | **static, partial**: `script/combat-encounter-oracle.mjs` writes only `x`, `y`, `length` per node |
| Monster combat fields: `Damages`, `Defence`, `MoveSpeed`, AFK type | `MONSTERS` | **static, partial**: `crates/combat/data/encounters.json` keeps `hp`, `respawn`, `height`, `monType` |
| Skill table: cooldown, cast time, mana cost, range, AFK type | `ATTACKS` + `TalentIconNames` | **static, not extracted** |
| Bullet hitboxes and player animation timings | `BULLET_ANIMS`, `SPRITE_DEFAULTS`, `CODE_IMAGES`, `PLAYER_ANIMS` | **static, partial**: replay extraction keeps `MOB_SPRITES` only |
| Lists: `NonAFKmonsters`, `MapType`, `NonDMGLinemonsterTypes`, `SceneTransitions`, `MapAFKtarget` | `customlists.js` | **static, partial**: `MapAFKtarget` via `idleon_stats_data::afk_target` |

**Count: 13 input groups are not ported.** They are crit chance, crit damage, accuracy/hit chance,
defence, attack speed, invulnerability/auto-respawn, forms, Royal Intervention, talent
enhancements, food, and the compositions for cooldown reduction, MP regen and the boss multiplier.
`fishingSpeed` is an upstream gap, not a port. Another **4 are private** to the stats engine and
need a public accessor: max HP, max MP, movement speed and talent numbers. Five static tables need
their extractor extended: the scene graph, monster combat fields, `ATTACKS`, bullet hitboxes and
weapon speed/reach. None of this blocks the engine port itself; see the input seam in §4.

The **component-term trace** the combat page lists as neutral is a separate piece:
`combat-sim-terms.js` builds the inputs inside `withTermScope` (`js/stats/term-overrides.js`) to
list every descriptor term behind each input. It depends on term-level tracking in every descriptor
above, not on the player. It belongs with the stat-engine work, not with this port.

## 3. Determinism and testability

**Deterministic under seed: yes.** Two conditions apply when comparing a Rust port step by step:
every random draw in a step must come in the same order (or the oracle must split the stream, see
below), and crystals, giants and plunder spawns must stay off, as the encounter oracle already
keeps them.

Evidence:

- One seeded stream. `createRng` (`combat-sim-core.js:127`) is mulberry32 with a call counter;
  every draw in the player module goes through `sim.rng` (96 call sites in `combat-sim-player.js`).
  `rg 'Math\.random|Date\.now|performance\.now'` over `combat-sim-*.js`, `combat-forms.js` and
  `combat-outcomes.js` finds only `combat-sim-setup-search.js:239`, a seed fallback in the setup
  search used when no `seedBase` is given; it is not part of a run. The UI draws its seeds with
  `crypto.getRandomValues` (`js/ui/combat-farming.js:76`), also outside the run.
- Measured: map 320, character 1 (Death Bringer, spear), one simulated hour, seed 1, run twice:
  27,063 kills and 953,774 draws both times. Seed 2 gives 26,448 kills and 927,210 draws, so
  seed-to-seed spread is about 2%.
- No browser dependency. The module imports data tables and `js/state.js` only. It runs headless
  in Node, the same way `script/combat-encounter-oracle.mjs` drives the stand-in. **None of the
  STOP conditions apply.**
- Same tick. `STEP_MS = 10` (`combat-sim-core.js:123`) with the 1000/60 ms render-frame batch
  (`:443`, `:568`). The Rust encounter replays the same values (`crates/combat/src/encounter.rs:49-51`,
  `:664`).

### The shared-stream problem, and the fix

Upstream gives the player, the world, the rewards, the specials and the progress modules one
shared stream. In one 10-minute run (map 320, character 1), the draws split as follows:

| Module | Draws |
| --- | ---: |
| `combat-sim-player.js` | 75,338 |
| `combat-sim-rewards.js` | 60,962 |
| `combat-sim-world.js` | 12,947 |
| `combat-sim-progress.js` | 4,286 (one per kill) |

The Rust reward port draws only for the card and obol rows (`drops.rs`). A port that matched the
player step by step would therefore first need the whole reward engine (coins, equipment, EXP,
items, crystals) in exact draw order. That would couple this port to roughly 1,600 more lines.

The oracle can split the stream instead. It replaces `sim.rng` with a router that sends draws made
from `combat-sim-rewards.js` and `combat-sim-special.js` to a second `createRng` stream, by reading
the caller's file from `new Error().stack`. This was tested: the routed run is deterministic
(4,286 kills, 92,607 / 60,962 draws on the two streams, identical across two runs). It costs about
8× runtime (2.9 s instead of 0.37 s for 10 minutes), which is fine for oracle-sized cases. The
Rust port then keeps two streams by design: player and world on one, rewards and progress on the
other. It never has to reproduce draw-for-draw interleaving with an unported reward path. Each
stream still has upstream's distributions. Upstream already notes that a seed cannot reproduce a
live run (`combat-sim-core.js:34-35`), so a different interleaving loses nothing a user could
check.

### What the fixture records

The pattern of `script/combat-encounter-oracle.mjs` carries over: load `fixtures/data.json`, build
`createCombatSim` with a fixed seed and `inputOverrides`, run, record. For a step-level comparison
the new oracle (`script/combat-auto-oracle.mjs`) should record:

- the resolved `sim.inputs.values` subset the player reads, so the Rust test can **inject** them
  and test the AI independently of the stat ports (§4);
- a trace sampled every 50 ms (the player's timer period): player `x`, `y`, `node`, `pmm[0]`,
  `animation`, `target` uid, `hp`, `mp`, and both streams' call counters;
- every `damage` event (time, monster uid, amount, crit flag), every `monsterKilled`, `cast`,
  `buff`, `playerHit` and `playerDied` event;
- totals: kills, damage, steps, frames, and per-type kills.

A short case (60 s) on a few maps per weapon type keeps the fixture small. The existing replay
codec can carry positions for visual checks, but the test should compare the trace directly: a
diverging draw counter points at the first wrong step. `options.telemetry`
(`combat-sim-telemetry.js`) can explain target-selection decisions when debugging a mismatch.

### Runtime

One simulated hour upstream (setup about 65 ms):

| Character | Weapon | Map | Kills/h | Runtime |
| --- | --- | --- | ---: | ---: |
| 0 Arcane Cultist | wand | 320 | 30,456 | 2.54 s |
| 1 Death Bringer | spear | 320 | 27,063 | 1.96 s |
| 2 Siege Breaker | bow | 320 | 3,465 | 1.02 s |
| 7 Voidwalker | fists | 320 | 3,117 | 0.63 s |
| 0 Arcane Cultist | wand | 1 | 28,185 | 2.44 s |
| 1 Death Bringer | spear | 306 (Clam Work) | 33,187 | 2.06 s |

Runtime grows with kill count, because rewards dominate busy runs. A Rust port should be at least
as fast. The upstream farming panels default to 30 minutes × 3 seeds (`js/ui/combat-farming.js:21`),
about 3-4 s per row, and Clamworks runs one row per upgrade. A section can afford to measure once
per explicit "Measure" action on the existing job runner (`idleon_common::Control` / `Progress`),
off the UI thread and cancellable, as `docs/plan.md` requires. It cannot re-measure on every edit.

Survival also runs headless. With `inputOverrides` `{ playerMaxHp: 2000, playerDefence: 0 }`,
character 7 on map 320 takes contact hits, dies twice in 10 minutes and auto-respawns. The
fixture's characters are all level 1,280+ and never take damage, so a survival oracle must use
such overrides or a purpose-built save.

## 4. Product output and slices

### `MeasuredRate`

The type would live in `crates/combat/src/measure.rs`. A consumer crate never depends on
`idleon-combat`; the app wires the measurement into a section's input, as the Killroy practice
handoff does today (`crates/app/src/sections/combat_sim/actions.rs`, `idleon_killroy::trial_inputs`).

```rust
/// What a measured run hands a consumer section.
pub struct MeasuredRate {
    pub kills_per_hour: f64,
    pub damage_per_second: f64,
    /// Kill credit per kill and the overkill tier the run reached, when the map counts them.
    pub multikill: Option<Multikill>,
    pub survival: Survival,
    /// Standard error of `kills_per_hour` across seeds; `None` for one seed.
    pub spread: Option<f64>,
    pub provenance: Provenance,
}

pub struct Survival {
    pub deaths: u32,
    pub seconds_dead: f64,
    pub lowest_hp_share: f64,
}

pub struct Provenance {
    pub character: usize,
    pub map: usize,
    /// `Imported save` or the preset's name.
    pub loadout: String,
    pub seeds: Vec<u64>,
    pub duration_ms: u64,
    /// `StandIn { interval_ms }` or `Auto { slice }`.
    pub player: PlayerKind,
    /// The run's `unsupported()` lines plus every input the stat engine could not justify
    /// (the way `Layers::missing` does), so the caption can say why the number is a bound.
    pub unmodelled: Vec<String>,
    /// True while a skipped mechanic can only add kills (skills off, for example).
    pub lower_bound: bool,
}
```

Each consumer offers it as a **default the user can override**, shown with its provenance line
("measured: Death Bringer, Clam Work, 30 min × 3 seeds, skills not modelled — lower bound"). A
measurement never silently replaces a typed value. The section keeps its typed field, adds "Use
measured", and marks the field as edited once the user changes it.

| Consumer | Field today | What it takes |
| --- | --- | --- |
| Killroy | `TrialInputs.kills_per_second` (`crates/killroy/src/trial.rs:42-43`) | `kills_per_hour / 3600` on the trial's map; already wired for the stand-in |
| Clamworks | kills per hour, drop rate, multikill (`crates/app/src/sections/clamworks/state.rs`) | `kills_per_hour` and `multikill` from a map 306 run. The drop rate is a stat, not a measurement: `crates/drop-rate` already computes it |
| Statue | statues per hour (manual) | `kills_per_hour` × the statue drop odds on the chosen map |
| BoneJoePickle | DPS (`crates/pickle/src/lib.rs:103`) | single-target DPS against one miniboss. A farm-map run's `damage_per_second` sums rolled damage, overkill included, so it overstates single-target DPS. Upstream's own Pickle page keeps the typed DPS (`pickle-calc.html:55`) |

Clam Work (map 306) is excluded from the Rust encounter today: "Clam Work spawns a monster count
derived from the save" (`script/combat-encounter-oracle.mjs`, `EXCLUDED`). Upstream runs it, and
`combat-sim-special.js` owns its pearl and HP rewrites. Clamworks needs that support as well as the
player.

### The input seam

The player reads an `AutoInputs` struct (the ~66 values in §2), never the stat engine directly. The
product builds it from `damage_layers` plus the descriptors; the oracle test builds it from the
fixture's recorded `sim.inputs.values`. This lets slices A-D be ported and pinned while crit,
accuracy, defence and attack speed are still missing. It also means a missing descriptor shows up
as a provenance line, never as a silent wrong number.

### Slices

Smallest first. Each slice ships on its own with its own oracle. Sizes are Rust lines including
tests, estimated from upstream line counts at roughly one Rust line per upstream line. That is
this repository's observed ratio: the 810-line `encounter.rs` covers most of the 801-line core
plus the world's spawn, respawn and kill handlers.

| Slice | Upstream functions | Size | Oracle compares | What a consumer gets |
| --- | --- | --- | --- | --- |
| **0. Data** | `SCENE_GEOMETRY` graph fields, `MONSTERS` combat fields, `ATTACKS` + `TalentIconNames`, `ITEMS` Speed/Reach, `BULLET_ANIMS` / `PLAYER_ANIMS` | **S** (~300, extractor + serde) | extracted JSON equals upstream tables | nothing visible |
| **A. Weapon auto-attack** | `_findNewNode`, `pathfinder`, `_movement`, `_jumping`, `_stopMoving`, `_actionsAttacks`, `_results`, bow arrow, `createDmgPixel`, `rollDamage`, `actionWaitTime`, `playerReach`; world `_movement` + `_nodeBoundary`; two-stream RNG | **L** (~1,300) | 50 ms trace + damage events, 60 s × 4 weapon types × 3 maps, `ola25 = 0` (auto-cast off), injected inputs | a kill rate with **skills off**, which is a lower bound |
| **B. Survival** | `_takeDamage`, world `contactDamage` / `monsterDamage` / `dealDmgToPlayer`, invulnerability, `_die`, auto-respawn, MP regen, health food and potions | **M** (~500) | same trace with `playerMaxHp` / `playerDefence` overrides that force hits and deaths | deaths and downtime; kill rate on maps where the character dies |
| **C. Skill pipeline + one class family** | `_autoCast`, `castAttack`, `_attackQueueLoop`, cooldowns, MP cost, buffs, and that family's `SKILL_HANDLERS` and bullets | **M-L** per family (~600 pipeline + ~300-500 per family) | trace plus `cast` / `buff` / `bullet` events for one character of that family | full kill rate for that family; others stay lower bounds |
| **D. All classes** | the remaining handlers, summons, spawn skills (Graveyard, Reindeer, Wormhole, Arcane Crystal), forms (`combat-forms.js`), teleports | **L** (~2,000) | the same per family | full kill rate for every class |
| **E. Consumers** | `MeasuredRate`, multi-seed runs, the four sections' "Use measured" defaults; Clam Work map support | **M** | section tests on recorded measurements | the four typed inputs get defaults |

**Is slice A good enough?** Measured on the fixture, 30-minute runs, kills with auto-cast on vs
off (`ola25 = 0`):

| Character | Weapon | Map 320 on / off | Map 306 on / off |
| --- | --- | ---: | ---: |
| Arcane Cultist | wand | 15,096 / 1,647 | 16,177 / 1,304 |
| Death Bringer | spear | 13,360 / 1,662 | 16,609 / 1,304 |
| Siege Breaker | bow | 1,779 / 1,408 | 1,115 / 1,116 |
| Voidwalker | fists | 1,568 / 1,580 | 1,117 / 1,117 |

For skill-driven classes, slice A underestimates by about 10×. For others it is close to the full
rate. So slice A alone is a sound oracle foundation, but as a consumer default it is honest only
when labelled as a lower bound, and it is misleading for wand and spear endgame characters. Slice C
should come right after A, starting with the family the maintainer's consumers farm with most. The
skills cast in 10 minutes on map 320 show what each family needs:

- Arcane Cultist: 529, 481, 525, 526
- Death Bringer: 105, 106, 90, 136
- Wind Walker: 270, 301, 362, 271
- Elemental Sorcerer: 496, 453, 497, 465

### What the stand-in keeps doing

The stand-in stays:

- the encounter loop's regression oracle (`fixtures/combat-encounter.json`);
- the player for maps or characters the auto player cannot run yet (dungeons, excluded maps, a
  class whose skills are unported when an exact rate is wanted);
- the "what if I killed one every N ms" what-if, which is still a useful knob for Killroy and
  Clamworks;
- the fallback a consumer offers when no measurement exists, labelled as typed.

## 5. Recommendation

**Go, after** the input prerequisites, with the engine work allowed to start at once behind the
input seam.

1. Start with **slice 0** (S) and **slice A** (L): the first oracle is `script/combat-auto-oracle.mjs`.
   It runs `autoPlayer()` with `ola25 = 0`, crystals, giants and plunder off, rewards, specials and
   progress routed to a second stream, recorded inputs, a 50 ms trace, and 60 s cases for fist,
   spear, bow and wand characters on maps 1, 60 and 320. Expect two to three weeks for 0 + A.
2. In parallel, as stat-engine work (plan 021's lane), port the descriptors the product needs
   before a measured rate may be shown: `crit-chance`, `crit-damage`, `accuracy` (+
   `computeHitChance`), `defence`, `actionWaitTimeAfk`'s attack-speed pool, and public accessors
   for `max_hp`, `max_mp` and `movement_speed`.
3. Then B, then C for the first class family, then E for Killroy, Clamworks and Statue. Upstream
   measures all three with the auto player: `js/workers/combat-sim-client.js:48` defaults every
   batch job to `'auto'`. D follows as demand allows; the full port is multi-week, on the order
   of 5,000 Rust lines.

### Three biggest risks

| Risk | How the slices contain it |
| --- | --- |
| **Fidelity drift through the shared RNG stream.** The player shares one stream with rewards this repository has only partly ported, so one misplaced draw desynchronises everything after it. | The oracle splits the stream (measured: deterministic, 8× slower). Every slice compares a 50 ms trace with draw counters, so the first diverging step is visible. Crystal, giant and plunder spawns stay off, as in the encounter oracle. |
| **Skill scope.** 58 handlers and about 1,300 lines of per-animation bullet code, with kill rates that depend on skills by an order of magnitude. | Skills come in per family (C), each with its own fixture. Until a family lands, its rate carries `lower_bound = true` and an `unmodelled` line naming the skills. Upstream's own `unmodelled` keys pass through. |
| **Wrong inputs presented as a measurement.** Four descriptors are missing, plus forms and food. | The `AutoInputs` seam keeps the AI testable on recorded inputs. The product build lists every input it could not justify in `Provenance::unmodelled`, as `Layers::missing` does, and a consumer only offers the value as an overridable default. |

### Open questions for the maintainer

1. Is a skills-off kill rate worth showing, labelled as a lower bound? It matches the full rate for
   the fixture's fist and bow characters but is about 10× low for its wand and spear characters.
2. Which class family should slice C start with? That should follow which characters people farm
   Clam Work and statues with.
3. Is the two-stream design acceptable, given that a seed then no longer matches upstream's
   single-stream run draw for draw, only in distribution?
4. What is the default measurement budget (duration × seeds) per consumer? Upstream uses
   30 min × 3 seeds, about 3-4 s per row here.
5. Should Pickle consume a measurement at all? A farm-map run measures overkill-inflated damage,
   and upstream's Pickle keeps a typed DPS.
6. Is Clam Work map support (the saved mob count, `combat-sim-special.js` pearl and HP rewrites)
   part of this port or a separate CS item?

### Captions that misstate the blocker

These user-visible captions say the combat simulator is not built. It is built; the missing piece
is the auto-mode player. They can be reworded in a small UI change, independent of this port:

- `crates/app/src/sections/killroy/comparison.rs:495`: "The combat simulator is not built yet, so
  the kill rate is the trials' stand-in input; …"
- `crates/app/src/sections/clamworks/claim.rs:229`: "Kills per hour and the drop rate are entries
  until the combat simulator derives them; …". The drop rate half is wrong for a second reason:
  it is a stat that `crates/drop-rate` already computes, not a simulator output.

The same claim also appears in docs and source comments, which a caption change should reword too:

- `docs/sections/killroy.md:17` and `docs/roadmap.json:1511` ("The combat simulator is still M2");
- `docs/sections/statue.md:17` and `docs/roadmap.json:1238` ("needs the combat simulator (CS-03)",
  which has shipped);
- `docs/plan.md:401` and `:421`;
- comments at `crates/app/src/sections/killroy/mod.rs:70`,
  `crates/app/src/sections/clamworks/{mod.rs:10, claim.rs:9, state.rs:15, actions.rs:135, actions.rs:187}`
  and `crates/killroy/src/trial.rs:42`.

The Statue and Pickle sections have no app caption that matches.
