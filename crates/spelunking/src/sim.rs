//! The expedition run: seeded floor generation, digging, elixirs, party
//! switching, exits and the loot a run banks.
//!
//! Ported from `js/stats/systems/w7/spelunk-expedition.js`. Upstream drives the
//! expedition through a 10 ms callback loop with a seeded local generator; this
//! port keeps the tick, the generator, the task queue and the order of every
//! draw, so the same inputs and seed reproduce the same run — which
//! `fixtures/spelunking.json` pins against the JS.
//!
//! The phased Amber policy (`amber-farm`) ports the upstream `amber` object: the
//! full-clear buildup, the deliberate stamina burn from the dump depth, the
//! orange activation at the finishing reserve, the boosted harvest and the rope
//! search. Every phase change is recorded with its reason, so a result explains
//! the policy it ran.

use super::depth::{dig_power, entry_effects, hit_damage, rock_stats, shop, stamina_multiplier};
use super::{Cave, Config, ConfigError, Inputs, Policy, Tool, ToolChoice, data, js_round};
use idleon_common::Control;
use idleon_common::Progress;

/// The upstream `step()` default, and the tick a run advances at.
const TICK_MS: i64 = 10;
/// The 144 tile slots one floor uses: 12 columns of 12 rows.
const SLOTS: usize = 144;
/// The ring lengths the floor generator walks, in slot steps.
const LENGTHS: [i64; 16] = [4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 11];
/// A descent hole: a rock that became the way down.
const EXIT: i64 = -1;
/// A rope the party can leave by.
const ROPE: i64 = -2;
/// A rope a hit has used up.
const ROPE_GONE: i64 = -3;
/// How many events a run keeps; the rest are counted.
const MAX_EVENTS: usize = 2000;
/// The seed the section uses when the caller does not pick one.
pub const DEFAULT_SEED: u32 = 1;
/// How many ticks the run advances between progress reports.
const REPORT_TICKS: usize = 100;

/// The run's options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunOptions {
    /// The generator seed: the same inputs and seed reproduce the result.
    pub seed: u32,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self { seed: DEFAULT_SEED }
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    /// The whole party ran out of stamina and paid the Delve toll.
    Exhausted,
    /// The Amber policy took a rope out before the party ran dry.
    Rope,
    /// No reachable exit and no live rock left.
    Blocked,
    /// The floor generator hit a value outside the supported range.
    Failed,
}

impl Status {
    pub fn id(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Exhausted => "exhausted",
            Status::Rope => "rope",
            Status::Blocked => "blocked",
            Status::Failed => "failed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Running => "Running",
            Status::Exhausted => "Stamina exhausted",
            Status::Rope => "Rope exit",
            Status::Blocked => "No way down",
            Status::Failed => "Run failed",
        }
    }
}

/// One reward line as the Delve toll left it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExitItem {
    pub reward: usize,
    pub gross: f64,
    pub expected: f64,
    /// The fraction the toll leaves, rolled for one extra item above the whole
    /// part. Amber keeps its exact fraction instead.
    pub chance: Option<f64>,
    pub rounded_up: Option<bool>,
    pub collected: f64,
}

/// How a settled run banked its loot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitKind {
    /// The Delve toll: it keeps a share and rolls each fraction.
    Toll,
    /// A rope out: the party keeps everything, Amber at the rope's multiplier.
    Rope,
}

impl ExitKind {
    /// The upstream exit name.
    pub fn id(self) -> &'static str {
        match self {
            ExitKind::Toll => "toll",
            ExitKind::Rope => "rope",
        }
    }
}

/// What the exit did to the loot.
#[derive(Clone, Debug, PartialEq)]
pub struct Exit {
    pub kind: ExitKind,
    /// The share of each reward the toll leaves before the item roll.
    pub retained: f64,
    /// The multiplier a rope applies to Amber; `1` for the toll.
    pub amber_multiplier: f64,
    pub items: Vec<ExitItem>,
}

/// One party member's stamina before and after the run.
#[derive(Clone, Debug, PartialEq)]
pub struct MemberStamina {
    pub id: usize,
    pub start: f64,
    pub end: f64,
    pub max: f64,
}

/// One party member's share of the run.
#[derive(Clone, Debug, PartialEq)]
pub struct Contribution {
    pub id: usize,
    /// The floor the member took over on, or `None` when it never dug.
    pub start_depth: Option<usize>,
    pub digs: f64,
    pub spent: f64,
    pub resisted_stamina: f64,
    pub hits: f64,
    pub misses: f64,
    pub damage: f64,
    pub broken: f64,
    pub descents: f64,
    pub grands: f64,
    pub rewards: [f64; 12],
    /// The member never joined the expedition.
    pub unused: bool,
    /// The member is at zero stamina.
    pub exhausted: bool,
    /// The member dug but never descended.
    pub no_advance: bool,
    /// The run ended on the floor the member joined, without going deeper.
    pub no_further_floor: bool,
    /// The member spent stamina only on resisted hits and never broke a rock.
    pub resist_stall: bool,
}

/// The five phases of the phased Amber policy, in the order they run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmberPhase {
    /// Full-clear buildup with the oranges saved.
    Buildup,
    /// Deliberate stamina burn on rocks this character cannot break.
    Dump,
    /// Drink the available oranges.
    Activate,
    /// Harvest within the remaining stamina budget.
    Harvest,
    /// Take the first untouched rope.
    RopeSearch,
}

impl AmberPhase {
    /// The upstream phase id.
    pub fn id(self) -> &'static str {
        match self {
            AmberPhase::Buildup => "buildup",
            AmberPhase::Dump => "dump",
            AmberPhase::Activate => "activate",
            AmberPhase::Harvest => "harvest",
            AmberPhase::RopeSearch => "rope-search",
        }
    }

    /// The menu label, matching the upstream page.
    pub fn label(self) -> &'static str {
        match self {
            AmberPhase::Buildup => "Full-clear buildup",
            AmberPhase::Dump => "Stamina burn",
            AmberPhase::Activate => "Drink oranges",
            AmberPhase::Harvest => "Boosted harvest",
            AmberPhase::RopeSearch => "Seek untouched rope",
        }
    }
}

/// One phase change the Amber policy made, with the reason the decision read.
#[derive(Clone, Debug, PartialEq)]
pub struct AmberTransition {
    pub phase: AmberPhase,
    pub reason: String,
    /// The in-game depth, one above the zero-based rock depth.
    pub depth: usize,
    /// Stamina spent when the phase changed.
    pub spent: f64,
    pub time: f64,
}

/// One orange the party drank, with the snapshot the effect accumulated from.
#[derive(Clone, Debug, PartialEq)]
pub struct AmberActivation {
    pub depth: usize,
    pub spent: f64,
    pub effect: f64,
    pub time: f64,
    pub slot: usize,
}

/// What the Amber policy did during one run.
#[derive(Clone, Debug, PartialEq)]
pub struct AmberResult {
    /// The phase the run ended in.
    pub phase: AmberPhase,
    pub transitions: Vec<AmberTransition>,
    pub activations: Vec<AmberActivation>,
    /// The native spent-stamina credit the burn digs earned.
    pub dump_spent: f64,
    pub dump_digs: u64,
}

/// The Amber policy state a run advances, which the result reports.
#[derive(Clone, Debug)]
struct AmberFarm {
    phase: AmberPhase,
    transitions: Vec<AmberTransition>,
    activations: Vec<AmberActivation>,
    dump_spent: f64,
    dump_digs: u64,
    activation_depth: Option<usize>,
}

/// The run's per-member breakdown and the flags it raises.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostics {
    pub members: Vec<Contribution>,
    pub resist_stall: bool,
    pub unused: usize,
    pub no_advance: usize,
    pub resisted_stamina: f64,
    pub blocked: bool,
}

/// One line of the run log.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Seconds since the expedition started.
    pub time: f64,
    /// The in-game depth, one above the zero-based rock depth.
    pub depth: usize,
    pub kind: EventKind,
}

/// What a log line records.
#[derive(Clone, Debug, PartialEq)]
pub enum EventKind {
    Floor,
    Dig {
        tool: usize,
        character: usize,
        used: f64,
    },
    Elixir {
        id: usize,
        slot: usize,
    },
    Switch {
        character: usize,
    },
    Boss,
    Discovery {
        name: String,
    },
    GrandDiscovery,
    ExitFound,
    Descend,
    /// The Amber policy changed phase, with the reason the decision read.
    AmberPhase {
        phase: AmberPhase,
        reason: String,
    },
    Exhausted,
    Rope,
    Blocked,
}

impl EventKind {
    /// The upstream event name.
    pub fn id(&self) -> &'static str {
        match self {
            EventKind::Floor => "floor",
            EventKind::Dig { .. } => "dig",
            EventKind::Elixir { .. } => "elixir",
            EventKind::Switch { .. } => "switch",
            EventKind::Boss => "boss",
            EventKind::Discovery { .. } => "discovery",
            EventKind::GrandDiscovery => "grand-discovery",
            EventKind::ExitFound => "exit-found",
            EventKind::Descend => "descend",
            EventKind::AmberPhase { .. } => "amber-phase",
            EventKind::Exhausted => "exhausted",
            EventKind::Rope => "rope",
            EventKind::Blocked => "blocked",
        }
    }
}

