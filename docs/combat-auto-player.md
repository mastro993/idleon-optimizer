# Combat auto-mode player: sizing and design

A research spike (plan 023, issue #278). It sizes the combat **auto-mode player**: the AI that
moves, auto-attacks, casts skills and fires projectiles. That player is what would turn a save into
the kill rates Clamworks, Killroy, the Statue calculator and BoneJoePickle still ask the user to type.
In `docs/roadmap.json` this player is the blocker `combat:auto-player`. It gates four tracked gaps:
combat DPS and survival, Statue's "Measure statue farming", Killroy's kills per second, and
Clamworks' kills per hour, drop rate and multikill. The missing stat formulas listed in §2 sit
under `stats:descriptors`. This document changes no code. It records what the player would take,
how it can be tested, the smallest useful slice, and a recommendation.

## 1. Size

### What the player does

The player plugs into the engine core `crates/combat/src/encounter.rs` already runs: the 10 ms step,
timed tasks, the frame batch and the seeded RNG. Its timers fire in this order inside one step:

| Timer | Interval |
| --- | ---: |
| reset node-change count | 10,000 ms |
| target selection | 150 ms |
| movement (path, jump, ladder, drop) | 50 ms |
| auto-attack trigger | 50 ms |
| attack resolution | 50 ms |
| stop moving | 150 ms |
| contact damage | 150 ms |
| attack-queue decrement | 50 ms |
| cooldowns, MP regen, buff tick, respawn countdown | 1,000 ms |
| skill auto-cast | 1,000 ms |
| health food | 150 ms |
| movement food | 1,000 ms |
| all-mobs-dead respawn (buff 167) | 1,000 ms |
| spawn-skill pixel helper | 330 ms |
| attack queue (scene updater, every step) | 10 ms |

After the core integrates position, each step resolves jumping, then death.

### Areas

| Area | What it covers | Approx. lines |
| --- | --- | ---: |
| Helpers | distance, breadth-first pathfinder, weapon wait time, damage roll | 140 |
| Target selection | eligible targets, the next node, portal destinations | 60 |
| Movement | landing, jumping, stopping | 160 |
| Auto attack (fist, spear, bow, wand) | trigger, resolution, bow swing and arrow, reach, wait time | 270 |
| Damage application | crit, super crit, mastery roll, boss multiplier, accuracy miss, status marks, non-damageable monsters | 70 |
| Skill pipeline | auto-cast, cast by id, the attack queue, the 50 ms and 1 s ticks | 230 |
| Skill effects | 58 skill handlers, per-animation bullets and areas | 1,300 |
| Summons and pets | pet init, summons with extras | 70 |
| Forms and teleports | form overrides, Wraith form, form reload, teleport, disciple | 160 |
| Spawn skills | Graveyard, Reindeer, Wormhole, Arcane Crystal | 190 |
| Buffs | add and mirror | 20 |
| Survival | contact and monster damage, invulnerability, death, auto-respawn | 110 |
| Food and potions | health food, movement food, mana potions, live food recompute | 170 |
| Manual casts (not auto mode) | normalising and replaying manual casts | 220 |

The world side matters too. With the stand-in player, monster positions never mattered, so the
Rust encounter has no monster movement (every 2 s, one random draw per monster) and no node
boundaries. The auto player chases monsters by position, so both become part of the work.

## 2. Inputs

The player reads 66 distinct input values, plus monster and scene data. Status:

- **decoded**: already computed in this repository;
- **private**: computed inside `idleon-stats-engine` as part of the damage layers, but not exported;
- **not modelled**: needs a system this repository does not carry yet;
- **static**: game data, complete or partial here.

| Input | Status here |
| --- | --- |
| Damage max, min and mastery | **decoded**: `idleon_stats_engine::damage_layers` (`max`, `min`, `mastery`) |
| Max HP | **private**: `Ctx::max_hp` (`crates/stats-engine/src/extra.rs`) |
| Max MP | **private**: `Ctx::max_mp` (`extra.rs`) |
| Movement speed bonus and speed (`16 × min(2.19, bonus)`) | **private**: `Ctx::movement_speed` (`extra.rs`) |
| Crit chance | **not modelled** (registered only in `crates/stats/src/breakdowns/mod.rs`) |
| Crit damage | **not modelled** (registered only) |
| Accuracy and hit chance | **not modelled** |
| Defence | **not modelled**: `crates/combat/src/loadout.rs` sums gear Defence only |
| Attack speed (etc bonus 56, meals, chip, bubble, box) | **not modelled** |
| Weapon type, speed and reach | **static, partial**: `crates/combat/data/combat.json` items keep `type` and `stats`, not speed or reach |
| Level, class, skill levels, attack loadout, saved cooldowns | **decoded** as raw save keys (`Lv0_X`, `CharacterClass_X`, `SL_X`, `AttackLoadout_X`, `AtkCD_X`; `crates/stats/src/snapshot.rs` reads `AttackLoadout`); trivial to expose |
| Talent numbers (98 talent ids, both columns) | **private**: `Ctx::talent` / `talent_tab` (`crates/stats-engine/src/systems.rs`); the loadout editor also has the talent table |
| Initial buffs (`BuffsActive_X`) | **decoded**: `crates/killroy/src/buffs.rs` |
| Cooldown reduction (talent 289, box reward) | **private** parts; composition not modelled |
| MP per second (food, sigil 5, card 51) | **private** parts; composition not modelled |
| Invulnerability, auto-respawn, self heal (`2.65 + talent 484`) | **not modelled** (small) |
| Boss multiplier (WIS, etc bonus 5, star sign, stamp, vault 22) | **private** parts; composition not modelled |
| Vault upgrades 7/8/9, arcane 13/20/26, compass range | **decoded** (`crates/account/src/bonuses.rs`, compass in `extra.rs`) |
| Form stats (Wraith, Tempest, Arcanist), grimoire 21 | **not modelled** |
| Royal Intervention chance and monsters (orblets, jelly reward, Ring of Glory, purified outposts) | **not modelled** as a composition; parts live in `crates/royal-guardian` and `crates/jelly` |
| Talent enhancements 165/362/497, active alchemy | **not modelled** (small) |
| Food slots, storage, healing, non-consumption, food capacity, lab refill | **partial**: food amounts in `extra.rs`, carry capacity in `crates/stats/src/breakdowns/capacity.rs`; the rest not modelled |
| Runtime defaults (initial HP/MP, player size, auto-cast flag) | constants |
| Scene graph: neighbours, neighbour type, connectors, portals, idle platforms | **static, partial**: the encounter data keeps only `x`, `y`, `length` per node |
| Monster combat fields: damage, defence, move speed, AFK type | **static, partial**: `crates/combat/data/encounters.json` keeps `hp`, `respawn`, `height`, `monType` |
| Skill table: cooldown, cast time, mana cost, range, AFK type | **static, missing** |
| Bullet hitboxes and player animation timings | **static, partial**: the replay data keeps monster sprites only |
| Lists: non-AFK monsters, map types, non-damage-line monster types, scene transitions, AFK targets | **static, partial**: AFK targets via `idleon_stats_data::afk_target` |

**Count: 13 input groups are not modelled.** They are crit chance, crit damage, accuracy/hit chance,
defence, attack speed, invulnerability/auto-respawn, forms, Royal Intervention, talent
enhancements, food, and the compositions for cooldown reduction, MP regen and the boss multiplier.
Another **4 are private** to the stats engine and need a public accessor: max HP, max MP, movement
speed and talent numbers. Five static tables need extending: the scene graph, monster combat
fields, the skill table, bullet hitboxes and weapon speed/reach. None of this blocks the engine work
itself; see the input seam in §4.

A **component-term trace** listing every descriptor term behind each input is a separate piece. It
depends on term-level tracking in every descriptor above, not on the player, and belongs with the
stat-engine work.

## 3. Determinism and testability

The player is deterministic under a seed: every draw goes through the run's seeded stream, and
nothing in a run reads the wall clock. Crystals, giants and plunder spawns stay off in tests, as
the encounter tests already keep them.

### Two streams

The player, the world, the rewards, the specials and the progress all draw random numbers. The
Rust reward model draws only for the card and obol rows (`drops.rs`), so one shared stream would
couple the player to the whole reward engine (coins, equipment, EXP, items, crystals) in exact
draw order. The Rust player therefore keeps two streams by design: player and world on one,
rewards and progress on the other. A seed reproduces a run exactly; a different interleaving of
reward draws loses nothing a user could check, since no seed reproduces a live game.

### What the tests record

For a step-level check a test should record:

- the resolved input values the player reads, so the test can **inject** them and test the AI
  independently of the stat formulas (§4);
- a trace sampled every 50 ms (the player's timer period): player `x`, `y`, node, movement,
  animation, target, HP, MP, and both streams' call counters;
- every damage event (time, monster, amount, crit flag), every kill, cast, buff, player hit and
  player death;
- totals: kills, damage, steps, frames, and per-type kills.

A short case (60 s) on a few maps per weapon type keeps the cases small. The existing replay codec
can carry positions for visual checks, but the test should compare the trace directly: a diverging
draw counter points at the first wrong step.

### Runtime

Runtime grows with kill count, because rewards dominate busy runs. A 30-minute × 3-seed
measurement takes a few seconds per row, and Clamworks runs one row per upgrade. A section can
afford to measure once per explicit "Measure" action on the existing job runner
(`idleon_common::Control` / `Progress`), off the UI thread and cancellable, as `docs/plan.md`
requires. It cannot re-measure on every edit.

The fixture's characters are all level 1,280+ and never take damage, so a survival test must
override max HP and defence or use a purpose-built save.

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
| Killroy | `TrialInputs.kills_per_second` (`crates/killroy/src/trial.rs`) | `kills_per_hour / 3600` on the trial's map; already wired for the stand-in |
| Clamworks | kills per hour, drop rate, multikill (`crates/app/src/sections/clamworks/state.rs`) | `kills_per_hour` and `multikill` from a map 306 run. The drop rate is a stat, not a measurement: `crates/drop-rate` already computes it |
| Statue | statues per hour (manual) | `kills_per_hour` × the statue drop odds on the chosen map |
| BoneJoePickle | DPS (`crates/pickle/src/lib.rs`) | single-target DPS against one miniboss. A farm-map run's `damage_per_second` sums rolled damage, overkill included, so it overstates single-target DPS |

Clam Work (map 306) is excluded from the Rust encounter today, because Clam Work spawns a monster
count derived from the save and rewrites pearls and HP. Clamworks needs that support as well as the
player.

### The input seam

The player reads an `AutoInputs` struct (the ~66 values in §2), never the stat engine directly. The
product builds it from `damage_layers` plus the descriptors; a test builds it from recorded values.
This lets slices A-D be built and pinned while crit, accuracy, defence and attack speed are still
missing. It also means a missing descriptor shows up as a provenance line, never as a silent wrong
number.

### Slices

Smallest first. Each slice ships on its own with its own tests. Sizes are Rust lines including
tests, estimated from the area sizes in §1.

| Slice | Scope | Size | Tests compare | What a consumer gets |
| --- | --- | --- | --- | --- |
| **0. Data** | scene graph fields, monster combat fields, the skill table, weapon speed/reach, bullet and player animation timings | **S** (~300, tables + serde) | the new table rows load and cover every map and skill | nothing visible |
| **A. Weapon auto-attack** | target selection, pathfinder, movement, jumping, stopping, auto-attack trigger and resolution, bow arrow, damage application, damage roll, wait time, reach; world movement and node boundaries; two-stream RNG | **L** (~1,300) | 50 ms trace + damage events, 60 s × 4 weapon types × 3 maps, auto-cast off, injected inputs | a kill rate with **skills off**, which is a lower bound |
| **B. Survival** | contact and monster damage, invulnerability, death, auto-respawn, MP regen, health food and potions | **M** (~500) | the same trace with max HP / defence overrides that force hits and deaths | deaths and downtime; kill rate on maps where the character dies |
| **C. Skill pipeline + one class family** | auto-cast, casting, the attack queue, cooldowns, MP cost, buffs, and that family's skill handlers and bullets | **M-L** per family (~600 pipeline + ~300-500 per family) | trace plus cast / buff / bullet events for one character of that family | full kill rate for that family; others stay lower bounds |
| **D. All classes** | the remaining handlers, summons, spawn skills (Graveyard, Reindeer, Wormhole, Arcane Crystal), forms, teleports | **L** (~2,000) | the same per family | full kill rate for every class |
| **E. Consumers** | `MeasuredRate`, multi-seed runs, the four sections' "Use measured" defaults; Clam Work map support | **M** | section tests on recorded measurements | the four typed inputs get defaults |

**Is slice A good enough?** For skill-driven classes (wand and spear endgame characters), turning
auto-cast off cuts the kill rate by about 10×; for fist and bow characters it barely moves. So
slice A alone is a sound foundation, but as a consumer default it is honest only when labelled as a
lower bound, and it is misleading for wand and spear endgame characters. Slice C should come right
after A, starting with the family the maintainer's consumers farm with most.

### What the stand-in keeps doing

The stand-in stays:

- the encounter loop's regression baseline;
- the player for maps or characters the auto player cannot run yet (dungeons, excluded maps, a
  class whose skills are not modelled when an exact rate is wanted);
- the "what if I killed one every N ms" what-if, which is still a useful knob for Killroy and
  Clamworks;
- the fallback a consumer offers when no measurement exists, labelled as typed.

## 5. Recommendation

**Go, after** the input prerequisites, with the engine work allowed to start at once behind the
input seam.

1. Start with **slice 0** (S) and **slice A** (L): auto-cast off, crystals, giants and plunder off,
   rewards, specials and progress on the second stream, injected inputs, a 50 ms trace, and 60 s
   cases for fist, spear, bow and wand characters on maps 1, 60 and 320. Expect two to three weeks
   for 0 + A.
2. In parallel, as stat-engine work (plan 021's lane), model the descriptors the product needs
   before a measured rate may be shown: crit chance, crit damage, accuracy and hit chance, defence,
   the attack-speed pool, and public accessors for `max_hp`, `max_mp` and `movement_speed`.
3. Then B, then C for the first class family, then E for Killroy, Clamworks and Statue. D follows
   as demand allows; the whole player is multi-week, on the order of 5,000 Rust lines.

### Three biggest risks

| Risk | How the slices contain it |
| --- | --- |
| **Fidelity drift through the RNG stream.** One misplaced draw desynchronises everything after it. | Two streams keep the player independent of the partly modelled rewards. Every slice compares a 50 ms trace with draw counters, so the first diverging step is visible. Crystal, giant and plunder spawns stay off, as in the encounter tests. |
| **Skill scope.** 58 handlers and about 1,300 lines of per-animation bullet code, with kill rates that depend on skills by an order of magnitude. | Skills come in per family (C), each with its own tests. Until a family lands, its rate carries `lower_bound = true` and an `unmodelled` line naming the skills. |
| **Wrong inputs presented as a measurement.** Four descriptors are missing, plus forms and food. | The `AutoInputs` seam keeps the AI testable on recorded inputs. The product build lists every input it could not justify in `Provenance::unmodelled`, as `Layers::missing` does, and a consumer only offers the value as an overridable default. |

### Open questions for the maintainer

1. Is a skills-off kill rate worth showing, labelled as a lower bound? It is close to the full rate
   for fist and bow characters but about 10× low for wand and spear characters.
2. Which class family should slice C start with? That should follow which characters people farm
   Clam Work and statues with.
3. What is the default measurement budget (duration × seeds) per consumer? 30 min × 3 seeds takes a
   few seconds per row.
4. Should Pickle consume a measurement at all? A farm-map run measures overkill-inflated damage.
5. Is Clam Work map support (the saved mob count, the pearl and HP rewrites) part of this work or a
   separate CS item?

### Captions that misstate the blocker

These user-visible captions say the combat simulator is not built. It is built; the missing piece
is the auto-mode player. They can be reworded in a small UI change, independent of this work:

- `crates/app/src/sections/killroy/comparison.rs`: "The combat simulator is not built yet, so the
  kill rate is the trials' stand-in input; …"
- `crates/app/src/sections/clamworks/claim.rs`: "Kills per hour and the drop rate are entries until
  the combat simulator derives them; …". The drop rate half is wrong for a second reason: it is a
  stat that `crates/drop-rate` already computes, not a simulator output.

The same claim also appears in docs and source comments, which a caption change should reword too:

- `docs/sections/killroy.md` and `docs/roadmap.json` ("The combat simulator is still M2");
- `docs/sections/statue.md` and `docs/roadmap.json` ("needs the combat simulator (CS-03)", which has
  shipped);
- comments in `crates/app/src/sections/killroy/mod.rs`,
  `crates/app/src/sections/clamworks/{mod.rs, claim.rs, state.rs, actions.rs}` and
  `crates/killroy/src/trial.rs`.

The Statue and Pickle sections have no app caption that matches.
