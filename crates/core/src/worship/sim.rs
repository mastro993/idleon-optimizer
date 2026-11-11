//! The deterministic 100 ms Worship TD run.
//!
//! A port of `createWorshipTdRun` in
//! `js/stats/systems/w3/worship-td.js`, reduced to what the live simulator
//! needs: wave countdowns, spawns, mob timers, tower firing, projectiles and
//! the totem. Plans, phases and the strategy search are later issues.

use super::{Replay, ReplayConfig, ReplayFrame};
use super::{
    TICKS_PER_SECOND, Trial, Wave, atom5_bonus, aura_applies, data, distance, geometry, hp_multi,
    kill_credit, kill_exp, new_tower_cost, placement_bands, skip_reward, tower_atkcd,
    tower_centers, tower_damage, tower_damage_multi, tower_range, tower_upgrade_cost,
    validate_placement, wave_row,
};

/// A seeded run's inputs: the save-derived account values the simulator reads.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Inputs {
    /// Tower trial levels: `Tower[9 + type]` in the save.
    pub tower_levels: [u32; 9],
    /// Best cleared waves per trial, for "new best" and the trial selector.
    pub best_waves: [u32; 8],
    /// Starting TD points.
    pub start_pts: f64,
    pub totem_hp: f64,
    /// Menagerie unlock 18 doubles each tower's first trait.
    pub first_trait_multi: f64,
    /// `Dream[7]`, the per-second wave-clock damage bonus.
    pub dream7: f64,
    /// `Menagerie` boost 15, capped by the wave clock.
    pub slow_roast_pre_cook_sec: f64,
    /// `Salt Lick[6]` as a multiplier.
    pub salt_lick_x: f64,
    /// Meals x Meritocracy kill-point multiplier.
    pub kill_pts_mult: f64,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            tower_levels: [0; 9],
            best_waves: [0; 8],
            start_pts: 5.0,
            totem_hp: 5.0,
            first_trait_multi: 1.0,
            dream7: 0.0,
            slow_roast_pre_cook_sec: 0.0,
            salt_lick_x: 1.0,
            kill_pts_mult: 1.0,
        }
    }
}

impl Inputs {
    /// The `Atom 5` tower-level damage bonus.
    pub fn atom5_bonus(&self) -> f64 {
        atom5_bonus(&self.tower_levels)
    }

    /// Whether a tower type is unlocked.
    pub fn unlocked(&self, type_index: usize) -> bool {
        self.tower_levels.get(type_index).copied().unwrap_or(0) >= 1
    }
}

/// Why a run stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The run hit its time limit.
    Time,
    /// The totem fell.
    Totem,
    /// The run reached the wave limit.
    MaxWave,
    /// A boss hold made no progress.
    Stalled,
}

impl EndReason {
    /// The word the HUD shows.
    pub fn label(self) -> &'static str {
        match self {
            EndReason::Time => "time limit",
            EndReason::Totem => "totem destroyed",
            EndReason::MaxWave => "wave limit",
            EndReason::Stalled => "stalled",
        }
    }
}

/// A refusal from a build action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunError {
    /// The run already finished; restart it.
    Finished,
    /// The tower type's trial level is below 1.
    Locked,
    /// The points on hand cannot cover the cost.
    Cost,
    /// The placement failed, with the game's reason.
    Placement(&'static str),
    /// No tower with that id stands.
    NoTower,
}

impl RunError {
    /// The line the section shows.
    pub fn label(&self) -> String {
        match self {
            RunError::Finished => "The run has ended — restart it".into(),
            RunError::Locked => "That tower type is locked at trial level 0".into(),
            RunError::Cost => "Not enough TD points".into(),
            RunError::Placement(reason) => (*reason).into(),
            RunError::NoTower => "That tower is gone".into(),
        }
    }
}

/// A completed build action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub id: u64,
    pub cost: f64,
}

/// A tower as described to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct TowerView {
    pub id: u64,
    pub type_index: usize,
    pub level: u32,
    pub traits: String,
    pub node: usize,
    pub x: f64,
    pub wx: f64,
    pub wy: f64,
    pub cx: f64,
    pub cy: f64,
    pub facing: f64,
    /// 0 cooling, 1 popup, 2 ready.
    pub state: u8,
    pub cd: f64,
    pub cd_max: f64,
    pub range: f64,
    pub last_fire: Option<u64>,
}

/// A live mob as described to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct MobView {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub hp_frac: f64,
    pub size: f64,
    pub mods: i64,
    pub frozen: bool,
    pub healing: bool,
    pub skull: f64,
    pub face_left: bool,
    pub walking: bool,
    pub node: usize,
    pub node_y: f64,
}

/// A projectile the renderer draws.
#[derive(Clone, Debug, PartialEq)]
pub struct BoulderView {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub dir: f64,
    pub born: u64,
}

/// A poison cloud the renderer draws.
#[derive(Clone, Debug, PartialEq)]
pub struct CloudView {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub born: u64,
    pub until: u64,
}

/// A Kraken underling the renderer draws.
#[derive(Clone, Debug, PartialEq)]
pub struct KrakenView {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub hits: f64,
}

/// A recent visual effect; never read by the simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub at: u64,
    pub end: u64,
    pub kind: EffectKind,
}

/// One visual effect's shape.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectKind {
    Pulse {
        x0: f64,
        y0: f64,
        x: f64,
        y: f64,
        hits: f64,
    },
    Fireball {
        x0: f64,
        y0: f64,
        x: f64,
        y: f64,
        dir: f64,
    },
    Boom {
        x: f64,
        y: f64,
        radius: f64,
        dir: f64,
    },
    Storm {
        x: f64,
        y: f64,
        mob: u64,
    },
    Ring {
        x: f64,
        y: f64,
        radius: f64,
        anim: usize,
    },
    Damage {
        x: f64,
        y: f64,
        value: f64,
        crit: bool,
        blocked: bool,
    },
    Stun {
        x: f64,
        y: f64,
    },
}

impl RunView {
    /// A run that has not started, for the first render before the pump builds it.
    pub fn empty() -> Self {
        Self {
            tick: 0,
            t: 0.0,
            wave: 0,
            countdown: 6.0,
            pts: 0.0,
            totem: 0.0,
            done: false,
            end_reason: None,
            mobs: Vec::new(),
            towers: Vec::new(),
            boulders: Vec::new(),
            clouds: Vec::new(),
            krakens: Vec::new(),
            effects: Vec::new(),
        }
    }
}

/// A plain snapshot of a live run for rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct RunView {
    pub tick: u64,
    pub t: f64,
    pub wave: u32,
    pub countdown: f64,
    pub pts: f64,
    pub totem: f64,
    pub done: bool,
    pub end_reason: Option<EndReason>,
    pub mobs: Vec<MobView>,
    pub towers: Vec<TowerView>,
    pub boulders: Vec<BoulderView>,
    pub clouds: Vec<CloudView>,
    pub krakens: Vec<KrakenView>,
    pub effects: Vec<Effect>,
}

/// A completed run's totals.
#[derive(Clone, Debug, PartialEq)]
pub struct RunResult {
    pub end_reason: EndReason,
    pub time_sec: f64,
    pub wave_reached: u32,
    pub highest_wave: u32,
    pub best_wave: u32,
    pub best_wave_time_sec: Option<f64>,
    pub new_best: bool,
    pub saved_best: u32,
    pub pts: f64,
    pub spent: f64,
    pub kill_pts: f64,
    pub skip_pts: f64,
    pub kills: u64,
    pub exp: f64,
    pub totem_damage: u32,
    pub totem_left: f64,
    pub leaks: u32,
    pub end_pending_frac: f64,
    pub towers: Vec<TowerView>,
}

#[derive(Clone, Debug)]
struct Tower {
    id: u64,
    type_index: usize,
    node: usize,
    x: f64,
    level: u32,
    traits: String,
    cd: f64,
    cd_max: f64,
    state: u8,
    ready_at: u64,
    last_fire: Option<u64>,
    map: [f64; 10],
    refund: f64,
    wx: f64,
    wy: f64,
    cx: f64,
    cy: f64,
    facing: f64,
    underlings: i64,
    shots: u64,
    damage: f64,
}