/// Everything one finished run reports.
#[derive(Clone, Debug, PartialEq)]
pub struct RunResult {
    pub config: Config,
    pub seed: u32,
    pub status: Status,
    /// Whether the loot was banked; a blocked run keeps nothing.
    pub settled: bool,
    pub seconds: f64,
    /// The floor the run started on, one above the zero-based depth.
    pub start_depth: usize,
    pub depth: usize,
    pub actions: u64,
    pub spent: f64,
    pub hits: u64,
    pub misses: u64,
    pub broken: u64,
    pub rewards: [f64; 12],
    pub gross_rewards: [f64; 12],
    pub exit: Option<Exit>,
    /// The Amber policy's record, when the run ran one.
    pub amber: Option<AmberResult>,
    pub stamina: Vec<MemberStamina>,
    pub diagnostics: Diagnostics,
    /// New Grand Discoveries this run, above the saved count.
    pub grands: f64,
    /// Rock names discovered this run.
    pub discoveries: Vec<String>,
    pub rng_calls: u64,
    pub events: Vec<Event>,
    pub events_dropped: usize,
    pub notes: Vec<String>,
    /// Why the generator stopped, when it hit an unsupported value.
    pub error: Option<String>,
}

/// The upstream `_rng`: a 32-bit mulberry32 stream with a draw counter.
#[derive(Clone, Debug)]
struct Rng {
    state: u32,
    calls: u64,
}

impl Rng {
    fn new(seed: u32) -> Self {
        Self {
            state: seed,
            calls: 0,
        }
    }

    fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        self.calls += 1;
        let mut n = (self.state ^ (self.state >> 15)).wrapping_mul(1 | self.state);
        n ^= n.wrapping_add((n ^ (n >> 7)).wrapping_mul(61 | n));
        f64::from(n ^ (n >> 14)) / 4_294_967_296.0
    }

    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() * (hi - lo + 1) as f64).floor() as i64
    }

    /// The cosmetic draws the game makes on a hit, break, refill or celebration.
    /// They are part of the stream, so they are drawn even though nothing reads
    /// them.
    fn rolls(&mut self, count: usize) {
        for _ in 0..count {
            self.next();
        }
    }
}

/// One object on the current floor.
#[derive(Clone, Debug)]
struct Rock {
    slot: i64,
    /// `0..rocks.len()` for a rock, [`EXIT`], [`ROPE`] or [`ROPE_GONE`].
    kind: i64,
    boss: bool,
    hp: f64,
    requirement: f64,
    amber: f64,
    hidden: bool,
    potion: bool,
    rise_at: Option<i64>,
    hit_at: Option<i64>,
    leave_at: Option<i64>,
    leave_y: f64,
}

/// A delayed callback the tick loop runs.
#[derive(Clone, Debug)]
struct Task {
    remaining: i64,
    kind: TaskKind,
}

#[derive(Clone, Debug)]
enum TaskKind {
    /// A new rock rises into place.
    Rise { slot: i64 },
    /// The old floor's rocks are gone.
    ClearRocks,
    /// The next floor is generated.
    NextFloor,
    /// The floor transition ends.
    EndTransition,
    /// A leaving rock records where it started to fade.
    Leave { slot: i64 },
    /// A used elixir slot becomes ready again.
    ElixirReady { slot: usize },
    /// One burst of the boss celebration's cosmetic draws.
    Celebration,
    /// Cosmetic draws from a delayed burst.
    Visual { draws: usize },
}

/// A delayed effect that ticks down every frame.
#[derive(Clone, Debug)]
enum Pending {
    /// The Grand Discovery delay, which holds the descent.
    Grand { remaining: i64 },
    /// The adjacent-rock strike the yellow elixir schedules.
    Adjacent {
        remaining: i64,
        depth: i64,
        slot: i64,
        damage: f64,
    },
    /// The nova a boss charge or the purple elixir sets off.
    Nova {
        remaining: i64,
        start: i64,
        damage: f64,
        seen: Vec<i64>,
    },
    /// The Shadow Strike the last dig scheduled.
    Shadow {
        remaining: i64,
        depth: i64,
        damage: f64,
        tool: usize,
        x: f64,
        y: f64,
    },
}

impl Pending {
    fn remaining(&self) -> i64 {
        match self {
            Pending::Grand { remaining }
            | Pending::Adjacent { remaining, .. }
            | Pending::Nova { remaining, .. }
            | Pending::Shadow { remaining, .. } => *remaining,
        }
    }

    fn set_remaining(&mut self, value: i64) {
        match self {
            Pending::Grand { remaining }
            | Pending::Adjacent { remaining, .. }
            | Pending::Nova { remaining, .. }
            | Pending::Shadow { remaining, .. } => *remaining = value,
        }
    }
}

/// The radar ping the blue elixir plants, which biases manual targeting.
#[derive(Clone, Copy, Debug)]
struct Radar {
    x: f64,
    y: f64,
    depth: i64,
}

/// Where a rock draws on the floor.
#[derive(Clone, Copy, Debug)]
struct Position {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// One expedition in progress.
pub struct Run<'a> {
    input: &'a Inputs,
    config: Config,
    cave: &'a Cave,
    seed: u32,
    rng: Rng,
    time: i64,
    stamina: Vec<f64>,
    start: Vec<f64>,
    starting_stamina: f64,
    effects: [f64; 13],
    charges: Vec<f64>,
    ready: Vec<f64>,
    tasks: Vec<Task>,
    flags: [f64; 12],
    rewards: [f64; 12],
    reward_order: Vec<usize>,
    events: Vec<Event>,
    event_count: usize,
    discovered: Vec<String>,
    depth: i64,
    start_depth: i64,
    full_count: i64,
    rope_threshold: i64,
    rocks: Vec<Rock>,
    mask: [u8; SLOTS],
    pending: Vec<Pending>,
    transition: bool,
    active: usize,
    found: bool,
    attempted: bool,
    contributions: Vec<Contribution>,
    countdown: f64,
    actions: u64,
    spent: f64,
    hits: u64,
    misses: u64,
    broken: u64,
    tool_id: usize,
    status: Status,
    settled: bool,
    /// The Amber policy's state, `Some` exactly for [`Policy::AmberFarm`].
    amber: Option<AmberFarm>,
    /// The last `finishers` party members, whose stamina the policy preserves.
    finishing_party: Vec<usize>,
    grand_count: f64,
    exalt: f64,
    prisma: f64,
    boss_charged: bool,
    boss_defeated: bool,
    gross_rewards: Option<[f64; 12]>,
    exit: Option<Exit>,
    radar: Option<Radar>,
    error: Option<String>,
}

