//! The encounter run (CS-03): a headless replay of the game's active-combat loop
//! over one map's spawn roster.
//!
//! Ported from `js/stats/systems/common/combat-sim-core.js` (the fixed 10 ms
//! step, the render-frame batch and the timed-task loop) and the parts of
//! `combat-sim-world.js` a run needs to spawn, respawn and kill monsters: the
//! scene spawn loop (`SceneEvents` init), the one-second respawn tick
//! (`_event_1second`), the `whenDrawing` respawn handler and the kill handler
//! (`_respawnHandler`, `_killHandler`). The two players upstream ships without
//! its AI module are ported exactly: [`Player::Idle`] (`idlePlayer`) and
//! [`Player::StandIn`] (`scriptedPlayer`), which kills one live monster per
//! interval for its remaining HP.
//!
//! Since CS-05 the run also rolls the kill's card and obol drops through
//! [`drops`], the extracted half of the reward tables, and samples its kills on
//! the upstream 10-second timeline. Since CS-06 it advances the saved furnace on
//! its 250 ms clock ([`crate::progress`]), banks the kills' Scurge, Coral and
//! Omega credits and reports the furnace and the bars it produced.
//!
//! Not modelled, and listed by [`unsupported`]:
//!
//! - the auto-mode player AI: movement, auto attack, skills, projectiles and
//!   contact damage (`combat-sim-player.js`);
//! - the rest of the reward engine: EXP, coins, equipment, the inventory,
//!   crystals, giants and plunder spawns (`combat-sim-rewards.js`);
//! - the account tuning of monster HP and respawn time (W7 tasks, achievements,
//!   Death Note), held at the base values in `data/encounters.json`;
//! - The Den, the Rift, caverns, Clam Work and dungeon-specific state; Killroy
//!   practice is projected from a run's kill rate by `idleon_killroy`, not
//!   replayed here.
//!
//! The run is deterministic: the modelled paths draw no random numbers, so a
//! map, player and duration fully describe it.
//!
//! `fixtures/combat-encounter.json` and `data/encounters.json` are written by
//! `script/combat-encounter-oracle.mjs` from one upstream checkout; the fixture
//! is the upstream engine's own kills, damage and respawn counters at neutral
//! monster tuning.

use crate::drops::{self, Inputs, Snapshot};
use crate::progress::{self, Config as ProgressConfig, Grants, Summary as ProgressSummary};
use idleon_common::{Control, Progress, Rng};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The game's fixed update step, in milliseconds.
pub const STEP_MS: u64 = 10;
/// Upstream's default render frame length (`1000 / 60`), in milliseconds.
const FRAME_MS: f64 = 1000.0 / 60.0;

const ENCOUNTERS_JSON: &str = include_str!("../data/encounters.json");

/// The extracted encounter tables.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncounterData {
    /// Maps whose map index the run does not support, with the reason.
    excluded: BTreeMap<String, String>,
    maps: Vec<Map>,
}

/// One map's spawn roster.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Map {
    /// The map index the save and the upstream page use.
    pub index: usize,
    /// `MapDispName`, with the underscores the game data uses.
    pub name: String,
    /// The scene script that spawns the roster.
    pub scene: String,
    /// The scene's monsters, in spawn order.
    pub mobs: Vec<Monster>,
}

/// One spawned monster: the def a kill and its respawn read.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Monster {
    /// The `MonsterDefinitionsGET` key, such as `mushG`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The node the scene stages it on.
    pub node: usize,
    /// `MonsterHPTotal`.
    pub hp: f64,
    /// `RespawnTime`; `0` is a resource that respawns after the kill delay.
    pub respawn: f64,
}

/// The tables the encounter run reads, written by
/// `script/combat-encounter-oracle.mjs`.
fn data() -> &'static EncounterData {
    static DATA: OnceLock<EncounterData> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(ENCOUNTERS_JSON).expect("encounters.json is valid"))
}

/// Every map the run supports, by index.
pub fn maps() -> &'static [Map] {
    &data().maps
}