impl Tower {
    fn crit_chance(&self) -> f64 {
        self.map[1]
    }
}

#[derive(Clone, Debug)]
struct Mob {
    id: u64,
    pi: usize,
    left: f64,
    vx: f64,
    ms: f64,
    base_speed: f64,
    hp: f64,
    max_hp: f64,
    size: f64,
    status_skull: f64,
    mods: i64,
    dead: bool,
    pers: i64,
    freeze_t: f64,
    heal_t: f64,
    born: u64,
    push: Option<Push>,
}

#[derive(Clone, Debug)]
struct Boulder {
    id: u64,
    tower: u64,
    node: usize,
    x: f64,
    born: u64,
    dir: f64,
    until: u64,
    left: i64,
    hit: Vec<u64>,
    damage: f64,
    crit_chance: f64,
}

#[derive(Clone, Debug)]
struct Cloud {
    id: u64,
    tower: u64,
    node: usize,
    x: f64,
    y: f64,
    born: u64,
    until: u64,
    period: f64,
    next: u64,
    damage: f64,
}

#[derive(Clone, Debug)]
struct Kraken {
    id: u64,
    tower: u64,
    node: usize,
    x: f64,
    y: f64,
    vx: f64,
    hits: f64,
    damage: f64,
    dir: f64,
    target: Option<u64>,
    next: u64,
    clear_at: u64,
    wander_at: i64,
    push: Option<Push>,
}

#[derive(Clone, Debug)]
struct Spawn {
    at: u64,
    wave: Wave,
}

#[derive(Clone, Copy, Debug)]
struct Push {
    from: f64,
    dx: f64,
    start: u64,
}

/// A delayed hit: fireball blasts and storm bolts land on later ticks.
#[derive(Clone, Debug)]
enum Landing {
    Blast {
        node: usize,
        x: f64,
        radius: f64,
        damage: f64,
        tower: u64,
        dir: f64,
        crit_chance: f64,
    },
    Storm {
        target: u64,
        damage: f64,
        tower: u64,
        crit_chance: f64,
        x: f64,
        y: f64,
    },
}

#[derive(Clone, Debug)]
struct Timed {
    at: u64,
    landing: Landing,
}

/// `quadOut`: a `moveBy` tween's ease.
fn quad_out(t: f64) -> f64 {
    t * (2.0 - t)
}

/// Where a push tween has moved an actor on this tick.
fn advance_push(push: Push, tick: u64) -> f64 {
    let elapsed = tick.saturating_sub(push.start);
    push.from + push.dx * quad_out((elapsed as f64 / PUSH_TICKS as f64).min(1.0))
}

/// One seeded run, stepped tick by tick.
pub struct Run {
    inputs: Inputs,
    trial_index: usize,
    rng: Rng,
    max_wave: u32,
    max_ticks: u64,
    tick: u64,
    /// The current 1-based wave.
    wave: u32,
    highest_wave: u32,
    /// Seconds until the next wave (or 9999 while a boss holds).
    countdown: f64,
    /// Pending spawn HP that has not entered the field yet.
    pending_hp: f64,
    /// Seconds into the wave clock (`G85`).
    wave_clock: f64,
    pts: f64,
    totem: f64,
    towers: Vec<Tower>,
    mobs: Vec<Mob>,
    spawns: Vec<Spawn>,
    landings: Vec<Timed>,
    boulders: Vec<Boulder>,
    clouds: Vec<Cloud>,
    krakens: Vec<Kraken>,
    effects: Vec<Effect>,
    kills: u64,
    exp: f64,
    totem_damage: u32,
    leaks: u32,
    spent: f64,
    skip_pts: f64,
    kill_pts: f64,
    best_cleared: u32,
    best_cleared_tick: Option<u64>,
    finished: bool,
    end_reason: EndReason,
    stall_min: f64,
    stall_pts: f64,
    stall_totem: f64,
    stall_tick: u64,
    crit_flag: bool,
    next_mob: u64,
    next_object: u64,
    next_tower: u64,
    tower_version: u64,
    replay: Option<Recorder>,
    result: Option<RunResult>,
}

/// One tower's aura pass: type, sides, range and centre.
type AuraSource = (usize, Vec<(bool, f64, usize)>, f64, f64, f64);

/// Mulberry32, the upstream seeded generator.
struct Rng {
    a: u32,
}

impl Rng {
    fn new(seed: u64) -> Self {
        let scaled = (seed as f64 * 2_654_435_761.0).rem_euclid(4_294_967_296.0) as u32;
        Self { a: scaled }
    }

    fn next(&mut self) -> f64 {
        self.a = self.a.wrapping_add(0x6D2B_79F5);
        let mut t = self.a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    fn int(&mut self, a: i64, b: i64) -> i64 {
        a + (self.next() * (b - a + 1) as f64).floor() as i64
    }
}

/// The run's opt-in replay recorder.
struct Recorder {
    interval_sec: f64,
    every: u64,
    from_wave: u32,
    max_frames: usize,
    frames: Vec<ReplayFrame>,
    tower_sets: Vec<Vec<TowerView>>,
    set_version: u64,
}

impl Recorder {
    fn new(config: ReplayConfig) -> Self {
        let every = (config.interval_sec * TICKS_PER_SECOND).round().max(1.0) as u64;
        Self {
            interval_sec: every as f64 / TICKS_PER_SECOND,
            every,
            from_wave: config.from_wave,
            max_frames: config.max_frames.max(16),
            frames: Vec::new(),
            tower_sets: Vec::new(),
            set_version: u64::MAX,
        }
    }

    fn record(&mut self, run: &Run) {
        if !run.tick.is_multiple_of(self.every) || run.wave < self.from_wave {
            return;
        }
        self.capture(run);
    }

    fn capture(&mut self, run: &Run) {
        if run.tower_version != self.set_version {
            // Replay layouts are static: cooldown and firing state move every
            // tick and belong to the live run, not to a scrub.
            let mut towers = run.tower_views();
            for tower in &mut towers {
                tower.state = 0;
                tower.cd = 0.0;
                tower.cd_max = 0.0;
                tower.last_fire = None;
            }
            self.tower_sets.push(towers);
            self.set_version = run.tower_version;
        }
        self.frames.push(ReplayFrame {
            tick: run.tick,
            t: run.tick as f64 / TICKS_PER_SECOND,
            wave: run.wave,
            countdown: run.countdown,
            pts: run.pts,
            totem: run.totem,
            set: self.tower_sets.len().saturating_sub(1),
            mobs: run.mob_views(),
            clouds: run.cloud_views(),
            krakens: run.kraken_views(),
        });
        if self.frames.len() > self.max_frames {
            self.every *= 2;
            self.interval_sec = self.every as f64 / TICKS_PER_SECOND;
            let every = self.every;
            self.frames.retain(|frame| frame.tick.is_multiple_of(every));
        }
    }

    fn finish(&mut self, run: &Run) {
        if self
            .frames
            .last()
            .is_none_or(|frame| frame.tick != run.tick)
        {
            self.capture(run);
        }
    }

    fn replay(&self, run: &Run) -> Replay {
        // Drop tower sets no frame references, renumbering the rest.
        let mut tower_sets: Vec<Vec<TowerView>> = Vec::new();
        let mut mapping = vec![None; self.tower_sets.len()];
        let mut frames = self.frames.clone();
        for frame in &mut frames {
            let compact = match mapping.get(frame.set).copied().flatten() {
                Some(index) => index,
                None => {
                    let index = tower_sets.len();
                    tower_sets.push(self.tower_sets.get(frame.set).cloned().unwrap_or_default());
                    if let Some(slot) = mapping.get_mut(frame.set) {
                        *slot = Some(index);
                    }
                    index
                }
            };
            frame.set = compact;
        }
        Replay::new(
            self.interval_sec,
            frames,
            tower_sets,
            run.finished,
            run.finished.then_some(run.end_reason),
        )
    }
}

const TRAIT_RECIPE: &str = "AAAAAAAAAAAA";
const STALL_SEC: f64 = 120.0;
const PUSH_TICKS: u64 = 6;

impl Run {
    /// A fresh run on a trial.
    pub fn new(inputs: Inputs, trial_index: usize, seed: u64) -> Self {
        Self::new_inner(inputs, trial_index, seed, None)
    }