impl<'a> Run<'a> {
    /// Start an expedition from a normalised configuration.
    pub fn new(
        input: &'a Inputs,
        config: Config,
        options: RunOptions,
    ) -> Result<Run<'a>, ConfigError> {
        let cave = input
            .caves
            .get(config.cave)
            .ok_or_else(|| ConfigError("Invalid cave.".into()))?;
        let stamina: Vec<f64> = input
            .characters
            .iter()
            .map(|character| {
                if config.stamina == super::StaminaMode::Full {
                    character.max_stamina
                } else {
                    character.stamina
                }
            })
            .collect();
        let start = stamina.clone();
        let starting_stamina = config.party.iter().map(|id| start[*id]).sum();
        let effects = entry_effects(input, &config);
        let charges: Vec<f64> = config
            .elixirs
            .iter()
            .map(|id| {
                if *id < 0 {
                    0.0
                } else {
                    input.elixirs.get(*id as usize).map_or(0.0, |e| e.charges)
                }
            })
            .collect();
        let ready = vec![0.0; charges.len()];
        let mut contributions: Vec<Contribution> = config
            .party
            .iter()
            .map(|id| Contribution {
                id: *id,
                start_depth: None,
                digs: 0.0,
                spent: 0.0,
                resisted_stamina: 0.0,
                hits: 0.0,
                misses: 0.0,
                damage: 0.0,
                broken: 0.0,
                descents: 0.0,
                grands: 0.0,
                rewards: [0.0; 12],
                unused: true,
                exhausted: false,
                no_advance: false,
                no_further_floor: false,
                resist_stall: false,
            })
            .collect();
        let active = config.party[0];
        let shortcut = !config.manic && input.shortcut && config.entry == super::Entry::Shortcut;
        let depth = if shortcut {
            let record = input.depths.get(cave.id).copied().unwrap_or(0.0);
            (50.0 * (js_round(record) / 50.0).floor() - 1.0).max(0.0) as i64
        } else {
            0
        };
        contributions[0].start_depth = Some(depth as usize + 1);
        let amber = config.amber.map(|_| AmberFarm {
            phase: AmberPhase::Buildup,
            transitions: Vec::new(),
            activations: Vec::new(),
            dump_spent: 0.0,
            dump_digs: 0,
            activation_depth: None,
        });
        let finishing_party = config.amber.map_or_else(Vec::new, |policy| {
            let count = (policy.finishers as usize).max(1);
            config.party[config.party.len().saturating_sub(count)..].to_vec()
        });
        let mut run = Run {
            input,
            cave,
            seed: options.seed,
            rng: Rng::new(options.seed),
            time: 0,
            stamina,
            start,
            starting_stamina,
            effects,
            charges,
            ready,
            tasks: Vec::new(),
            flags: [0.0; 12],
            rewards: [0.0; 12],
            reward_order: Vec::new(),
            events: Vec::new(),
            event_count: 0,
            discovered: input.discoveries.clone(),
            depth,
            start_depth: depth,
            full_count: depth,
            rope_threshold: if depth > 0 { depth + 5 } else { 4 },
            rocks: Vec::new(),
            mask: [0; SLOTS],
            pending: Vec::new(),
            transition: true,
            active,
            found: false,
            attempted: false,
            contributions,
            countdown: 0.0,
            actions: 0,
            spent: 0.0,
            hits: 0,
            misses: 0,
            broken: 0,
            tool_id: input.tools.last().map_or(0, |tool| tool.id),
            status: Status::Running,
            settled: false,
            amber,
            finishing_party,
            grand_count: input.grands.get(cave.id).copied().unwrap_or(0.0),
            exalt: 0.0,
            prisma: 0.0,
            boss_charged: false,
            boss_defeated: input.bosses.get(cave.id).is_some_and(|won| *won != 0.0),
            gross_rewards: None,
            exit: None,
            radar: None,
            error: None,
            config,
        };
        run.tasks.push(Task {
            remaining: 930,
            kind: TaskKind::EndTransition,
        });
        run.generate();
        Ok(run)
    }

    /// Whether the run has stopped.
    pub fn done(&self) -> bool {
        self.status != Status::Running
    }

    /// Seconds since the expedition started.
    pub fn seconds(&self) -> f64 {
        self.time as f64 / 100.0
    }

    /// The share of the party's starting stamina the run has spent.
    pub fn progress(&self) -> Progress {
        let fraction = if self.done() {
            1.0
        } else {
            let remaining: f64 = self.config.party.iter().map(|id| self.stamina[*id]).sum();
            (1.0 - remaining / self.starting_stamina.max(f64::MIN_POSITIVE)).clamp(0.0, 0.99)
        };
        Progress {
            fraction: fraction as f32,
            message: format!(
                "Depth {} · {}% stamina spent",
                self.depth + 1,
                js_round(100.0 * fraction) as i64
            ),
        }
    }

    /// Advance the tick loop `count` frames.
    pub fn step(&mut self, count: usize) {
        for _ in 0..count {
            if self.status != Status::Running {
                return;
            }
            self.tick();
        }
    }

    /// The finished run.
    pub fn result(&self) -> RunResult {
        let exhausted = self.status == Status::Exhausted;
        let members: Vec<Contribution> = self
            .contributions
            .iter()
            .map(|member| {
                let stamina = self.stamina[member.id];
                Contribution {
                    unused: member.start_depth.is_none(),
                    exhausted: member.start_depth.is_some() && stamina <= 0.0,
                    no_advance: member.start_depth.is_some() && member.descents == 0.0,
                    no_further_floor: exhausted
                        && member.start_depth.is_some()
                        && self.depth < member.start_depth.unwrap_or(0) as i64,
                    resist_stall: self.status != Status::Running
                        && member.start_depth.is_some()
                        && stamina <= 0.0
                        && member.digs > 0.0
                        && member.misses > 0.0
                        && member.broken == 0.0
                        && member.descents == 0.0,
                    ..member.clone()
                }
            })
            .collect();
        RunResult {
            config: self.config.clone(),
            seed: self.seed,
            status: self.status,
            settled: self.settled,
            seconds: self.seconds(),
            start_depth: self.start_depth as usize + 1,
            depth: self.depth as usize + 1,
            actions: self.actions,
            spent: self.spent,
            hits: self.hits,
            misses: self.misses,
            broken: self.broken,
            rewards: self.rewards,
            gross_rewards: self.gross_rewards.unwrap_or(self.rewards),
            exit: self.exit.clone(),
            amber: self.amber.as_ref().map(|amber| AmberResult {
                phase: amber.phase,
                transitions: amber.transitions.clone(),
                activations: amber.activations.clone(),
                dump_spent: amber.dump_spent,
                dump_digs: amber.dump_digs,
            }),
            stamina: self
                .config
                .party
                .iter()
                .map(|id| MemberStamina {
                    id: *id,
                    start: self.start[*id],
                    end: self.stamina[*id],
                    max: self.input.characters[*id].max_stamina,
                })
                .collect(),
            diagnostics: Diagnostics {
                resist_stall: members.iter().any(|member| member.resist_stall),
                unused: members.iter().filter(|member| member.unused).count(),
                no_advance: members.iter().filter(|member| member.no_advance).count(),
                resisted_stamina: members.iter().map(|member| member.resisted_stamina).sum(),
                blocked: self.status == Status::Blocked,
                members,
            },
            grands: self.grand_count - self.input.grands.get(self.cave.id).copied().unwrap_or(0.0),
            discoveries: self
                .discovered
                .iter()
                .filter(|name| !self.input.discoveries.contains(name))
                .cloned()
                .collect(),
            rng_calls: self.rng.calls,
            events: self.events.clone(),
            events_dropped: self.event_count - self.events.len(),
            notes: notes(self.input, &self.config),
            error: self.error.clone(),
        }
    }

    /// The party slot of the active member.
    fn active_slot(&self) -> usize {
        self.config
            .party
            .iter()
            .position(|id| *id == self.active)
            .unwrap_or(0)
    }

    fn shop(&self, index: usize) -> f64 {
        shop(self.input, index)
    }

    fn quantity(&self, elixir: usize) -> f64 {
        self.input
            .elixirs
            .get(elixir)
            .map_or(0.0, |elixir| elixir.quantity)
    }

    fn power(&self) -> f64 {
        dig_power(self.input, &self.effects)
    }

    fn cost_multi(&self) -> f64 {
        stamina_multiplier(&self.config, &self.effects)
    }

    fn amber_boost(&self) -> f64 {
        if self.effects[3] > 0.5 {
            1.0 + 0.1 * self.quantity(3) * self.effects[3] / 100.0
        } else {
            1.0
        }
    }

    /// The share of the first finisher's starting stamina the policy preserves.
    fn amber_reserve(&self) -> f64 {
        self.config
            .amber
            .map_or(0.0, |amber| amber.reserve_pct as f64 / 100.0)
    }

    /// Whether the finishing party's turn has come, or the first finisher is at
    /// the reserve.
    fn at_amber_reserve(&self) -> bool {
        let Some(first) = self.finishing_party.first().copied() else {
            return false;
        };
        let Some(first_index) = self.config.party.iter().position(|id| *id == first) else {
            return false;
        };
        let index = self.active_slot();
        index > first_index
            || (index == first_index
                && self.stamina[self.active] <= self.start[self.active] * self.amber_reserve())
    }

    /// Record one Amber phase change, with the reason the decision read.
    fn amber_phase(&mut self, phase: AmberPhase, reason: &str) {
        let depth = self.depth as usize + 1;
        let spent = self.spent;
        let time = self.seconds();
        let Some(amber) = self.amber.as_mut() else {
            return;
        };
        if amber.phase == phase {
            return;
        }
        amber.phase = phase;
        amber.transitions.push(AmberTransition {
            phase,
            reason: reason.to_string(),
            depth,
            spent,
            time,
        });
        self.record(EventKind::AmberPhase {
            phase,
            reason: reason.to_string(),
        });
    }

    /// Leave through the first untouched rope once the harvest target or the
    /// finishing reserve says to seek one.
    fn try_amber_rope(&mut self) -> bool {
        let Some(policy) = self.config.amber else {
            return false;
        };
        let Some(phase) = self.amber.as_ref().map(|amber| amber.phase) else {
            return false;
        };
        if !matches!(phase, AmberPhase::Harvest | AmberPhase::RopeSearch) {
            return false;
        }
        let remaining: f64 = self.config.party.iter().map(|id| self.stamina[*id]).sum();
        let reserve: f64 = self
            .finishing_party
            .iter()
            .map(|id| self.start[*id])
            .sum::<f64>()
            * policy.rope_reserve_pct as f64
            / 100.0;
        let activation_depth = self
            .amber
            .as_ref()
            .and_then(|amber| amber.activation_depth)
            .unwrap_or(0) as i64;
        if self.depth + 1 >= activation_depth + policy.harvest_depths as i64 || remaining <= reserve
        {
            let reason = if remaining <= reserve {
                "Low finishing stamina"
            } else {
                "Harvest depth target reached"
            };
            self.amber_phase(AmberPhase::RopeSearch, reason);
        }
        if self.amber.as_ref().map(|amber| amber.phase) != Some(AmberPhase::RopeSearch) {
            return false;
        }
        if !self.rocks.iter().any(|rock| rock.kind == ROPE) {
            return false;
        }
        self.settle(Status::Rope);
        true
    }

    /// What a dig costs after the refund elixir, as the harvest budget reads it.
    fn harvest_net(&self, tool: &Tool) -> f64 {
        let refund = if self.effects[0] > 0.0 {
            1.0 - self.quantity(2) / 100.0
        } else {
            1.0
        };
        tool.cost * self.cost_multi() * refund
    }

    /// What a burn dig costs after the refund elixir, as the dump search reads it.
    fn burn_net(&self, tool: &Tool) -> f64 {
        let cost = tool.cost * self.cost_multi();
        let refund = if self.effects[0] > 0.0 {
            cost * self.quantity(2) / 100.0
        } else {
            0.0
        };
        (cost - refund).max(0.0)
    }

    /// Whether the party's remaining stamina budget can still break this rock.
    fn can_harvest(&self, rock: &Rock, tool: &Tool, remaining: f64) -> bool {
        let ratio = self.power() / rock.requirement;
        let chance = ratio.powf(0.6).min(1.0);
        let damage = hit_damage(self.input, tool.damage, ratio);
        chance > 0.0
            && damage > 0.0
            && (rock.hp / damage).ceil() / chance * self.harvest_net(tool).max(0.0) <= remaining
    }

    fn record(&mut self, kind: EventKind) {
        self.event_count += 1;
        if self.events.len() < MAX_EVENTS {
            self.events.push(Event {
                time: self.seconds(),
                depth: self.depth as usize + 1,
                kind,
            });
        }
    }

    fn fail(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
        self.status = Status::Failed;
    }

    /// `generate`: build the next floor's mask, rocks, exits and potion.
    fn generate(&mut self) {
        self.flags[1] = 1.0;
        self.flags[8] = 1.0;
        for slot in 0..SLOTS {
            if self.mask[slot] == 1 && self.rng.next() < 0.007 {
                self.flags[5] = 1.0;
            }
        }
        self.mask = [0; SLOTS];
        for n in 0..16i64 {
            self.mask[(52 + n + 8 * (n / 4)) as usize] = 1;
        }
        let cave_id = self.cave.id as i64;
        let scale = 6.0 * (cave_id + 2 + cave_id / 2) as f64;
        let mut cursor: i64 = 40;
        for ring in 0..LENGTHS.len() {
            for _ in 0..LENGTHS[ring] {
                let chance = (0.97 - 0.02 * ring as f64)
                    * ((self.depth + 2) as f64 / (self.depth as f64 + scale));
                let roll = self.rng.next();
                let adjacent = neighbors(cursor)
                    .into_iter()
                    .any(|delta| self.mask_at(cursor + delta) == 1);
                if roll < chance && adjacent {
                    self.mask[cursor as usize] = 1;
                }
                cursor += [1, 12, -1, -12][ring % 4];
            }
        }
        let mut rope = -1;
        let mut candidates = 0;
        for row in 2..23i64 {
            if rope != -1 {
                break;
            }
            let count = js_round(row as f64 - (2 * (row - 11)).max(0) as f64 + 1.0) as i64;
            for j in 0..count {
                let slot = 12 * row - 11 * j;
                if slot % 13 != 0 && self.mask_at(slot) == 1 {
                    let roll = self.rng.next();
                    let wanted = roll < 0.1 + 0.05 * j as f64 || candidates == 7;
                    if wanted && self.depth > 6 && self.depth > self.rope_threshold {
                        rope = slot;
                    } else {
                        candidates += 1;
                    }
                }
            }
        }
        let mut boss_slots: Vec<i64> = Vec::new();
        if self.depth as f64 == self.cave.boss_depth {
            let mut anchors = Vec::new();
            for slot in 0..132i64 {
                if slot % 12 < 10 && (0..6).all(|k| self.mask_at(slot + k + 8 * (k / 4)) == 1) {
                    anchors.push(slot);
                }
            }
            // The generator always seeds a 16-tile block, so an anchor exists;
            // a floor without one simply carries no boss.
            if !anchors.is_empty() {
                let anchor = anchors[self.rng.int(0, anchors.len() as i64 - 1) as usize];
                boss_slots = vec![
                    anchor,
                    anchor + 1,
                    anchor + 2,
                    anchor + 12,
                    anchor + 13,
                    anchor + 14,
                ];
            }
        }
        self.rocks.clear();
        for slot in 0..SLOTS {
            let slot = slot as i64;
            if self.mask[slot as usize] != 1 || boss_slots.contains(&slot) {
                continue;
            }
            if slot == rope && self.rng.next() < 0.25 && self.depth != 0 {
                self.add_rock(slot, ROPE, false);
                continue;
            }
            let mut kind = 0usize;
            for n in 1..self.cave.rocks.len().saturating_sub(1) {
                let threshold = self.cave.rocks[n].depth;
                if self.depth as f64 >= threshold {
                    let span = self.depth as f64 - threshold;
                    let chance = span / (span + 2.0 * threshold).max(1.0) * 1.5;
                    if self.rng.next() >= chance {
                        break;
                    }
                    kind = n;
                }
            }
            self.add_rock(slot, kind as i64, false);
        }
        if let Some(anchor) = boss_slots.first() {
            let boss = self.cave.rocks.len() as i64 - 1;
            self.add_rock(*anchor, boss, true);
        }
        let exits = (1.0 + self.rng.next() + (self.shop(18) + self.input.holes_bonus) / 100.0)
            .clamp(1.0, 3.0)
            .floor() as i64;
        if (self
            .rocks
            .iter()
            .filter(|rock| rock.kind >= 0 && !rock.boss)
            .count() as i64)
            < exits
        {
            self.fail("Too few rocks for native exit placement.");
            return;
        }
        for _ in 0..exits {
            if let Some(index) = self.choose(|rock| rock.kind >= 0 && !rock.boss && !rock.hidden) {
                self.rocks[index].hidden = true;
            }
        }
        if (self.depth + 1) % 5 == 0
            && self.shop(36) >= 1.0
            && self.config.elixirs.iter().any(|id| *id >= 0)
            && let Some(index) = self.choose(|rock| rock.kind >= 0 && !rock.boss && !rock.hidden)
        {
            self.rocks[index].potion = true;
        }
        self.found = false;
        if self.config.policy == Policy::Auto {
            self.tool_id = self
                .input
                .tools
                .iter()
                .rfind(|tool| self.input.overuse || self.stamina[self.active] >= tool.cost)
                .map_or(0, |tool| tool.id);
        }
        self.record(EventKind::Floor);
    }

    fn mask_at(&self, slot: i64) -> u8 {
        usize::try_from(slot)
            .ok()
            .and_then(|slot| self.mask.get(slot).copied())
            .unwrap_or(0)
    }

    /// `choose`: rejection-sample one rock that passes `test`.
    fn choose(&mut self, test: impl Fn(&Rock) -> bool) -> Option<usize> {
        if self.rocks.is_empty() {
            return None;
        }
        loop {
            let index = self.rng.int(0, self.rocks.len() as i64 - 1) as usize;
            if test(&self.rocks[index]) {
                return Some(index);
            }
        }
    }

    fn add_rock(&mut self, slot: i64, kind: i64, boss: bool) {
        let (hp, requirement, amber) = if kind < 0 {
            (1.0, 0.0, 0.0)
        } else {
            rock_stats(self.cave, kind as usize, self.depth)
        };
        if !hp.is_finite() || hp <= 0.0 || !requirement.is_finite() || !amber.is_finite() {
            self.fail("Rock values exceed the supported numeric range.");
            return;
        }
        self.rocks.push(Rock {
            slot,
            kind,
            boss,
            hp,
            requirement,
            amber,
            hidden: false,
            potion: false,
            rise_at: None,
            hit_at: None,
            leave_at: None,
            leave_y: 0.0,
        });
        self.tasks.push(Task {
            remaining: 4 * slot,
            kind: TaskKind::Rise { slot },
        });
    }

    /// `refill`: a potion or boss charges a random equipped elixir slot.
    fn refill(&mut self) {
        let mut slot = self.rng.int(0, 4) as usize;
        let mut tries = 0;
        while tries < 50 && self.config.elixirs.get(slot).copied().unwrap_or(-1) < 0 {
            slot = self.rng.int(0, 4) as usize;
            tries += 1;
        }
        if self.config.elixirs.get(slot).copied().unwrap_or(-1) >= 0
            && let Some(charges) = self.charges.get_mut(slot)
        {
            *charges += 1.0;
            self.rng.rolls(6);
        }
    }

    /// `position`: where a rock draws, which is also what the hit radius reads.
    fn position(&self, index: usize) -> Position {
        let rock = &self.rocks[index];
        let (grid_x, grid_y) = grid(rock.slot);
        let art = sprite(self.cave, rock);
        let dy = rock.rise_at.map_or(0.0, |rise| {
            tween(
                0.0,
                -12.0,
                (self.time - rise) as f64 * 10.0,
                300.0,
                quad_out,
            )
        });
        let (sx, sy) = match rock.hit_at {
            None => (1.0, 1.0),
            Some(hit) => (
                tween(
                    1.32,
                    1.0,
                    (self.time - hit) as f64 * 10.0,
                    800.0,
                    elastic_out,
                ),
                tween(
                    0.82,
                    1.0,
                    (self.time - hit) as f64 * 10.0,
                    800.0,
                    elastic_out,
                ),
            ),
        };
        let boss_x = if rock.boss {
            20.0 + self.cave.boss_offset[0]
        } else {
            0.0
        };
        let boss_y = if rock.boss {
            40.0 + self.cave.boss_offset[1]
        } else {
            0.0
        };
        let x = js_round(grid_x - art.0 / 2.0 + boss_x) + js_round(art.0 / 2.0);
        let y = match rock.leave_at {
            Some(leave) => tween(
                rock.leave_y,
                rock.leave_y - 16.0,
                (self.time - leave) as f64 * 10.0,
                300.0,
                quad_out,
            ),
            None => {
                let offset = if rock.kind == EXIT { 10.0 } else { 19.0 };
                js_round(grid_y + offset - art.1 + boss_y) + art.1 + dy
            }
        };
        Position {
            x,
            y,
            w: art.0 * sx,
            h: art.1 * sy,
        }
    }

    /// `within`: whether a dig at `(x, y)` reaches the rock.
    fn within(&self, index: usize, x: f64, y: f64, radius: f64) -> bool {
        let position = self.position(index);
        distance(position.x, position.y - position.h / 2.0, x, y) <= radius
    }

    /// `hit`: one tool swing against one rock.
    fn hit(&mut self, index: usize, damage: f64) {
        if self.rocks[index].kind < 0 || self.transition {
            return;
        }
        self.attempted = true;
        let mut cut = false;
        for rock in &mut self.rocks {
            if rock.kind == ROPE {
                rock.kind = ROPE_GONE;
                cut = true;
            }
        }
        if cut {
            self.rng.rolls(15 * 6);
        }
        let ratio = self.power() / self.rocks[index].requirement;
        if self.rng.next() > ratio.powf(0.6) {
            self.misses += 1;
            self.flags[4] = 1.0;
            let slot = self.active_slot();
            self.contributions[slot].misses += 1.0;
            return;
        }
        self.hits += 1;
        let slot = self.active_slot();
        self.contributions[slot].hits += 1.0;
        self.rocks[index].hit_at = Some(self.time);
        let dealt = hit_damage(self.input, damage, ratio);
        self.contributions[slot].damage += self.rocks[index].hp.max(0.0).min(dealt);
        self.rocks[index].hp -= dealt;
    }

    /// `destroy`: every broken rock drops its loot and may reveal the exit.
    fn destroy(&mut self) {
        let mut index = 0;
        while index < self.rocks.len() {
            if self.status != Status::Running {
                return;
            }
            let rock = &self.rocks[index];
            if rock.kind < 0 || rock.hp > 0.0 {
                index += 1;
                continue;
            }
            let slot = rock.slot;
            let kind = rock.kind;
            let boss = rock.boss;
            let hidden = rock.hidden;
            let potion = rock.potion;
            let amber = rock.amber;
            self.broken += 1;
            let active = self.active_slot();
            self.contributions[active].broken += 1.0;
            self.rng.rolls(2 * 5);
            let mut drops: Vec<(usize, f64)> = Vec::new();
            if self.rng.next()
                < ((self.shop(7) + self.shop(52)) / 100.0 / (1.0 + 9.0 * self.shop(67))).min(0.8)
            {
                let amount = self.input.amber
                    * self.amber_boost()
                    * (1.0 + 10.0 * self.effects[12] / 100.0)
                    * (1.0 + self.shop(21) * (self.depth + 1) as f64 / 150.0)
                    * (1.0 + self.shop(35) * self.full_count as f64 / 100.0)
                    * amber;
                self.drop(0, amount, &mut drops);
                let extra = (self.rng.next()
                    + (5.0 + self.shop(42) + 25.0 * self.effects[6])
                        / 100.0
                        / (1.0 + 19.0 * self.shop(67)))
                .floor()
                .max(0.0);
                for _ in 0..extra as usize {
                    self.drop(0, amount, &mut drops);
                }
            }
            if self.rng.next()
                < (1.0 + self.shop(28) / 100.0)
                    / (100.0 * 1.5f64.powf(self.input.prisma + self.prisma))
                && self.shop(28) > 0.0
            {
                self.drop(1, 1.0, &mut drops);
                self.prisma = js_round(self.exalt + 1.0);
            }
            if self.rng.next()
                < (1.0 + self.shop(27) / 100.0) / (60.0 * 8f64.powf(self.input.exalt + self.exalt))
                && self.shop(27) > 0.0
            {
                self.drop(2, 1.0, &mut drops);
                self.exalt += 1.0;
            }
            if self.rng.next() < 0.002 * (1.0 + self.shop(29) / 100.0) && self.shop(29) > 0.0 {
                if self.rng.next() < 0.6 {
                    self.drop(3, 1.0, &mut drops);
                } else if self.rng.next() < 0.04 {
                    self.drop(4, 1.0, &mut drops);
                } else {
                    self.rng.next();
                    self.drop(5, 1.0, &mut drops);
                }
            }
            if self.rng.next() < 1e-4 * (1.0 + self.shop(30) / 100.0) && self.shop(30) > 0.0 {
                self.drop(7, 1.0, &mut drops);
            }
            if self.rng.next() < self.depth as f64 / (self.depth as f64 + 40.0) * 0.2
                && [0, 2, 4, 6].contains(&self.cave.id)
            {
                let quantity =
                    ((self.depth - 10).max(0) as f64).powi(2) + 2.0 * self.depth as f64 + 1.0;
                self.drop(8 + self.cave.id / 2, quantity.clamp(1.0, 1e6), &mut drops);
            }
            let bags = if self.input.bags == 0.0 {
                0.0
            } else {
                (0.025 - self.depth as f64 / (self.depth as f64 + 50.0) * 0.014)
                    * (1.0 + 50.0 * (self.input.bags - 1.0) / 100.0).max(1.0)
            };
            if self.rng.next() < bags {
                self.effects[12] += 1.0;
                self.drop_visual();
            }
            for _ in 0..drops.len() {
                self.drop_visual();
            }
            if potion {
                self.refill();
            }
            if boss && !self.boss_charged {
                // `n < shop[37]` runs once more than the whole part when the
                // shop bonus is fractional.
                for _ in 0..(self.shop(37).max(0.0).ceil() as usize) {
                    self.refill();
                }
                self.boss_charged = true;
                self.record(EventKind::Boss);
            }
            if boss && !self.boss_defeated {
                self.boss_defeated = true;
                self.celebrate();
            }
            if !boss && kind >= 0 {
                let name = self
                    .cave
                    .rocks
                    .get(kind as usize)
                    .map(|rock| rock.name.clone())
                    .unwrap_or_default();
                if !self.discovered.contains(&name) {
                    self.discovered.push(name.clone());
                    self.record(EventKind::Discovery { name });
                } else if kind as usize == self.cave.rocks.len().saturating_sub(2)
                    && self.shop(43) >= 1.0
                    && self.grand_count < 10.0
                    && self.rng.next()
                        < self.input.grand / 2f64.powf(self.grand_count)
                            * (1.0 + (4.0 * self.effects[9]).min(4.0))
                {
                    self.grand_count += 1.0;
                    self.pending.push(Pending::Grand { remaining: 12_000 });
                    self.record(EventKind::GrandDiscovery);
                    self.contributions[active].grands += 1.0;
                }
            }
            if self.effects[5] > 0.0 {
                let quantity = self.quantity(5);
                for (n, delta) in neighbors(slot).into_iter().enumerate() {
                    if self.rng.next() < quantity / 100.0 {
                        self.pending.push(Pending::Adjacent {
                            remaining: 202 + 5 * n as i64,
                            depth: self.depth,
                            slot: slot + delta,
                            damage: self.input.tools.first().map_or(0.0, |tool| tool.damage),
                        });
                    }
                }
            }
            if hidden && !self.found {
                let rock = &mut self.rocks[index];
                rock.kind = EXIT;
                rock.hp = 1.0;
                rock.rise_at = None;
                rock.hit_at = None;
                self.found = true;
                if self.config.policy == Policy::Auto {
                    self.countdown = 130.0;
                }
                self.record(EventKind::ExitFound);
                index += 1;
            } else {
                self.rocks.remove(index);
            }
        }
    }

    /// The six cosmetic draws a drop makes, which stay in the stream.
    fn drop_visual(&mut self) {
        self.rng.int(12, 16);
        self.rng.int(1, 2);
        self.rng.int(8, 12);
        self.rng.int(1, 2);
        self.rng.next();
        self.rng.next();
    }

    /// The boss celebration's cosmetic draws.
    fn celebrate(&mut self) {
        for group in 0..7 {
            self.tasks.push(Task {
                remaining: 300 * group,
                kind: TaskKind::Celebration,
            });
        }
    }

    fn drop(&mut self, reward: usize, quantity: f64, drops: &mut Vec<(usize, f64)>) {
        if reward >= self.rewards.len() {
            return;
        }
        if !self.reward_order.contains(&reward) {
            self.reward_order.push(reward);
        }
        self.rewards[reward] += quantity;
        let slot = self.active_slot();
        self.contributions[slot].rewards[reward] += quantity;
        drops.push((reward, quantity));
    }

    /// `nova`: a rope-threshold bump and a delayed area hit.
    fn nova(&mut self) {
        self.rope_threshold += 4;
        let tool = if self.shop(40) >= 1.0 {
            self.input.tools.last().map(|tool| tool.id)
        } else {
            Some(self.tool_id)
        };
        let damage = tool
            .and_then(|id| self.input.tools.iter().find(|tool| tool.id == id))
            .map_or(0.0, |tool| tool.damage);
        self.pending.push(Pending::Nova {
            remaining: 600,
            start: self.time,
            damage,
            seen: Vec::new(),
        });
    }

    /// `useElixir`: spend one belt slot's charge.
    fn use_elixir(&mut self, slot: usize) -> bool {
        let Some(id) = self.config.elixirs.get(slot).copied() else {
            return false;
        };
        let Some(charges) = self.charges.get(slot).copied() else {
            return false;
        };
        let ready = self.ready.get(slot).copied().unwrap_or(0.0);
        if id < 0
            || charges <= 0.0
            || ready != 0.0
            || ([0, 2].contains(&id) && self.effects[id as usize] >= 1.0)
        {
            return false;
        }
        if self.rng.next() >= self.input.nonconsume {
            self.charges[slot] -= 1.0;
        }
        if self.rng.next() < self.shop(39) / 100.0 {
            self.nova();
        }
        match id {
            0 | 2 | 4 | 9 => self.effects[id as usize] = 1.0,
            3 => {
                self.effects[3] = js_round(self.effects[3] + self.spent);
                let activation = AmberActivation {
                    depth: self.depth as usize + 1,
                    spent: self.spent,
                    effect: self.effects[3],
                    time: self.seconds(),
                    slot,
                };
                if let Some(amber) = self.amber.as_mut() {
                    amber.activations.push(activation);
                }
            }
            5 => self.effects[5] += 2.0,
            6 | 7 | 10 => self.effects[id as usize] += 1.0,
            8 => self.nova(),
            1 => {
                if let Some(target) = self.rocks.iter().find(|rock| rock.hidden) {
                    let angle = self.rng.int(0, 359) as f64 * std::f64::consts::PI / 180.0;
                    let radius = self.rng.int(0, 150) as f64;
                    let (grid_x, grid_y) = grid(target.slot);
                    self.radar = Some(Radar {
                        x: js_round(grid_x + angle.cos() * radius),
                        y: js_round(grid_y + angle.sin() * radius),
                        depth: self.depth,
                    });
                }
            }
            _ => {}
        }
        if self.charges[slot] <= 0.0 {
            self.ready[slot] = 2.0;
        }
        let draws = 7 * self.rng.int(4, 5) as usize;
        self.rng.rolls(draws);
        self.actions += 1;
        self.record(EventKind::Elixir {
            id: id as usize,
            slot,
        });
        true
    }

    /// `chooseElixir`: whether the party drinks one charge before digging.
    fn choose_elixir(&mut self) -> bool {
        let manual = self.config.policy != Policy::Auto;
        // The Amber policy drinks its double-Amber oranges ahead of everything.
        if self.amber.is_some() {
            for slot in 0..self.charges.len() {
                if self.config.elixirs.get(slot) == Some(&6) && self.use_elixir(slot) {
                    return true;
                }
            }
        }
        let party_max: f64 = (0..self.config.party.len())
            .map(|index| {
                self.input
                    .characters
                    .get(index)
                    .map_or(0.0, |c| c.max_stamina)
            })
            .sum();
        for slot in 0..self.charges.len() {
            let Some(id) = self.config.elixirs.get(slot).copied() else {
                continue;
            };
            if id < 0 || self.charges[slot] <= 0.0 || self.ready[slot] != 0.0 {
                continue;
            }
            let mut use_it = (id == 0 && self.effects[0] == 0.0)
                || (id == 2 && self.effects[2] == 0.0)
                || [6, 7].contains(&id);
            if id == 1 && self.flags[1] != 0.0 {
                use_it = true;
                self.flags[1] = 0.0;
            }
            if id == 3 {
                use_it = match self.amber.as_ref().map(|amber| amber.phase) {
                    Some(phase) => matches!(
                        phase,
                        AmberPhase::Activate | AmberPhase::Harvest | AmberPhase::RopeSearch
                    ),
                    None => self.spent > 0.6 * party_max,
                };
            }
            if id == 4 {
                use_it = self.flags[4] == 1.0
                    && (self.effects[4] == 0.0
                        || 5.0 / (1.0 + self.quantity(4) / 100.0).powf(self.effects[4] - 1.0)
                            < 1.0);
            }
            if id == 5 && self.flags[5] != 0.0 {
                use_it = true;
                self.flags[5] = 0.0;
            }
            if id == 8 && self.flags[8] != 0.0 && self.effects[5] == 0.0 {
                use_it = true;
                self.flags[8] = 0.0;
            }
            if manual && id == 9 {
                use_it = self.effects[9] == 0.0;
            }
            if manual && id == 10 {
                use_it = self.flags[4] == 1.0 && self.effects[10] == 0.0;
            }
            if use_it && self.use_elixir(slot) {
                return true;
            }
        }
        false
    }

    /// `dig`: one tool swing at a floor position.
    fn dig(&mut self, x: f64, y: f64, tool: usize) {
        let Some(definition) = self
            .input
            .tools
            .iter()
            .find(|candidate| candidate.id == tool)
        else {
            return;
        };
        let radius = definition.radius;
        let damage = definition.damage;
        let cost = definition.cost * self.cost_multi();
        let stamina = self.stamina[self.active];
        if stamina == 0.0 || (!self.input.overuse && tool != 0 && stamina.ceil() < cost) {
            if self.config.policy == Policy::Auto {
                while self.tool_id > 0 {
                    let known = self.input.tools.iter().find(|t| t.id == self.tool_id);
                    let Some(known) = known else {
                        self.tool_id -= 1;
                        continue;
                    };
                    if self.stamina[self.active] >= known.cost {
                        break;
                    }
                    self.tool_id -= 1;
                }
            }
            return;
        }
        self.attempted = false;
        let hits_before = self.hits;
        for index in 0..self.rocks.len() {
            if self.within(index, x, y, radius) {
                self.hit(index, damage);
            }
        }
        if !self.attempted {
            return;
        }
        let before = self.stamina[self.active];
        if self.effects[0] > 0.0 {
            self.stamina[self.active] += cost * self.quantity(2) / 100.0;
        }
        self.stamina[self.active] = (self.stamina[self.active] - cost).max(0.0);
        let slot = self.active_slot();
        let used = (before - self.stamina[self.active]).max(0.0);
        self.contributions[slot].digs += 1.0;
        self.contributions[slot].spent += used;
        if self.hits == hits_before {
            self.contributions[slot].resisted_stamina += used;
        }
        self.spent += self.input.characters[self.active].max_stamina.min(cost);
        self.actions += 1;
        self.record(EventKind::Dig {
            tool,
            character: self.active,
            used: before - self.stamina[self.active],
        });
        let chance = self.shop(31) * (1.0 + self.effects[7].min(0.5)) / 100.0;
        if self.rng.next() < chance {
            let count = (1.0
                + (self.shop(32) + self.quantity(7) * self.effects[7]) / 100.0
                + self.rng.next())
            .floor()
            .max(0.0) as usize;
            for n in 0..count {
                if n >= self.rocks.len() {
                    break;
                }
                let index = self.rng.int(0, self.rocks.len() as i64 - 1) as usize;
                if self.rocks[index].kind < 0 || self.rocks[index].hp <= 0.0 {
                    continue;
                }
                self.rng.next();
                let facing = self.rng.int(0, 1);
                let position = self.position(index);
                let boss = f64::from(u8::from(self.rocks[index].boss));
                let art = data::sprite(&format!("CaveTool{tool}")).unwrap_or([0.0, 0.0]);
                let px = js_round(
                    position.x + (20.0 * boss + (25.0 * facing as f64 - 4.0 - position.w / 2.0)),
                ) + js_round(art[0] / 4.0)
                    + 14.0 * (-1f64).powi(facing as i32);
                let py = js_round(position.y + (40.0 * boss + (-20.0 - position.h)))
                    + js_round(art[1] / 1.3)
                    + 7.0;
                self.pending.push(Pending::Shadow {
                    remaining: 202 + 5 * n as i64,
                    depth: self.depth,
                    damage,
                    tool,
                    x: px,
                    y: py,
                });
            }
        }
    }

    /// `descend`: leave the floor for the next one.
    fn descend(&mut self) {
        let slot = self.active_slot();
        self.contributions[slot].descents += 1.0;
        if !self.rocks.iter().any(|rock| rock.kind >= 0) {
            self.full_count += 1;
        }
        self.effects[5] = (self.effects[5] - 1.0).max(0.0);
        if self.effects[4] > 0.0 {
            self.effects[4] += 1.0;
        }
        self.transition = true;
        self.found = false;
        self.radar = None;
        self.record(EventKind::Descend);
        let slots: Vec<i64> = self.rocks.iter().map(|rock| rock.slot).collect();
        for slot in slots {
            self.tasks.push(Task {
                remaining: 4 * slot,
                kind: TaskKind::Leave { slot },
            });
        }
        self.tasks.push(Task {
            remaining: 876,
            kind: TaskKind::ClearRocks,
        });
        self.tasks.push(Task {
            remaining: 930,
            kind: TaskKind::NextFloor,
        });
    }

    /// `settle`: end the run and bank or keep its loot.
    fn settle(&mut self, status: Status) {
        self.status = status;
        if !matches!(status, Status::Exhausted | Status::Rope) {
            return;
        }
        self.settled = true;
        self.gross_rewards = Some(self.rewards);
        if status == Status::Rope {
            let amber_multiplier = 1.0 + self.shop(22) / 100.0;
            let mut items = Vec::new();
            for reward in self.reward_order.clone() {
                if reward == 0 {
                    self.rewards[0] *= amber_multiplier;
                }
                items.push(ExitItem {
                    reward,
                    gross: self.gross_rewards.unwrap_or(self.rewards)[reward],
                    expected: self.rewards[reward],
                    chance: None,
                    rounded_up: None,
                    collected: self.rewards[reward],
                });
            }
            self.exit = Some(Exit {
                kind: ExitKind::Rope,
                retained: 1.0,
                amber_multiplier,
                items,
            });
            self.record(EventKind::Rope);
            return;
        }
        let retained = 1.0 - self.shop(19) / 100.0;
        let mut items = Vec::new();
        for reward in self.reward_order.clone() {
            self.rewards[reward] *= retained;
            let expected = self.rewards[reward];
            let chance = expected - expected.floor();
            let rounded_up = if reward == 0 {
                None
            } else {
                Some(self.rng.next() < chance)
            };
            if reward != 0 {
                self.rewards[reward] = if rounded_up == Some(true) {
                    expected.ceil()
                } else {
                    expected.floor()
                };
            }
            items.push(ExitItem {
                reward,
                gross: self.gross_rewards.unwrap_or(self.rewards)[reward],
                expected,
                chance: if reward == 0 { None } else { Some(chance) },
                rounded_up,
                collected: self.rewards[reward],
            });
        }
        self.exit = Some(Exit {
            kind: ExitKind::Toll,
            retained,
            amber_multiplier: 1.0,
            items,
        });
        self.record(EventKind::Exhausted);
    }

    /// `decision`: drink an elixir, descend or dig.
    fn decision(&mut self) {
        let manual = self.config.policy != Policy::Auto;
        let live: Vec<usize> = (0..self.rocks.len())
            .filter(|index| self.rocks[*index].kind >= 0)
            .collect();
        if self.amber.is_some() {
            if matches!(
                self.amber.as_ref().map(|amber| amber.phase),
                Some(AmberPhase::Buildup | AmberPhase::Dump)
            ) && self.at_amber_reserve()
            {
                if let Some(amber) = self.amber.as_mut() {
                    amber.activation_depth = Some(self.depth as usize + 1);
                }
                self.amber_phase(AmberPhase::Activate, "Finishing stamina reserve reached");
            }
            if self.try_amber_rope() {
                return;
            }
        }
        if self.choose_elixir() {
            self.countdown = if manual {
                -36.0 + (self.config.reaction_ms / 10.0).ceil()
            } else {
                -15.0
            };
            return;
        }
        if self.amber.as_ref().map(|amber| amber.phase) == Some(AmberPhase::Activate) {
            let waiting = self.config.elixirs.iter().enumerate().any(|(slot, id)| {
                *id == 3
                    && self.charges.get(slot).copied().unwrap_or(0.0) > 0.0
                    && self.ready.get(slot).copied().unwrap_or(0.0) != 0.0
            });
            if waiting {
                return;
            }
            self.amber_phase(
                AmberPhase::Harvest,
                "Available oranges consumed after stamina burn",
            );
            if self.try_amber_rope() {
                return;
            }
        }
        let clear = if manual {
            self.config.policy == Policy::Clear
                || self.amber.is_some()
                || (self.config.policy == Policy::ClearFifth && (self.depth + 1) % 5 == 0)
        } else {
            self.config.full_clear
        };
        let grand = self
            .pending
            .iter()
            .any(|pending| matches!(pending, Pending::Grand { .. }));
        if manual && self.found && !grand && (!clear || live.is_empty()) {
            self.descend();
            return;
        }
        if !manual && self.found && !clear {
            if !grand {
                self.descend();
            }
            return;
        }
        if live.is_empty() && (!self.found || manual) {
            let waiting = self.pending.iter().any(|pending| {
                matches!(
                    pending,
                    Pending::Nova { .. } | Pending::Shadow { .. } | Pending::Adjacent { .. }
                )
            });
            if !self.found && !waiting {
                self.settle(Status::Blocked);
            }
            return;
        }
        let cost_multi = self.cost_multi();
        let mut tools: Vec<usize> = if manual {
            let chosen = self.config.tool;
            self.input
                .tools
                .iter()
                .filter(|tool| {
                    (chosen == ToolChoice::Adaptive || chosen == ToolChoice::Tool(tool.id))
                        && (self.input.overuse
                            || tool.id == 0
                            || self.stamina[self.active].ceil() >= tool.cost * cost_multi)
                })
                .map(|tool| tool.id)
                .collect()
        } else {
            vec![self.tool_id]
        };
        if manual
            && tools.is_empty()
            && let Some(first) = self.input.tools.first()
        {
            tools.push(first.id);
        }
        let harvesting = matches!(
            self.amber.as_ref().map(|amber| amber.phase),
            Some(AmberPhase::Harvest | AmberPhase::RopeSearch)
        );
        let remaining: f64 = if harvesting {
            self.config.party.iter().map(|id| self.stamina[*id]).sum()
        } else {
            0.0
        };
        let reachable_loot = harvesting
            && tools.iter().any(|tool| {
                let Some(definition) = self.input.tools.iter().find(|def| def.id == *tool) else {
                    return false;
                };
                live.iter()
                    .any(|index| self.can_harvest(&self.rocks[*index], definition, remaining))
            });
        if harvesting && self.found && !grand && !reachable_loot {
            self.descend();
            return;
        }
        if matches!(
            self.amber.as_ref().map(|amber| amber.phase),
            Some(AmberPhase::Buildup | AmberPhase::Dump)
        ) && self.finishing_party.first() == Some(&self.active)
        {
            let available =
                self.stamina[self.active] - self.start[self.active] * self.amber_reserve();
            tools.retain(|tool| {
                self.input
                    .tools
                    .iter()
                    .find(|definition| definition.id == *tool)
                    .is_some_and(|definition| self.harvest_net(definition) <= available)
            });
            if tools.is_empty() {
                if let Some(amber) = self.amber.as_mut() {
                    amber.activation_depth = Some(self.depth as usize + 1);
                }
                self.amber_phase(
                    AmberPhase::Activate,
                    "Next dig would consume finishing reserve",
                );
                return;
            }
        }
        if matches!(
            self.amber.as_ref().map(|amber| amber.phase),
            Some(AmberPhase::Buildup | AmberPhase::Dump)
        ) && let Some(policy) = self.config.amber
            && self.depth + 1 >= policy.dump_depth as i64
        {
            let mut dump: Option<(f64, f64, usize, f64, f64)> = None;
            for tool in &tools {
                let Some(definition) = self.input.tools.iter().find(|def| def.id == *tool) else {
                    continue;
                };
                for index in &live {
                    let rock = &self.rocks[*index];
                    let ratio = self.power() / rock.requirement;
                    let chance = ratio.powf(0.6).min(1.0);
                    let net = self.stamina[self.active].min(self.burn_net(definition));
                    if net == 0.0 {
                        continue;
                    }
                    let damage = hit_damage(self.input, definition.damage, ratio);
                    if chance * damage * self.stamina[self.active] / net >= rock.hp {
                        continue;
                    }
                    let position = self.position(*index);
                    let (x, y) = (position.x, position.y - position.h / 2.0);
                    let mut hit = 0.0;
                    for other in &live {
                        if !self.within(*other, x, y, definition.radius) {
                            continue;
                        }
                        let other = &self.rocks[*other];
                        let other_ratio = self.power() / other.requirement;
                        hit += other_ratio.powf(0.6).min(1.0)
                            * (hit_damage(self.input, definition.damage, other_ratio) / other.hp)
                                .min(1.0);
                    }
                    let score = hit / net;
                    if dump.is_none_or(|(_, _, _, best, best_net)| {
                        score < best || (score == best && net > best_net)
                    }) {
                        dump = Some((x, y, *tool, score, net));
                    }
                }
            }
            if let Some((x, y, tool, _, _)) = dump {
                self.amber_phase(
                    AmberPhase::Dump,
                    "Burn on visible objects unlikely to break with this character",
                );
                let before = self.spent;
                self.dig(x, y, tool);
                if let Some(amber) = self.amber.as_mut() {
                    amber.dump_spent += self.spent - before;
                    amber.dump_digs += 1;
                }
                self.countdown = -36.0 + (self.config.reaction_ms / 10.0).ceil();
                return;
            }
        }
        let mut best: Option<(f64, f64, usize, f64)> = None;
        let mut best_score = -1.0;
        for tool in &tools {
            let Some(definition) = self.input.tools.iter().find(|t| t.id == *tool) else {
                continue;
            };
            let radius = definition.radius;
            let points: Vec<(f64, f64)> = if manual {
                live.iter()
                    .map(|index| {
                        let position = self.position(*index);
                        (position.x, position.y - position.h / 2.0)
                    })
                    .collect()
            } else {
                (0..25)
                    .map(|_| {
                        let (grid_x, grid_y) = grid(self.rng.int(0, SLOTS as i64 - 1));
                        (
                            grid_x + self.rng.int(-20, 20) as f64,
                            grid_y + self.rng.int(-20, 20) as f64,
                        )
                    })
                    .collect()
            };
            for (x, y) in points {
                let mut count = 0usize;
                let mut score = 0.0;
                for index in &live {
                    if !self.within(*index, x, y, radius) {
                        continue;
                    }
                    count += 1;
                    let rock = &self.rocks[*index];
                    let ratio = self.power() / rock.requirement;
                    let chance = ratio.powf(0.6).min(1.0);
                    let dealt = if harvesting {
                        hit_damage(self.input, definition.damage, ratio)
                    } else {
                        definition.damage * (1.0 + self.shop(33) / 100.0)
                    };
                    score += if harvesting {
                        if reachable_loot && !self.can_harvest(rock, definition, remaining) {
                            0.0
                        } else {
                            chance * (dealt / rock.hp).min(1.0)
                        }
                    } else if self.config.policy == Policy::Dive {
                        chance
                            * (f64::from(u8::from(dealt >= rock.hp))
                                + 0.05 * (dealt / rock.hp).min(1.0))
                    } else {
                        rock.hp.min(dealt) * chance
                    };
                }
                if count == 0 {
                    continue;
                }
                if !manual {
                    score = count as f64;
                } else {
                    score /= definition.cost
                        * cost_multi
                        * if self.effects[0] > 0.0 { 0.7 } else { 1.0 };
                }
                if manual
                    && !self.found
                    && self.radar.is_some_and(|radar| {
                        radar.depth == self.depth && distance(x, y, radar.x, radar.y) < 170.0
                    })
                {
                    score *= 1.1;
                }
                if score > best_score {
                    best_score = score;
                    best = Some((x, y, *tool, count as f64));
                }
            }
        }
        if let Some((x, y, tool, _)) = best {
            self.dig(x, y, tool);
        }
        if !manual
            && let Some(definition) = self.input.tools.iter().find(|t| t.id == self.tool_id)
            && best.map_or(0.0, |(_, _, _, count)| count) < definition.cost
            && self.tool_id > 0
            && let Some(previous) = self.input.tools.iter().rfind(|tool| tool.id < self.tool_id)
        {
            self.tool_id = previous.id;
        }
        if !manual
            && self.found
            && !grand
            && (!clear || !self.rocks.iter().any(|rock| rock.kind >= 0))
        {
            self.descend();
        }
        if manual {
            self.countdown = -36.0 + (self.config.reaction_ms / 10.0).ceil();
        }
    }

    /// `advanceEffects`: tick the delayed effects and fire the due ones.
    fn advance_effects(&mut self) {
        for pending in &mut self.pending {
            let remaining = pending.remaining() - 1;
            pending.set_remaining(remaining);
        }
        for index in 0..self.pending.len() {
            match self.pending[index].clone() {
                Pending::Nova {
                    remaining,
                    start,
                    damage,
                    mut seen,
                } => {
                    if remaining > 450 && remaining % 5 == 0 && !self.transition {
                        let scale = tween(
                            0.1,
                            2.0,
                            (self.time - start) as f64 * 10.0,
                            1350.0,
                            quad_out,
                        );
                        let size = 100.0 * scale;
                        let slots: Vec<i64> = self.rocks.iter().map(|rock| rock.slot).collect();
                        for slot in slots {
                            if seen.contains(&slot) {
                                continue;
                            }
                            let (grid_x, grid_y) = grid(slot);
                            let radius = size / 50.0 * 193.0 + 30.0;
                            if distance(
                                65.0 + size / 50.0 * 49.0,
                                520.0 + size / 50.0 * 49.0,
                                grid_x,
                                grid_y,
                            ) >= radius
                            {
                                continue;
                            }
                            seen.push(slot);
                            if self.rng.next() < self.shop(38) / 100.0
                                && let Some(rock) =
                                    self.rocks.iter().position(|rock| rock.slot == slot)
                            {
                                self.hit(rock, damage);
                            }
                        }
                    }
                    if let Pending::Nova { seen: stored, .. } = &mut self.pending[index] {
                        *stored = seen;
                    }
                }
                Pending::Adjacent {
                    remaining,
                    depth,
                    slot,
                    damage,
                } => {
                    if !self.transition
                        && depth == self.depth
                        && remaining == 185
                        && let Some(rock) = self.rocks.iter().position(|rock| rock.slot == slot)
                    {
                        self.hit(rock, damage);
                    }
                }
                Pending::Shadow {
                    remaining,
                    depth,
                    damage,
                    tool,
                    x,
                    y,
                } => {
                    if !self.transition && depth == self.depth && remaining == 134 {
                        let radius = self
                            .input
                            .tools
                            .iter()
                            .find(|candidate| candidate.id == tool)
                            .map_or(0.0, |candidate| candidate.radius);
                        for index in 0..self.rocks.len() {
                            if self.within(index, x, y, radius) {
                                self.hit(index, damage);
                            }
                        }
                    }
                }
                Pending::Grand { .. } => {}
            }
        }
        self.pending.retain(|pending| pending.remaining() >= 0);
    }

    /// One 10 ms frame of the upstream callback loop.
    fn tick(&mut self) {
        self.time += 1;
        let mut index = 0;
        while index < self.tasks.len() {
            self.tasks[index].remaining -= TICK_MS;
            if self.tasks[index].remaining <= 0 {
                let task = self.tasks.remove(index);
                self.run_task(task.kind);
            } else {
                index += 1;
            }
        }
        if !self.transition {
            self.destroy();
        }
        self.advance_effects();
        self.tick_stamina();
        for slot in 0..self.charges.len() {
            if self.charges[slot] > 0.0 && self.ready[slot] == 2.0 {
                self.ready[slot] = 2.1;
                self.tasks.push(Task {
                    remaining: 800,
                    kind: TaskKind::ElixirReady { slot },
                });
            }
        }
    }

    fn run_task(&mut self, kind: TaskKind) {
        match kind {
            TaskKind::Rise { slot } => {
                if let Some(rock) = self.rocks.iter_mut().find(|rock| rock.slot == slot) {
                    rock.rise_at = Some(self.time);
                }
            }
            TaskKind::ClearRocks => self.rocks.clear(),
            TaskKind::NextFloor => {
                self.depth += 1;
                self.generate();
                self.tasks.push(Task {
                    remaining: 930,
                    kind: TaskKind::EndTransition,
                });
            }
            TaskKind::EndTransition => self.transition = false,
            TaskKind::Leave { slot } => {
                if let Some(index) = self.rocks.iter().position(|rock| rock.slot == slot) {
                    let y = self.position(index).y;
                    let rock = &mut self.rocks[index];
                    rock.leave_y = y;
                    rock.leave_at = Some(self.time);
                }
            }
            TaskKind::ElixirReady { slot } => {
                if let Some(ready) = self.ready.get_mut(slot) {
                    *ready = 0.0;
                }
            }
            TaskKind::Celebration => {
                for n in 0..30 {
                    self.tasks.push(Task {
                        remaining: 10 * n,
                        kind: TaskKind::Visual { draws: 5 },
                    });
                }
            }
            TaskKind::Visual { draws } => self.rng.rolls(draws),
        }
    }

    /// The exhaustion, switch and decision block of the upstream tick.
    fn tick_stamina(&mut self) {
        if self.stamina[self.active] <= 0.0 {
            if self.effects[2] > 0.0 && self.rng.next() < 0.5 {
                self.effects[2] = 0.0;
                let share = 0.5f64.min(self.quantity(2) / 100.0);
                self.stamina[self.active] = self.input.characters[self.active].max_stamina * share;
            } else {
                self.effects[0] = 0.0;
                self.effects[2] = 0.0;
                let next = self
                    .config
                    .party
                    .iter()
                    .copied()
                    .find(|id| self.stamina[*id] > 0.0);
                match next {
                    None => {
                        if !self.transition {
                            self.settle(Status::Exhausted);
                        }
                    }
                    Some(next) => {
                        self.active = next;
                        let slot = self.active_slot();
                        self.contributions[slot].start_depth = Some(self.depth as usize + 1);
                        if self.input.switch_nova {
                            self.nova();
                        }
                        self.record(EventKind::Switch { character: next });
                    }
                }
            }
        }
        if self.status == Status::Running && !self.transition {
            let step = if self.config.policy == Policy::Auto {
                1.0 + self.config.speed as f64 + (self.config.speed / 4) as f64
            } else {
                1.0
            };
            self.countdown = js_round(self.countdown - step);
            if self.countdown < -35.0 {
                self.countdown = 0.0;
                self.decision();
            }
        }
    }
}