/// The map with `index`, when the run supports it.
pub fn map(index: usize) -> Option<&'static Map> {
    maps().iter().find(|map| map.index == index)
}

/// Why a map is not part of the run, when it is one the extractor skipped.
pub fn excluded(index: usize) -> Option<&'static str> {
    data().excluded.get(&index.to_string()).map(String::as_str)
}

/// The map's readable name, with the game data's underscores as spaces.
pub fn map_name(map: &Map) -> String {
    map.name.replace('_', " ")
}

/// What drives the run. Both are upstream players the simulator ships without
/// its AI module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    /// `scriptedPlayer`: one live monster per interval, for its remaining HP.
    StandIn { kill_every_ms: u64 },
    /// `idlePlayer`: never attacks.
    Idle,
}

impl Player {
    pub fn id(self) -> &'static str {
        match self {
            Player::StandIn { .. } => "standIn",
            Player::Idle => "idle",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Player::StandIn { .. } => "Fixed kill interval (stand-in)",
            Player::Idle => "Idle (no attacks)",
        }
    }
}

/// What one run simulates.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub map: usize,
    pub player: Player,
    /// The simulated length; the game steps in 10 ms units.
    pub duration_ms: u64,
    /// The stream seed every roll draws from.
    pub seed: u64,
    /// The saved cards, obol pages and fragments the drops start from.
    pub drops: Snapshot,
    /// The stat multipliers the drop rolls read.
    pub drop_inputs: Inputs,
    /// The saved furnace and the account progress a run advances.
    pub progress: ProgressConfig,
    /// How a kill credits the Coral Kid, the Scurge and Omega Arcade.
    pub grants: Grants,
}

impl Options {
    /// A run with no drop state and the neutral drop multipliers, which is all
    /// a comparison needs: it reads kills, damage and respawn times.
    pub fn basic(map: usize, player: Player, duration_ms: u64) -> Self {
        Self {
            map,
            player,
            duration_ms,
            seed: 1,
            drops: Snapshot::default(),
            drop_inputs: Inputs::default(),
            progress: ProgressConfig::default(),
            grants: Grants::default(),
        }
    }

    /// The run in 10 ms steps, as upstream's `_run` counts them.
    pub fn steps(&self) -> u64 {
        (self.duration_ms + STEP_MS / 2) / STEP_MS
    }
}

/// The upstream timeline's sampling period: `timelineEveryMs`.
pub const TIMELINE_MS: u64 = 10_000;

/// One kills-over-time sample, taken every ten simulated seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub t_ms: u64,
    pub kills: u64,
}

/// What a finished run reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub map: usize,
    pub scene: String,
    pub player: &'static str,
    /// The duration the run was asked for, in milliseconds.
    pub duration_ms: u64,
    /// The simulated time, in milliseconds; equal to `duration_ms` for a run
    /// that reached its end.
    pub simulated_ms: u64,
    /// Fixed steps taken.
    pub steps: u64,
    /// Render frames drawn.
    pub frames: u64,
    /// Monsters killed.
    pub kills: u64,
    /// Kills per monster type, in spawn-order key order.
    pub kills_by_type: BTreeMap<String, u64>,
    /// Damage the player dealt: the stand-in hits for the monster's remaining
    /// HP plus one, which is what the upstream `damage` events sum.
    pub damage: f64,
    /// Stand-in kill ticks that found no live monster.
    pub idle_ticks: u64,
    /// Each roster monster's respawn counter at the end: `0` or less is ready
    /// to respawn, a positive value is the seconds left.
    pub respawn: Vec<f64>,
    /// The seed the run rolled with.
    pub seed: u64,
    /// The cumulative kills at every ten-second mark, plus the run's end.
    pub timeline: Vec<Sample>,
    /// The cards and obol fragments the kills dropped.
    pub drops: drops::Summary,
    /// The furnace and the combat-driven progress the run banked.
    pub progress: ProgressSummary,
}

impl Outcome {
    /// Kills per hour of simulated time.
    pub fn kills_per_hour(&self) -> f64 {
        if self.simulated_ms == 0 {
            return 0.0;
        }
        self.kills as f64 * 3_600_000.0 / self.simulated_ms as f64
    }