    /// A fresh run that records a replay at `config`'s interval.
    pub fn new_with_replay(
        inputs: Inputs,
        trial_index: usize,
        seed: u64,
        config: ReplayConfig,
    ) -> Self {
        Self::new_inner(inputs, trial_index, seed, Some(config))
    }

    fn new_inner(
        inputs: Inputs,
        trial_index: usize,
        seed: u64,
        replay: Option<ReplayConfig>,
    ) -> Self {
        let max_ticks = (4.0 * 3600.0 * TICKS_PER_SECOND) as u64;
        Self {
            pts: inputs.start_pts,
            totem: inputs.totem_hp,
            inputs,
            trial_index,
            rng: Rng::new(seed),
            max_wave: 1000,
            max_ticks,
            tick: 0,
            wave: 0,
            highest_wave: 0,
            countdown: 6.0,
            pending_hp: 0.0,
            wave_clock: 0.0,
            towers: Vec::new(),
            mobs: Vec::new(),
            spawns: Vec::new(),
            landings: Vec::new(),
            boulders: Vec::new(),
            clouds: Vec::new(),
            krakens: Vec::new(),
            effects: Vec::new(),
            kills: 0,
            exp: 0.0,
            totem_damage: 0,
            leaks: 0,
            spent: 0.0,
            skip_pts: 0.0,
            kill_pts: 0.0,
            best_cleared: 0,
            best_cleared_tick: None,
            finished: false,
            end_reason: EndReason::Time,
            stall_min: f64::INFINITY,
            stall_pts: 5.0,
            stall_totem: 5.0,
            stall_tick: 0,
            crit_flag: false,
            next_mob: 0,
            next_object: 0,
            next_tower: 0,
            tower_version: 0,
            replay: replay.map(Recorder::new),
            result: None,
        }
    }

    /// The trial this run plays.
    pub fn trial(&self) -> &'static Trial {
        super::trial(self.trial_index).expect("the run's trial exists in the shipped data")
    }

    /// Whether the run has stopped.
    pub fn done(&self) -> bool {
        self.finished
    }

    /// The current tick.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The points on hand.
    pub fn points(&self) -> f64 {
        self.pts
    }

    fn geom_node_y(&self, node: usize) -> f64 {
        geometry(self.trial())
            .and_then(|geom| geom.nodes.get(node))
            .map(|def| def.y)
            .unwrap_or(0.0)
    }

    fn mob_cx(&self, mob: &Mob) -> f64 {
        mob.left + self.trial().mob_def.frame_w / 2.0
    }

    fn mob_cy(&self, mob: &Mob) -> f64 {
        let mob_def = &self.trial().mob_def;
        let y = self.geom_node_y(self.trial().path[mob.pi]);
        y - mob_def.height + (mob_def.height - 50.0) / 2.0 + mob_def.frame_h / 2.0
    }

    fn trial_level(&self, type_index: usize) -> u32 {
        self.inputs.tower_levels[type_index]
    }

    /// Place a tower of `type_index` at a requested spot.
    pub fn place(&mut self, type_index: usize, node: usize, x: f64) -> Result<Placed, RunError> {
        if self.finished {
            return Err(RunError::Finished);
        }
        let trial = self.trial();
        if !self.inputs.unlocked(type_index) {
            return Err(RunError::Locked);
        }
        let existing: Vec<super::Spot> = self
            .towers
            .iter()
            .map(|tower| super::Spot {
                node: tower.node,
                x: tower.x,
            })
            .collect();
        let check = validate_placement(trial, &existing, node, x, 0.0);
        if !check.ok {
            return Err(RunError::Placement(check.reason));
        }
        let cost = new_tower_cost(type_index, self.towers.len());
        if self.pts < cost {
            return Err(RunError::Cost);
        }
        let centers = tower_centers(trial, node, check.x);
        let id = self.next_tower + 1;
        self.next_tower = id;
        self.towers.push(Tower {
            id,
            type_index,
            node,
            x: check.x,
            level: 1,
            traits: String::new(),
            cd: 0.0,
            cd_max: tower_atkcd(type_index, 1, self.trial_level(type_index)),
            state: 1,
            ready_at: self.tick + 4,
            last_fire: None,
            map: [0.0; 10],
            refund: (0.6 * cost).ceil(),
            wx: centers.wx,
            wy: centers.wy,
            cx: centers.cx,
            cy: centers.cy,
            facing: centers.facing,
            underlings: 0,
            shots: 0,
            damage: 0.0,
        });
        self.pts -= cost;
        self.spent += cost;
        self.apply_traits();
        self.tower_version += 1;
        Ok(Placed { id, cost })
    }

    /// What selling a tower refunds.
    pub fn refund(&self, id: u64) -> f64 {
        self.towers
            .iter()
            .find(|tower| tower.id == id)
            .map(|tower| tower.refund)
            .unwrap_or(0.0)
    }

    /// The cost of the next level of a standing tower.
    pub fn upgrade_cost(&self, id: u64) -> Option<f64> {
        let tower = self.towers.iter().find(|tower| tower.id == id)?;
        Some(tower_upgrade_cost(
            tower.type_index,
            tower.level,
            self.trial_level(tower.type_index),
        ))
    }

    /// Upgrade a standing tower by one level.
    pub fn upgrade(&mut self, id: u64) -> Result<Placed, RunError> {
        if self.finished {
            return Err(RunError::Finished);
        }
        let Some(index) = self.towers.iter().position(|tower| tower.id == id) else {
            return Err(RunError::NoTower);
        };
        let cost = tower_upgrade_cost(
            self.towers[index].type_index,
            self.towers[index].level,
            self.trial_level(self.towers[index].type_index),
        );
        if self.pts < cost {
            return Err(RunError::Cost);
        }
        self.pts -= cost;
        self.spent += cost;
        let trial_level = self.trial_level(self.towers[index].type_index);
        let tower = &mut self.towers[index];
        tower.level += 1;
        tower.refund += (0.6 * cost).round();
        tower.cd_max = tower_atkcd(tower.type_index, tower.level, trial_level);
        let earned = (tower.level / 5) as usize;
        let mut learned = false;
        while tower.traits.len() < earned {
            let next = TRAIT_RECIPE.chars().nth(tower.traits.len()).unwrap_or('A');
            tower.traits.push(next);
            learned = true;
        }
        if learned {
            self.apply_traits();
        }
        self.tower_version += 1;
        Ok(Placed { id, cost })
    }

    /// Sell a tower, refunding 60% of everything spent on it.
    pub fn sell(&mut self, id: u64) -> Result<f64, RunError> {
        if self.finished {
            return Err(RunError::Finished);
        }
        let Some(index) = self.towers.iter().position(|tower| tower.id == id) else {
            return Err(RunError::NoTower);
        };
        let refund = self.towers.remove(index).refund;
        self.pts = (self.pts + refund).round();
        self.apply_traits();
        self.tower_version += 1;
        Ok(refund)
    }

    /// Skip the rest of the wave countdown, awarding its points.
    pub fn skip_wave(&mut self) -> f64 {
        if self.finished || self.countdown > 999.0 {
            return 0.0;
        }
        let reward = skip_reward(self.countdown, self.wave);
        self.pts += reward;
        self.skip_pts += reward;
        self.countdown = 0.0;
        reward
    }