/// Run one expedition to the end, reporting progress.
///
/// Returns `None` when the reporter asked to stop.
pub fn simulate(
    input: &Inputs,
    config: Config,
    options: RunOptions,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Result<Option<RunResult>, ConfigError> {
    let mut run = Run::new(input, config, options)?;
    loop {
        if run.done() {
            return Ok(Some(run.result()));
        }
        run.step(REPORT_TICKS);
        if report(run.progress()) == Control::Stop {
            return Ok(None);
        }
    }
}

/// The notes a run carries, matching `buildSpelunkInputs`.
fn notes(input: &Inputs, config: &Config) -> Vec<String> {
    let mut notes = vec![
        "Native 10 ms callback and animated targeting model, with a seeded local RNG stream. \
         Unsaved global RNG and callback phases prevent reproducing a particular live-client run."
            .to_string(),
        "Account bonuses stay fixed during a run. New discoveries, boss unlocks and records are \
         reported, not fed back into cached account stats."
            .to_string(),
        "No AFK skill EXP, purchase sequencing or daily-haul claims are included.".to_string(),
    ];
    if input.partial {
        notes.push(
            "Some account multipliers are partial. Inspect Power and maximum-stamina breakdowns \
             for missing metadata."
                .to_string(),
        );
    }
    if config.elixirs.contains(&11) {
        notes.push(
            "The unlabeled elixir has no effect branch in the current game source.".to_string(),
        );
    }
    notes
}

/// The sprite a rock draws with, as `[width, height]`.
///
/// The extracted table carries every sprite the generator can name; a name it
/// lacks draws at zero size, which no tool reaches.
fn sprite(cave: &Cave, rock: &Rock) -> (f64, f64) {
    let name = if rock.boss {
        format!("CaveBoss{}", cave.id)
    } else if rock.kind >= 0 {
        format!("CaveRock{}{}", cave.letter, rock.kind)
    } else if rock.kind == EXIT {
        format!("CaveRockExit{}", cave.id)
    } else {
        "CaveRope".to_string()
    };
    data::sprite(&name)
        .map(|art| (art[0], art[1]))
        .unwrap_or((0.0, 0.0))
}

/// The tile slot's drawing origin.
fn grid(slot: i64) -> (f64, f64) {
    (
        477.0 + 36.0 * (slot % 12) as f64 - 36.0 * (slot / 12) as f64,
        26.0 + 18.0 * (slot % 12) as f64 + 18.0 * (slot / 12) as f64,
    )
}

/// The eight-way distance the hit radius uses.
fn distance(x: f64, y: f64, a: f64, b: f64) -> f64 {
    let dx = (x - a).abs();
    let dy = (y - b).abs();
    0.9604339 * dx.max(dy) + 0.397824735 * dx.min(dy)
}

/// The neighbours a tile has, in the order the elixir rolls them.
fn neighbors(slot: i64) -> Vec<i64> {
    let mut out = Vec::with_capacity(4);
    if slot % 12 != 0 {
        out.push(-1);
    }
    if slot > 11 {
        out.push(-12);
    }
    if slot < 132 {
        out.push(12);
    }
    if (slot + 1) % 12 != 0 {
        out.push(1);
    }
    out
}

fn quad_out(t: f64) -> f64 {
    -t * (t - 2.0)
}

fn elastic_out(t: f64) -> f64 {
    if t == 0.0 {
        0.0
    } else if t == 1.0 {
        1.0
    } else {
        (-6.931_471_805_599_453 * t).exp()
            * (std::f64::consts::TAU * (0.001 * t - 75e-6) / 3e-4).sin()
            + 1.0
    }
}

fn tween(from: f64, to: f64, elapsed: f64, duration: f64, easing: impl Fn(f64) -> f64) -> f64 {
    let q = easing(elapsed.clamp(0.0, duration) / duration);
    from * (1.0 - q) + to * q
}

#[cfg(test)]
mod tests;