    /// Damage dealt per second of simulated time.
    pub fn damage_per_second(&self) -> f64 {
        if self.simulated_ms == 0 {
            return 0.0;
        }
        self.damage * 1_000.0 / self.simulated_ms as f64
    }

    /// The mean time between kills, in milliseconds.
    pub fn kill_interval_ms(&self) -> Option<f64> {
        (self.kills > 0).then(|| self.simulated_ms as f64 / self.kills as f64)
    }

    /// The stand-in kill ticks the respawn timers absorbed.
    pub fn kill_ticks(&self) -> u64 {
        self.kills + self.idle_ticks
    }
}

/// Why this build's run leaves part of the upstream simulator out. The UI lists
/// it beside the options and keeps it in the exported result.
pub fn unsupported() -> &'static [&'static str] {
    &[
        "Auto-mode player AI (movement, auto attack, skills, projectiles), so a run cannot yet \
         measure character DPS or survival.",
        "The rest of the reward engine: EXP, coins, equipment and the inventory. The card and \
         obol rows are modelled, but an obol that needs a bag slot (a full page without the \
         auto-trash) is not collected.",
        "Crystals, giants and Plunder spawns, which the reward engine creates on a kill.",
        "Account monster tuning (W7 tasks, achievements, Death Note): the run uses the base \
         monster HP and respawn time.",
        "The per-kill forge, pet and alchemy time (talents 280, 362 and 490) and the shiny-pet \
         speed's star-sign term, which need the talent and star-sign formulas: the furnace still \
         advances with the run's own clock.",
        "Killroy, the Rift, caverns other than the Den, Clam Work and dungeons, which need saved \
         state or runtime-only enemy data. The Den runs as its own projection on the results tab.",
        "Replays and the combat assets, which arrive with CS-07.",
    ]
}

/// One roster entry's live state.
struct Live {
    kind: String,
    hp_tot: f64,
    hp: f64,
    respawn: f64,
    /// `willDie`: set by a hit that dropped HP to zero, cleared by respawning.
    will_die: bool,
    /// The damage the killing hit dealt, booked with the kill.
    hit: f64,
    alive: bool,
    counter: f64,
}

impl Live {
    fn new(monster: &Monster) -> Self {
        Self {
            kind: monster.kind.clone(),
            hp_tot: monster.hp,
            hp: monster.hp,
            respawn: monster.respawn,
            will_die: false,
            hit: 0.0,
            alive: true,
            counter: 0.0,
        }
    }
}

/// A repeating timer, in the order the game registers it.
struct Timer {
    remaining: i64,
    interval: i64,
    kind: TimerKind,
}

enum TimerKind {
    /// The stand-in's kill tick (`scriptedPlayer`).
    Kill,
    /// `_forgeTick`: the saved furnace advances every 250 ms.
    Forge,
    /// `_event_1second`: every live respawn counter drops by one.
    Respawn,
}