    /// Rebuild every tower's trait map and cooldown, as the game's trait pass does.
    fn apply_traits(&mut self) {
        let first = self.inputs.first_trait_multi;
        for tower in &mut self.towers {
            let def = &data().traits[tower.type_index];
            tower.map = [0.0; 10];
            for (index, side) in tower.traits.chars().enumerate() {
                let side = if side == 'B' { &def.b } else { &def.a };
                tower.map[side.key] += side.value * if index == 0 { first } else { 1.0 };
            }
        }
        let sources: Vec<AuraSource> = self
            .towers
            .iter()
            .map(|tower| {
                let def = &data().traits[tower.type_index];
                let range = tower_range(
                    tower.type_index,
                    tower.level,
                    self.trial_level(tower.type_index),
                ) + 10.0;
                let sides = tower
                    .traits
                    .chars()
                    .enumerate()
                    .map(|(index, letter)| {
                        let side = if letter == 'B' { &def.b } else { &def.a };
                        (
                            letter != 'B',
                            side.value * if index == 0 { first } else { 1.0 },
                            side.key,
                        )
                    })
                    .collect();
                (tower.type_index, sides, range, tower.cx, tower.cy)
            })
            .collect();
        for (i, source) in sources.iter().enumerate() {
            let (src_type, range, src_cx, src_cy) = (source.0, source.2, source.3, source.4);
            for (is_a, value, key) in &source.1 {
                for (j, dest) in self.towers.iter_mut().enumerate() {
                    let dist = distance(dest.cx - src_cx, dest.cy - src_cy);
                    if aura_applies(src_type, dist, *is_a, range, i == j) {
                        dest.map[*key] += value;
                    }
                }
            }
        }
        let levels = self.inputs.tower_levels;
        for tower in &mut self.towers {
            tower.cd_max = tower_atkcd(tower.type_index, tower.level, levels[tower.type_index])
                / (1.0 + tower.map[0] / 100.0);
        }
    }

    fn damage_multi(&self) -> f64 {
        let first_map9 = self.towers.first().map(|tower| tower.map[9]).unwrap_or(0.0);
        tower_damage_multi(&self.inputs, first_map9, self.wave_clock)
    }

    fn crit(&mut self, tower: u64, damage: f64) -> f64 {
        let chance = self
            .towers
            .iter()
            .find(|tower_def| tower_def.id == tower)
            .map(Tower::crit_chance)
            .unwrap_or(0.0);
        self.crit_with(chance, damage)
    }

    fn crit_with(&mut self, chance: f64, damage: f64) -> f64 {
        self.crit_flag = chance > self.rng.int(0, 1000) as f64 / 10.0;
        if self.crit_flag { damage * 1.5 } else { damage }
    }

    fn hit(&mut self, mob_index: usize, mut damage: f64, tower: Option<u64>) {
        let was_crit = self.crit_flag;
        self.crit_flag = false;
        if self.mobs.get(mob_index).is_none_or(|mob| mob.dead) {
            return;
        }
        let path = self.trial().path.clone();
        let (mods, pi) = (self.mobs[mob_index].mods, self.mobs[mob_index].pi);
        if mods == 2 {
            if self.mobs[mob_index].pers >= 0 {
                self.mobs[mob_index].pers += 1;
                if self.mobs[mob_index].pers > 5 {
                    self.mobs[mob_index].pers = -3;
                    damage = 0.0;
                }
            } else {
                damage = 0.0;
            }
        } else if mods == 5 {
            let node = geometry(self.trial())
                .and_then(|geom| geom.nodes.get(path[pi]))
                .cloned();
            if let Some(node) = node {
                let left = if self.mobs[mob_index].vx > 0.0 {
                    (self.mobs[mob_index].left + 10.0)
                        .min(node.length - 10.0)
                        .round()
                } else {
                    (self.mobs[mob_index].left - 10.0)
                        .max(node.x + 10.0)
                        .round()
                };
                self.mobs[mob_index].left = left;
            }
        }
        let x = self.mob_cx(&self.mobs[mob_index]);
        let y = self.mob_cy(&self.mobs[mob_index]);
        let size = self.mobs[mob_index].size;
        self.damage_number(x, y, size, damage, was_crit);
        if damage <= 0.0 {
            return;
        }
        self.mobs[mob_index].hp -= damage;
        if let Some(id) = tower
            && let Some(tower) = self.towers.iter_mut().find(|tower| tower.id == id)
        {
            tower.damage += damage.min(self.mobs[mob_index].hp + damage);
        }
        if self.mobs[mob_index].hp <= 0.0 {
            self.kill_mob(mob_index);
        }
    }

    fn damage_number(&mut self, x: f64, y: f64, size: f64, damage: f64, crit: bool) {
        if self.effects.len() > 400 {
            return;
        }
        let y = y - self.trial().mob_def.frame_h * size / 2.0;
        self.effects.push(Effect {
            at: self.tick,
            end: self.tick + 8,
            kind: EffectKind::Damage {
                x,
                y,
                value: damage,
                crit,
                blocked: damage <= 0.0,
            },
        });
    }

    fn kill_mob(&mut self, index: usize) {
        self.mobs[index].dead = true;
        if self.totem > 0.5 {
            let (status_skull, size) = (self.mobs[index].status_skull, self.mobs[index].size);
            let gain = kill_credit(
                status_skull,
                size,
                f64::from(self.wave),
                self.inputs.salt_lick_x,
                self.inputs.kill_pts_mult,
            );
            let before = self.pts;
            self.pts = (self.pts + gain).round();
            self.kill_pts += self.pts - before;
            self.exp += kill_exp(self.trial_index, status_skull);
            self.kills += 1;
        }
        let cx = self.mob_cx(&self.mobs[index]);
        let cy = self.mob_cy(&self.mobs[index]);
        let mods = self.mobs[index].mods;
        if mods == 4 {
            let stuns: Vec<(f64, f64)> = self
                .towers
                .iter_mut()
                .filter(|tower| {
                    distance(tower.cx - cx, tower.cy - cy) < 70.0 && tower.cd < 0.1 + tower.cd_max
                })
                .map(|tower| {
                    tower.cd = 4.0 + tower.cd_max;
                    (tower.wx, tower.wy - 40.0)
                })
                .collect();
            for (x, y) in stuns {
                self.effects.push(Effect {
                    at: self.tick,
                    end: self.tick + 10,
                    kind: EffectKind::Stun { x, y },
                });
            }
        }
        if mods == 1 {
            let nearby: Vec<usize> = self
                .mobs
                .iter()
                .enumerate()
                .filter(|(_, mob)| {
                    !mob.dead && distance(self.mob_cx(mob) - cx, self.mob_cy(mob) - cy) < 60.0
                })
                .map(|(index, _)| index)
                .collect();
            for index in nearby {
                self.mobs[index].heal_t = 3.0;
            }
        }
    }

    fn freeze(&mut self, index: usize, tower_id: u64) {
        let (tower_map4, tower_map5) = self
            .towers
            .iter()
            .find(|tower| tower.id == tower_id)
            .map(|tower| (tower.map[4], tower.map[5]))
            .unwrap_or((0.0, 0.0));
        let mob = &mut self.mobs[index];
        let slowed = mob.vx * (0.3 / (1.0 + tower_map4 / 100.0));
        mob.vx = if mob.vx > 0.0 {
            slowed.ceil()
        } else {
            slowed.floor()
        };
        mob.ms = mob.vx.abs();
        mob.freeze_t = 6.0 + tower_map5;
    }

    fn find_target(&self, tower: &Tower, range: f64) -> Option<usize> {
        let trial = self.trial();
        let mut best: Option<usize> = None;
        for (index, mob) in self.mobs.iter().enumerate() {
            if mob.dead {
                continue;
            }
            if distance(self.mob_cx(mob) - tower.wx, self.mob_cy(mob) - tower.wy) >= range {
                continue;
            }
            let Some(current) = best else {
                best = Some(index);
                continue;
            };
            let other = &self.mobs[current];
            if mob.pi == other.pi {
                let flag = trial.flags.get(mob.pi).copied().unwrap_or(0);
                if (flag == 0 && mob.left > other.left) || (flag == 1 && mob.left < other.left) {
                    best = Some(index);
                }
            } else if mob.pi > other.pi {
                best = Some(index);
            }
        }
        best
    }

    fn at(&mut self, delay: u64, landing: Landing) {
        self.landings.push(Timed {
            at: self.tick + delay,
            landing,
        });
    }

    fn launched(&mut self, index: usize) {
        let tower = &mut self.towers[index];
        tower.cd = tower.cd_max;
        tower.state = 0;
        tower.shots += 1;
        tower.last_fire = Some(self.tick);
    }

    fn fire(&mut self, index: usize) {
        let trial = self.trial();
        let type_index = self.towers[index].type_index;
        let level = self.towers[index].level;
        let trial_level = self.trial_level(type_index);
        let range = tower_range(type_index, level, trial_level);
        let damage = tower_damage(type_index, level, trial_level, self.damage_multi());
        if type_index == 6 {
            if self.towers[index].underlings >= (4.0 + self.towers[index].map[4]).round() as i64 {
                return;
            }
            self.launched(index);
            let tower = &mut self.towers[index];
            tower.underlings += 1;
            let (node, x, y, facing, map5) =
                (tower.node, tower.cx, tower.cy, tower.facing, tower.map[5]);
            let id = self.next_object + 1;
            self.next_object = id;
            let vx = if self.rng.int(1, 2) == 1 { -3.0 } else { 3.0 };
            self.krakens.push(Kraken {
                id,
                tower: self.towers[index].id,
                node,
                x,
                y,
                vx,
                hits: 2.0 + map5,
                damage: damage.max(1.0),
                dir: if facing == 1.0 { -1.0 } else { 1.0 },
                target: None,
                next: self.tick + 2,
                clear_at: self.tick + 20,
                wander_at: -1,
                push: None,
            });
            return;
        }
        let Some(target) = self.find_target(&self.towers[index], range) else {
            return;
        };
        let tower_id = self.towers[index].id;
        let tcx = self.mob_cx(&self.mobs[target]);
        let target_vx = self.mobs[target].vx;
        let target_node = trial.path[self.mobs[target].pi];
        let node = geometry(trial)
            .and_then(|geom| geom.nodes.get(target_node))
            .cloned()
            .unwrap_or(super::Node {
                x: 0.0,
                length: 0.0,
                y: 0.0,
                neighbors: Vec::new(),
            });
        let map3 = self.towers[index].map[3];
        let map4 = self.towers[index].map[4];
        let map5 = self.towers[index].map[5];
        match type_index {
            0 => {
                self.launched(index);
                let n = 1.0 + (map3 / 100.0).floor();
                let extra = if map3 % 100.0 > self.rng.int(0, 100) as f64 {
                    1.0
                } else {
                    0.0
                };
                let hits = n + extra;
                let is_crit = self.towers[index].map[1] > self.rng.int(0, 1000) as f64 / 10.0;
                let damage = if is_crit { damage * 1.5 } else { damage };
                let (wx, wy) = (self.towers[index].wx, self.towers[index].wy);
                self.effects.push(Effect {
                    at: self.tick,
                    end: self.tick + 3,
                    kind: EffectKind::Pulse {
                        x0: wx,
                        y0: wy - 20.0,
                        x: tcx,
                        y: self.mob_cy(&self.mobs[target]),
                        hits,
                    },
                });
                for _ in 0..hits as usize {
                    if self.mobs[target].dead {
                        break;
                    }
                    self.crit_flag = is_crit;
                    self.hit(target, damage, Some(tower_id));
                    let size = self.mobs[target].size;
                    if !self.mobs[target].dead && size != 4.0 && map5 > 0.0 {
                        let path_node = geometry(self.trial())
                            .and_then(|geom| {
                                geom.nodes.get(self.trial().path[self.mobs[target].pi])
                            })
                            .cloned()
                            .unwrap_or(super::Node {
                                x: 0.0,
                                length: 0.0,
                                y: 0.0,
                                neighbors: Vec::new(),
                            });
                        let left = if self.mobs[target].vx > 0.0 {
                            (self.mobs[target].left - map5)
                                .max(path_node.x + 10.0)
                                .round()
                        } else {
                            (self.mobs[target].left + map5)
                                .min(path_node.length - 10.0)
                                .round()
                        };
                        self.mobs[target].left = left;
                    }
                }
            }
            1 => {
                self.launched(index);
                let dir = if tcx + 12.0 * target_vx > self.towers[index].wx {
                    1.0
                } else {
                    -1.0
                };
                let land_x = (tcx + 18.3 * target_vx)
                    .max(node.x)
                    .min(node.length)
                    .round();
                let radius = ((100.0 + map5).round() / 100.0) * 40.0;
                let ox = self.towers[index].wx - 22.0 + 36.0;
                let oy = node.y - 51.0 + 40.0;
                self.effects.push(Effect {
                    at: self.tick,
                    end: self.tick + 18,
                    kind: EffectKind::Fireball {
                        x0: ox,
                        y0: oy,
                        x: land_x,
                        y: node.y,
                        dir,
                    },
                });
                self.at(
                    18,
                    Landing::Blast {
                        node: target_node,
                        x: land_x,
                        radius,
                        damage,
                        tower: tower_id,
                        dir,
                        crit_chance: self.towers[index].map[1],
                    },
                );
                if map4 > self.rng.int(1, 100) as f64 {
                    let x2 = (self.rng.int(-50, 50) as f64 + tcx + 12.0 * target_vx)
                        .max(node.x)
                        .min(node.length)
                        .round();
                    self.effects.push(Effect {
                        at: self.tick + 5,
                        end: self.tick + 23,
                        kind: EffectKind::Fireball {
                            x0: ox,
                            y0: oy,
                            x: x2,
                            y: node.y,
                            dir,
                        },
                    });
                    self.at(
                        23,
                        Landing::Blast {
                            node: target_node,
                            x: x2,
                            radius,
                            damage,
                            tower: tower_id,
                            dir,
                            crit_chance: self.towers[index].map[1],
                        },
                    );
                }
            }
            2 => {
                if target_node != self.towers[index].node {
                    return;
                }
                self.launched(index);
                let index2 = self.towers[index].cx;
                let map4 = self.towers[index].map[4];
                let map5 = self.towers[index].map[5];
                let id = self.next_object + 1;
                self.next_object = id;
                self.boulders.push(Boulder {
                    id,
                    tower: tower_id,
                    node: self.towers[index].node,
                    x: index2,
                    born: self.tick,
                    dir: if tcx >= index2 { 1.0 } else { -1.0 },
                    until: self.tick + (25.0 * (1.0 + map4 / 100.0)).round() as u64,
                    left: (5.0 + map5) as i64,
                    hit: Vec::new(),
                    damage,
                    crit_chance: self.towers[index].map[1],
                });
            }
            3 => {
                let wx = self.towers[index].wx;
                let facing = self.towers[index].facing;
                if !((wx > tcx && facing == -1.0) || (facing == 1.0 && tcx > wx)) {
                    return;
                }
                self.launched(index);
                let pc = self.proj_center(index);
                self.effects.push(Effect {
                    at: self.tick,
                    end: self.tick + 9,
                    kind: EffectKind::Ring {
                        x: pc.0,
                        y: pc.1,
                        radius: range - 6.0,
                        anim: 3,
                    },
                });
                for mob_index in 0..self.mobs.len() {
                    if self.mobs[mob_index].dead {
                        continue;
                    }
                    if distance(
                        self.mob_cx(&self.mobs[mob_index]) - pc.0,
                        self.mob_cy(&self.mobs[mob_index]) - pc.1,
                    ) < range - 6.0
                    {
                        let dmg = self.crit(tower_id, damage);
                        self.hit(mob_index, dmg, Some(tower_id));
                        if !self.mobs[mob_index].dead {
                            self.freeze(mob_index, tower_id);
                        }
                    }
                }
            }
            4 => {
                self.launched(index);
                let hp_frac = self.mobs[target].hp / self.mobs[target].max_hp;
                let storm_damage = if hp_frac < 0.6 {
                    damage * (1.0 + map4 / 100.0)
                } else {
                    damage
                };
                self.effects.push(Effect {
                    at: self.tick,
                    end: self.tick + 10,
                    kind: EffectKind::Storm {
                        x: tcx,
                        y: node.y - 55.0,
                        mob: self.mobs[target].id,
                    },
                });
                let target_id = self.mobs[target].id;
                self.at(
                    3,
                    Landing::Storm {
                        target: target_id,
                        damage: storm_damage,
                        tower: tower_id,
                        crit_chance: self.towers[index].map[1],
                        x: tcx,
                        y: node.y - 55.0,
                    },
                );
            }
            5 => {
                self.launched(index);
            }
            7 => {
                self.launched(index);
                let cy = node.y - 24.0;
                let period = 2.0 / (1.0 + self.towers[index].map[5] / 100.0);
                let id = self.next_object + 1;
                self.next_object = id;
                let until =
                    self.tick + ((10.0 + self.towers[index].map[4]) * 10.0).round() as u64 + 8;
                self.clouds.push(Cloud {
                    id,
                    tower: tower_id,
                    node: target_node,
                    x: tcx,
                    y: cy,
                    born: self.tick,
                    until,
                    period,
                    next: self.tick + (period * 10.0).round().max(1.0) as u64,
                    damage,
                });
            }
            8 => {
                self.launched(index);
                let pc = self.proj_center(index);
                self.effects.push(Effect {
                    at: self.tick,
                    end: self.tick + 9,
                    kind: EffectKind::Ring {
                        x: pc.0,
                        y: pc.1,
                        radius: range - 6.0 + 40.0,
                        anim: 7,
                    },
                });
                for mob_index in 0..self.mobs.len() {
                    if self.mobs[mob_index].dead {
                        continue;
                    }
                    if distance(
                        self.mob_cx(&self.mobs[mob_index]) - pc.0,
                        self.mob_cy(&self.mobs[mob_index]) - pc.1,
                    ) < range - 6.0 + 40.0
                    {
                        let dmg = self.crit(tower_id, damage);
                        self.hit(mob_index, dmg, Some(tower_id));
                        self.mobs[mob_index].mods = 0;
                    }
                }
                if self.rng.int(1, 100) < self.towers[index].map[4] as i64 {
                    self.towers[index].cd = self.towers[index].cd_max / 2.0;
                }
            }
            _ => {}
        }
    }