/// Run `options`, reporting progress and stopping when the reporter says so.
///
/// Returns `None` when the run was stopped. The map must be one [`maps`]
/// returns; an unknown map reports an error instead of panicking.
pub fn run(
    options: Options,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<Outcome>, String> {
    let map = map(options.map).ok_or_else(|| match excluded(options.map) {
        Some(reason) => format!("Map {} is not supported: {reason}", options.map),
        None => format!("Map {} has no extracted scene roster.", options.map),
    })?;
    let steps = options.steps();
    let mut sim = Sim::new(map, &options);
    let report_every = (steps / 100).max(1);
    for step in 0..steps {
        if step % report_every == 0 {
            let seconds = sim.time_ms as f64 / 1000.0;
            let total = options.duration_ms as f64 / 1000.0;
            let progress = Progress {
                fraction: step as f32 / steps as f32,
                message: format!(
                    "{seconds:.0}s of {total:.0}s simulated, {} kills",
                    sim.kills
                ),
            };
            if report(progress) == Control::Stop {
                return Ok(None);
            }
        }
        sim.step();
    }
    Ok(Some(sim.outcome(options, map)))
}

/// Why a run of `options` cannot start, when it cannot.
pub fn validate(options: &Options) -> Result<(), String> {
    if let Player::StandIn { kill_every_ms } = options.player
        && (kill_every_ms < STEP_MS || kill_every_ms % STEP_MS != 0)
    {
        return Err("The stand-in kills on a 10 ms step: use a multiple of 10 ms.".into());
    }
    if options.duration_ms < STEP_MS {
        return Err("A run lasts at least one 10 ms step.".into());
    }
    map(options.map)
        .map(|_| ())
        .ok_or_else(|| match excluded(options.map) {
            Some(reason) => format!("Map {} is not supported: {reason}", options.map),
            None => format!("Map {} has no extracted scene roster.", options.map),
        })
}

struct Sim {
    /// The roster in spawn order, mirroring `MapMonstersList`.
    mobs: Vec<Live>,
    timers: Vec<Timer>,
    time_ms: u64,
    steps: u64,
    frames: u64,
    frame_acc: f64,
    frame_steps_left: u32,
    kills: u64,
    kills_by_type: BTreeMap<String, u64>,
    damage: f64,
    idle_ticks: u64,
    rng: Rng,
    drops: drops::State,
    progress: progress::State,
    grants: Grants,
    timeline: Vec<Sample>,
}

impl Sim {
    fn new(map: &Map, options: &Options) -> Self {
        let player = options.player;
        let mut timers = Vec::new();
        // The player module's `init` registers its timers before the scene
        // starts, so the stand-in's tick is armed before the respawn tick.
        if let Player::StandIn { kill_every_ms } = player {
            let interval = kill_every_ms as i64;
            timers.push(Timer {
                remaining: interval,
                interval,
                kind: TimerKind::Kill,
            });
        }
        // `initCombatProgress` registers the furnace before the scene starts.
        timers.push(Timer {
            remaining: 250,
            interval: 250,
            kind: TimerKind::Forge,
        });
        timers.push(Timer {
            remaining: 1000,
            interval: 1000,
            kind: TimerKind::Respawn,
        });
        Self {
            mobs: map.mobs.iter().map(Live::new).collect(),
            timers,
            time_ms: 0,
            steps: 0,
            frames: 0,
            frame_acc: 0.0,
            frame_steps_left: 0,
            kills: 0,
            kills_by_type: BTreeMap::new(),
            damage: 0.0,
            idle_ticks: 0,
            rng: Rng::new(options.seed),
            drops: drops::State::new(options.drops.clone(), options.drop_inputs),
            progress: progress::State::new(options.progress.clone()),
            grants: options.grants.clone(),
            timeline: Vec::new(),
        }
    }

    /// `_step`: plan the frame's steps, fire due timers, advance the clock and
    /// draw when the batch is spent.
    fn step(&mut self) {
        self.begin_frame();
        self.fire_timers();
        // `initCombatProgress`'s scene updater spends banked pet time.
        self.progress.pet_batch();
        self.time_ms += STEP_MS;
        self.steps += 1;
        if self.time_ms.is_multiple_of(TIMELINE_MS) {
            self.sample();
        }
        self.frame_steps_left -= 1;
        if self.frame_steps_left == 0 {
            self.draw();
        }
    }

    /// One kills-over-time sample, the upstream timeline push.
    fn sample(&mut self) {
        self.timeline.push(Sample {
            t_ms: self.time_ms,
            kills: self.kills,
        });
    }

    /// `_beginFrame`: accumulate frame time, take whole steps while the
    /// accumulator is strictly above one step, and draw a frame that has none.
    fn begin_frame(&mut self) {
        let mut guard = 0;
        while self.frame_steps_left == 0 && guard < 1000 {
            self.frame_acc += FRAME_MS;
            let mut steps = 0;
            while self.frame_acc > STEP_MS as f64 {
                self.frame_acc -= STEP_MS as f64;
                steps += 1;
            }
            if steps == 0 {
                self.draw();
            } else {
                self.frame_steps_left = steps;
            }
            guard += 1;
        }
    }

    /// The literal Stencyl task loop: walk in registration order, subtract the
    /// step, fire at or below zero and re-arm a repeating task.
    fn fire_timers(&mut self) {
        let mut kill_tick = false;
        let mut forge_tick = false;
        let mut respawn_tick = false;
        for timer in &mut self.timers {
            timer.remaining -= STEP_MS as i64;
            if timer.remaining <= 0 {
                match timer.kind {
                    TimerKind::Kill => kill_tick = true,
                    TimerKind::Forge => forge_tick = true,
                    TimerKind::Respawn => respawn_tick = true,
                }
                timer.remaining += timer.interval;
            }
        }
        if kill_tick {
            self.stand_in_kill();
        }
        if forge_tick {
            self.progress.tick(&mut self.rng);
        }
        if respawn_tick {
            for mob in &mut self.mobs {
                if -1.0 < mob.counter {
                    mob.counter -= 1.0;
                }
            }
        }
    }

    /// `scriptedPlayer`: first live crystal or giant, otherwise the first live
    /// monster, for its remaining HP. A tick with nothing alive is an idle tick.
    fn stand_in_kill(&mut self) {
        let Some(index) = self
            .mobs
            .iter()
            .position(|mob| mob.alive && mob.hp > 0.0 && !mob.will_die)
        else {
            self.idle_ticks += 1;
            return;
        };
        let mob = &mut self.mobs[index];
        mob.hit = mob.hp.max(0.0) + 1.0;
        mob.hp -= mob.hit;
        mob.will_die = true;
    }

    /// `drawFrame`: every monster's respawn handler, then its kill handler.
    fn draw(&mut self) {
        self.frames += 1;
        for index in 0..self.mobs.len() {
            self.respawn_handler(index);
            self.kill_handler(index);
        }
    }

    /// `_respawnHandler`: a dead, untempered monster comes back once its
    /// counter is spent.
    fn respawn_handler(&mut self, index: usize) {
        let mob = &mut self.mobs[index];
        if mob.alive || mob.kind.contains("BugNest") || mob.counter > 0.0 {
            return;
        }
        mob.will_die = false;
        mob.hp = mob.hp_tot;
        mob.alive = true;
    }

    /// `_killHandler`: book the kill and arm the respawn counter. The reward
    /// engine's call sits inside the same handler upstream, in the reward gate
    /// this build does not run.
    fn kill_handler(&mut self, index: usize) {
        let mob = &mut self.mobs[index];
        if !mob.alive || mob.counter > 0.0 || mob.hp > 0.0 {
            return;
        }
        // `max(RespawnTime - talent302, 4)`: no buffs are active in this run.
        mob.counter = mob.respawn.max(4.0);
        mob.alive = false;
        let kind = mob.kind.clone();
        self.kills += 1;
        self.damage += mob.hit;
        *self.kills_by_type.entry(kind.clone()).or_insert(0) += 1;
        // The reward gate's `monsterKill` and drop branch run inside the kill
        // handler; the progress credits and the card and obol rows are modelled.
        if drops::rolls(&kind) {
            let grants = self.grants.clone();
            self.progress.kill(&mut self.rng, &grants, &kind);
            self.drops.kill(&mut self.rng, &kind);
        }
    }

    fn outcome(mut self, options: Options, map: &Map) -> Outcome {
        // The chart always ends on the run's own numbers, even when the
        // duration is not a whole ten-second mark.
        if self
            .timeline
            .last()
            .is_none_or(|sample| sample.t_ms != self.time_ms)
        {
            self.sample();
        }
        Outcome {
            map: options.map,
            scene: map.scene.clone(),
            player: options.player.id(),
            duration_ms: options.duration_ms,
            simulated_ms: self.time_ms,
            steps: self.steps,
            frames: self.frames,
            kills: self.kills,
            kills_by_type: self.kills_by_type,
            damage: self.damage,
            idle_ticks: self.idle_ticks,
            respawn: self.mobs.iter().map(|mob| mob.counter).collect(),
            seed: options.seed,
            timeline: self.timeline,
            drops: self.drops.summary(),
            progress: self.progress.summary(),
        }
    }
}