    /// The projectile actor's centre, as the game creates it.
    fn proj_center(&self, index: usize) -> (f64, f64) {
        let tower = &self.towers[index];
        (
            (tower.wx - 19.0).round() + 19.0,
            (tower.wy - 5.0).round() + 19.0,
        )
    }

    fn alive_hp(&self) -> f64 {
        self.mobs
            .iter()
            .filter(|mob| !mob.dead)
            .map(|mob| mob.hp)
            .sum()
    }

    fn spawn(&mut self, row: Wave) {
        let trial = self.trial();
        let hp = (trial.hp_base * hp_multi(row.size, row.skull)).ceil();
        self.pending_hp -= hp;
        let geometry = geometry(trial).expect("the trial's geometry exists");
        let node = &geometry.nodes[trial.path[0]];
        let speed_div = if trial.map == 308 { 1.2 } else { 1.0 };
        let speed =
            trial.mob_def.move_speed * (1.5 - ((row.size - 1.0) / 3.0).powf(0.7)) / speed_div;
        let id = self.next_mob + 1;
        self.next_mob = id;
        let left = if trial.flags[0] == 0 {
            node.x
        } else {
            node.length
        };
        let mid = (node.x + node.length) / 2.0;
        let mut mob = Mob {
            id,
            pi: 0,
            left,
            vx: speed,
            ms: speed,
            base_speed: speed,
            hp,
            max_hp: hp,
            size: row.size,
            status_skull: row.skull.min(16.0),
            mods: row.mods as i64,
            dead: false,
            pers: 0,
            freeze_t: 0.0,
            heal_t: 0.0,
            born: self.tick,
            push: None,
        };
        mob.vx = if mob.left > mid { -speed } else { speed };
        self.mobs.push(mob);
    }

    fn enter_node(&mut self, index: usize) {
        let trial = self.trial();
        let mob = &mut self.mobs[index];
        let node = geometry(trial)
            .and_then(|geom| geom.nodes.get(trial.path[mob.pi]))
            .cloned()
            .expect("a path node exists");
        mob.left = if trial.flags[mob.pi] == 0 {
            node.x + 10.0
        } else {
            node.length - 10.0
        } - trial.mob_def.frame_w / 2.0;
        let mid = (node.x + node.length) / 2.0;
        let magnitude = mob.ms;
        mob.vx = if mob.left > mid {
            -magnitude
        } else {
            magnitude
        };
    }

    fn mob_timers(&mut self) {
        for index in 0..self.mobs.len() {
            let mob = &self.mobs[index];
            if mob.dead {
                continue;
            }
            let age = self.tick - mob.born;
            if age == 0 || !age.is_multiple_of(10) {
                continue;
            }
            if mob.mods == 3 && mob.hp > 0.0 && mob.hp < 0.5 * mob.max_hp {
                self.mobs[index].hp = (self.mobs[index].hp + 0.013 * self.mobs[index].max_hp)
                    .min(self.mobs[index].max_hp);
            } else if mob.mods == 2 && mob.pers < 0 {
                self.mobs[index].pers += 1;
            }
            let mob = &mut self.mobs[index];
            if mob.heal_t > 0.0 && mob.hp > 0.0 {
                mob.hp = (mob.hp + 0.08 * mob.max_hp).min(mob.max_hp).ceil();
            }
            if !age.is_multiple_of(20) {
                continue;
            }
            if mob.heal_t > 0.0 {
                mob.heal_t -= 2.0;
            }
            if mob.freeze_t > 0.0 {
                mob.freeze_t -= 2.0;
                if mob.freeze_t <= 0.0 {
                    mob.freeze_t = 0.0;
                    mob.ms = mob.base_speed;
                    mob.vx = if mob.vx > 0.0 {
                        mob.base_speed
                    } else {
                        -mob.base_speed
                    };
                }
            }
        }
    }

    fn step_once(&mut self) {
        self.record_replay();
        let trial = self.trial();
        if self.tick.is_multiple_of(10) {
            self.countdown = (self.countdown - 1.0).round();
            self.wave_clock = (self.wave_clock + 1.0).min(14400.0);
            if self.wave_clock < self.inputs.slow_roast_pre_cook_sec {
                self.wave_clock = self.inputs.slow_roast_pre_cook_sec.min(14400.0);
            }
            if self.countdown < 0.0 {
                self.wave += 1;
                self.highest_wave = self.highest_wave.max(self.wave);
                self.countdown = super::next_wave_countdown(f64::from(self.wave));
                self.wave_clock = -10.0;
                let row = wave_row(trial, self.wave);
                self.pending_hp +=
                    (row.count * (trial.hp_base * hp_multi(row.size, row.skull)).ceil()).round();
                let interval = (7.0 + 1.3 * (row.count / 5.0).floor()) / row.count.max(1.0);
                for step in 0..row.count as u32 {
                    let at = self.tick + (f64::from(step) * interval * 10.0).round() as u64;
                    self.spawns.push(Spawn { at, wave: row });
                }
                if self.wave > 3 {
                    self.apply_traits();
                }
                if self.wave > self.max_wave {
                    self.end_reason = EndReason::MaxWave;
                    self.finished = true;
                    return;
                }
            }
            let alive = self.mobs.iter().filter(|mob| !mob.dead).count();
            let pending = self.pending_hp + self.alive_hp();
            if self.countdown > 999.0 && pending <= 0.0 {
                self.countdown = 20.0;
            }
            if alive == 0
                && self.spawns.is_empty()
                && pending <= 0.5
                && self.highest_wave > self.best_cleared
            {
                self.best_cleared = self.highest_wave;
                self.best_cleared_tick = Some(self.tick);
            }
            if self.countdown > 999.0 && pending > 0.5 {
                if pending < self.stall_min * (1.0 - 1e-12)
                    || self.pts != self.stall_pts
                    || self.totem != self.stall_totem
                {
                    self.stall_min = self.stall_min.min(pending);
                    self.stall_pts = self.pts;
                    self.stall_totem = self.totem;
                    self.stall_tick = self.tick;
                } else if self.tick - self.stall_tick >= (STALL_SEC * TICKS_PER_SECOND) as u64 {
                    self.end_reason = EndReason::Stalled;
                    self.finished = true;
                    return;
                }
            } else {
                self.stall_min = f64::INFINITY;
                self.stall_tick = self.tick;
            }
        }
        if !self.spawns.is_empty() {
            let due: Vec<Spawn> = self
                .spawns
                .iter()
                .filter(|spawn| spawn.at <= self.tick)
                .cloned()
                .collect();
            self.spawns.retain(|spawn| spawn.at > self.tick);
            for spawn in due {
                self.spawn(spawn.wave);
            }
        }
        self.mob_timers();
        // Mob movement and leaks.
        for index in 0..self.mobs.len() {
            if self.mobs[index].dead {
                continue;
            }
            self.mobs[index].left = match self.mobs[index].push {
                Some(push) => advance_push(push, self.tick),
                None => self.mobs[index].left + self.mobs[index].vx,
            };
            let node_index = self.trial().path[self.mobs[index].pi];
            let node = geometry(self.trial())
                .and_then(|geom| geom.nodes.get(node_index))
                .cloned()
                .expect("a path node exists");
            let center = self.mob_cx(&self.mobs[index]);
            let flag = self.trial().flags[self.mobs[index].pi];
            if (flag == 0 && center > node.length - 20.0) || (flag == 1 && center < node.x + 20.0) {
                self.mobs[index].pi += 1;
                if self.mobs[index].pi >= self.trial().path.len() {
                    self.mobs[index].pi = 0;
                    self.totem -= 1.0;
                    self.totem_damage += 1;
                    self.leaks += 1;
                    if self.totem <= 0.0 {
                        self.end_reason = EndReason::Totem;
                        self.finished = true;
                        break;
                    }
                }
                self.enter_node(index);
            }
        }
        if self.finished {
            return;
        }
        // Towers: cooldown, popup, ready scan.
        for index in 0..self.towers.len() {
            if self.towers[index].cd > 0.0 {
                self.towers[index].cd -= 0.1;
                if self.towers[index].cd <= 0.0 {
                    self.towers[index].state = 1;
                    self.towers[index].ready_at = self.tick + 3;
                }
            }
            if self.towers[index].state == 1 && self.tick >= self.towers[index].ready_at {
                self.towers[index].state = 2;
            }
            if self.towers[index].state == 2 {
                self.fire(index);
            }
        }
        // Delayed landings.
        if !self.landings.is_empty() {
            let due: Vec<Landing> = self
                .landings
                .iter()
                .filter(|timed| timed.at <= self.tick)
                .map(|timed| timed.landing.clone())
                .collect();
            self.landings.retain(|timed| timed.at > self.tick);
            for landing in due {
                self.land(landing);
            }
        }
        // Boulders.
        if !self.boulders.is_empty() {
            let mut boulders = Vec::new();
            for mut boulder in std::mem::take(&mut self.boulders) {
                boulder.x += 5.0 * boulder.dir;
                for index in 0..self.mobs.len() {
                    if boulder.left <= 0 {
                        break;
                    }
                    let mob = &self.mobs[index];
                    if mob.dead
                        || boulder.hit.contains(&mob.id)
                        || self.trial().path[mob.pi] != boulder.node
                    {
                        continue;
                    }
                    if (self.mob_cx(mob) - boulder.x).abs() < 35.0 {
                        boulder.hit.push(mob.id);
                        boulder.left -= 1;
                        let dmg = self.crit_with(boulder.crit_chance, boulder.damage);
                        self.hit(index, dmg, Some(boulder.tower));
                    }
                }
                if boulder.left > 0 && self.tick < boulder.until {
                    boulders.push(boulder);
                }
            }
            self.boulders = boulders;
        }
        // Poison clouds.
        if !self.clouds.is_empty() {
            let mut clouds = Vec::new();
            for mut cloud in std::mem::take(&mut self.clouds) {
                if self.tick >= cloud.next {
                    cloud.next = self.tick + (cloud.period * 10.0).round().max(1.0) as u64;
                    for index in 0..self.mobs.len() {
                        let mob = &self.mobs[index];
                        if mob.dead || self.trial().path[mob.pi] != cloud.node {
                            continue;
                        }
                        if (self.mob_cx(mob) - cloud.x).abs() < 55.0 {
                            self.hit(index, cloud.damage, Some(cloud.tower));
                        }
                    }
                }
                if self.tick < cloud.until {
                    clouds.push(cloud);
                }
            }
            self.clouds = clouds;
        }
        // Kraken underlings.
        if !self.krakens.is_empty() {
            let mut krakens = Vec::new();
            for mut kraken in std::mem::take(&mut self.krakens) {
                let node = geometry(self.trial())
                    .and_then(|geom| geom.nodes.get(kraken.node))
                    .cloned()
                    .expect("an underling's node exists");
                kraken.x = match kraken.push {
                    Some(push) => advance_push(push, self.tick),
                    None => kraken.x + kraken.vx,
                };
                let mut left = kraken.x - 19.0;
                if left + kraken.vx > node.length {
                    kraken.vx = -3.0;
                    kraken.x = node.length - 5.0 + 19.0;
                } else if left + kraken.vx < node.x {
                    kraken.vx = 3.0;
                    kraken.x = node.x + 10.0 + 19.0;
                }
                left = kraken.x - 19.0;
                if self.tick >= kraken.clear_at {
                    kraken.clear_at = self.tick + 20;
                    kraken.target = None;
                    let center = self
                        .towers
                        .iter()
                        .find(|tower| tower.id == kraken.tower)
                        .map(|tower| tower.cx)
                        .unwrap_or(0.0);
                    if left + kraken.vx > center + 140.0 {
                        kraken.vx = -3.0;
                    } else if left + kraken.vx < center - 140.0 {
                        kraken.vx = 3.0;
                    } else {
                        kraken.wander_at = (self.tick + 10) as i64;
                    }
                }
                if kraken.wander_at >= 0 && self.tick >= kraken.wander_at as u64 {
                    kraken.wander_at = -1;
                    if self.rng.int(1, 4) == 1 {
                        kraken.vx = if self.rng.int(1, 2) == 1 { -3.0 } else { 3.0 };
                    } else if self.rng.int(1, 4) == 1 {
                        kraken.vx = 0.0;
                    }
                }
                if self.tick >= kraken.next {
                    kraken.next = self.tick + 2;
                    if kraken.target.is_none() {
                        for mob in &self.mobs {
                            if mob.dead || self.trial().path[mob.pi] != kraken.node {
                                continue;
                            }
                            if distance(kraken.x - self.mob_cx(mob), kraken.y - self.mob_cy(mob))
                                < 250.0
                            {
                                kraken.target = Some(mob.id);
                                break;
                            }
                        }
                    }
                    if let Some(target_id) = kraken.target {
                        let target_index = self.mobs.iter().position(|mob| mob.id == target_id);
                        if let Some(target_index) = target_index {
                            kraken.vx = if left > self.mobs[target_index].left {
                                -3.0
                            } else {
                                3.0
                            };
                        }
                        let mut contact = None;
                        for index in 0..self.mobs.len() {
                            let mob = &self.mobs[index];
                            if mob.dead || self.trial().path[mob.pi] != kraken.node {
                                continue;
                            }
                            if (kraken.x - self.mob_cx(mob)).abs() >= 20.0 {
                                continue;
                            }
                            contact = Some(index);
                            break;
                        }
                        if let Some(index) = contact {
                            let dmg = if kraken.hits > self.rng.int(0, 1000) as f64 / 10.0 {
                                kraken.damage * 1.5
                            } else {
                                kraken.damage
                            };
                            if let Some(target_index) = self
                                .mobs
                                .iter()
                                .position(|mob| Some(mob.id) == kraken.target)
                            {
                                self.hit(target_index, dmg, Some(kraken.tower));
                            }
                            kraken.target = Some(self.mobs[index].id);
                            let size = self.mobs[index].size;
                            let push = Push {
                                from: self.mobs[index].left,
                                dx: kraken.dir * (50.0 / size),
                                start: self.tick,
                            };
                            self.mobs[index].push = Some(push);
                            kraken.push = Some(Push {
                                from: kraken.x,
                                dx: -kraken.dir * (25.0 + 5.0 * size),
                                start: self.tick,
                            });
                            kraken.hits = (kraken.hits - 1.0).round();
                        }
                    }
                }
                if kraken.hits <= 0.0 {
                    if let Some(tower) = self
                        .towers
                        .iter_mut()
                        .find(|tower| tower.id == kraken.tower)
                    {
                        tower.underlings -= 1;
                    }
                } else {
                    krakens.push(kraken);
                }
            }
            self.krakens = krakens;
        }
        if self.tick.is_multiple_of(50) && self.mobs.len() > 64 {
            self.mobs.retain(|mob| !mob.dead);
        }
        if self.tick.is_multiple_of(20) {
            self.effects.retain(|effect| effect.end + 2 >= self.tick);
        }
        self.tick += 1;
        if self.tick >= self.max_ticks {
            self.end_reason = EndReason::Time;
            self.finished = true;
        }
    }

    fn land(&mut self, landing: Landing) {
        match landing {
            Landing::Blast {
                node,
                x,
                radius,
                damage,
                tower,
                dir,
                crit_chance,
            } => {
                let mut hit_count = 0;
                for index in 0..self.mobs.len() {
                    if hit_count >= 5 {
                        break;
                    }
                    let mob = &self.mobs[index];
                    if mob.dead || self.trial().path[mob.pi] != node {
                        continue;
                    }
                    if (self.mob_cx(mob) - x).abs() < radius {
                        let dmg = self.crit_with(crit_chance, damage);
                        self.hit(index, dmg, Some(tower));
                        hit_count += 1;
                    }
                }
                let y = self.geom_node_y(node);
                self.effects.push(Effect {
                    at: self.tick,
                    end: self.tick + 7,
                    kind: EffectKind::Boom { x, y, radius, dir },
                });
            }
            Landing::Storm {
                target,
                damage,
                tower,
                crit_chance,
                x,
                y,
            } => {
                if let Some(index) = self
                    .mobs
                    .iter()
                    .position(|mob| mob.id == target && !mob.dead)
                {
                    let dmg = self.crit_with(crit_chance, damage);
                    self.hit(index, dmg, Some(tower));
                }
                let _ = (x, y);
            }
        }
    }

    /// Advance the run by `ticks` 100 ms steps; false once it has finished.
    pub fn step(&mut self, ticks: u32) -> bool {
        for _ in 0..ticks {
            if self.finished {
                break;
            }
            self.step_once();
        }
        !self.finished
    }

    /// The run's replay timeline, once it has recorded one.
    pub fn replay(&mut self) -> Option<Replay> {
        if self.finished {
            self.finish_replay();
        }
        let recorder = self.replay.as_ref()?;
        Some(recorder.replay(self))
    }

    fn record_replay(&mut self) {
        if let Some(mut recorder) = self.replay.take() {
            recorder.record(self);
            self.replay = Some(recorder);
        }
    }

    fn finish_replay(&mut self) {
        if let Some(mut recorder) = self.replay.take() {
            recorder.finish(self);
            self.replay = Some(recorder);
        }
    }

    /// A snapshot for rendering.
    pub fn view(&self) -> RunView {
        RunView {
            tick: self.tick,
            t: self.tick as f64 / TICKS_PER_SECOND,
            wave: self.wave,
            countdown: self.countdown,
            pts: self.pts,
            totem: self.totem,
            done: self.finished,
            end_reason: self.finished.then_some(self.end_reason),
            mobs: self.mob_views(),
            towers: self.tower_views(),
            boulders: self
                .boulders
                .iter()
                .map(|boulder| BoulderView {
                    id: boulder.id,
                    x: boulder.x,
                    y: self.geom_node_y(boulder.node),
                    dir: boulder.dir,
                    born: boulder.born,
                })
                .collect(),
            clouds: self.cloud_views(),
            krakens: self.kraken_views(),
            effects: self
                .effects
                .iter()
                .filter(|effect| effect.end >= self.tick)
                .cloned()
                .collect(),
        }
    }

    fn mob_views(&self) -> Vec<MobView> {
        self.mobs
            .iter()
            .filter(|mob| !mob.dead)
            .map(|mob| MobView {
                id: mob.id,
                x: self.mob_cx(mob),
                y: self.mob_cy(mob),
                hp_frac: (mob.hp / mob.max_hp).max(0.0),
                size: mob.size,
                mods: mob.mods,
                frozen: mob.freeze_t > 0.0,
                healing: mob.heal_t > 0.0,
                skull: mob.status_skull,
                face_left: mob.vx < 0.0,
                walking: mob.vx != 0.0,
                node: self.trial().path[mob.pi],
                node_y: self.geom_node_y(self.trial().path[mob.pi]),
            })
            .collect()
    }

    fn cloud_views(&self) -> Vec<CloudView> {
        self.clouds
            .iter()
            .map(|cloud| CloudView {
                id: cloud.id,
                x: cloud.x,
                y: cloud.y,
                born: cloud.born,
                until: cloud.until,
            })
            .collect()
    }

    fn kraken_views(&self) -> Vec<KrakenView> {
        self.krakens
            .iter()
            .map(|kraken| KrakenView {
                id: kraken.id,
                x: kraken.x,
                y: kraken.y,
                vx: kraken.vx,
                hits: kraken.hits,
            })
            .collect()
    }

    fn tower_views(&self) -> Vec<TowerView> {
        self.towers
            .iter()
            .map(|tower| self.tower_view(tower))
            .collect()
    }

    fn tower_view(&self, tower: &Tower) -> TowerView {
        TowerView {
            id: tower.id,
            type_index: tower.type_index,
            level: tower.level,
            traits: tower.traits.clone(),
            node: tower.node,
            x: tower.x,
            wx: tower.wx,
            wy: tower.wy,
            cx: tower.cx,
            cy: tower.cy,
            facing: tower.facing,
            state: tower.state,
            cd: tower.cd,
            cd_max: tower.cd_max,
            range: tower_range(
                tower.type_index,
                tower.level,
                self.trial_level(tower.type_index),
            ),
            last_fire: tower.last_fire,
        }
    }

    /// The completed run's totals, computed once.
    pub fn result(&mut self) -> RunResult {
        if self.finished {
            self.finish_replay();
        }
        if let Some(result) = &self.result {
            return result.clone();
        }
        let end_wave = self.highest_wave.max(1);
        let row = wave_row(self.trial(), end_wave);
        let end_wave_hp =
            (row.count * (self.trial().hp_base * hp_multi(row.size, row.skull)).ceil()).max(1.0);
        let pending = (self.pending_hp + self.alive_hp()) / end_wave_hp;
        let result = RunResult {
            end_reason: self.end_reason,
            time_sec: self.tick as f64 / TICKS_PER_SECOND,
            wave_reached: self.wave,
            highest_wave: self.highest_wave,
            best_wave: self.best_cleared,
            best_wave_time_sec: self
                .best_cleared_tick
                .map(|tick| tick as f64 / TICKS_PER_SECOND),
            new_best: self.best_cleared > self.inputs.best_waves[self.trial_index],
            saved_best: self.inputs.best_waves[self.trial_index],
            pts: self.pts,
            spent: self.spent,
            kill_pts: self.kill_pts,
            skip_pts: self.skip_pts,
            kills: self.kills,
            exp: self.exp,
            totem_damage: self.totem_damage,
            totem_left: self.totem,
            leaks: self.leaks,
            end_pending_frac: pending.clamp(0.0, 10.0),
            towers: self
                .towers
                .iter()
                .map(|tower| self.tower_view(tower))
                .collect(),
        };
        if self.finished {
            self.result = Some(result.clone());
        }
        result
    }

    /// The legal spot nearest a requested x on a node, for a palette click.
    pub fn snapped(&self, node: usize, x: f64) -> Option<f64> {
        let existing: Vec<super::Spot> = self
            .towers
            .iter()
            .map(|tower| super::Spot {
                node: tower.node,
                x: tower.x,
            })
            .collect();
        let check = validate_placement(self.trial(), &existing, node, x, 0.0);
        check.ok.then_some(check.x)
    }

    /// The legal x spans on a node, for the slot hint.
    pub fn node_spans(&self, node: usize) -> Vec<(f64, f64)> {
        placement_bands(self.trial())
            .get(node)
            .cloned()
            .unwrap_or_default()
    }
}
