//! Worship Tower Defense: the static tables, the formula port and the
//! deterministic wave simulator.
//!
//! Ported from the upstream calculator
//! (`js/stats/systems/w3/worship-td.js`, `js/stats/data/w3/worship-td.js` and
//! `worship-td-calc.html`). The tables in `data/worship-td.json` are written by
//! `script/extract-worship-td-data.mjs`; every formula below keeps the upstream
//! evaluation order so a seeded run matches the source tick for tick.
//!
//! Two upstream runtime choices are explicit assumptions here, as they are
//! there: wizards scan on 100 ms ticks with a 300 ms popup after each cooldown,
//! and a pulse volley rolls one critical for all of its hits. Build plans,
//! phases and their staged layouts live in [`plan`]; a run's recorded timeline
//! lives in [`replay`]. The strategy search lives in [`optimize`]: it reports
//! progress and stops when the reporter asks, so a section can cancel it.

mod optimize;
mod plan;
mod replay;
mod sim;

pub use optimize::{
    Finalist, Mode, Options, Outcome, Score, Search, Trials, optimize, simulate_trials,
};
pub use plan::{
    GuidePreset, PhaseSell, PhaseWhen, Plan, PlanIssue, PlanPhase, PlanPlacement, PlanReject,
    PlanStage, PlanTower, RootPolicy, SkipPolicy, guide_preset, guide_presets_for_trial,
    heuristic_plan, plan_issues, plan_stages, tower_upgrade_access,
};
pub use replay::{Replay, ReplayConfig, ReplayFrame};
pub use sim::{
    CloudView, EffectKind, EndReason, Inputs, KrakenView, MobView, PhaseLog, Placed, PlanFailure,
    Run, RunError, RunResult, RunView, TowerView,
};

/// The modelling choices and neutral inputs the simulator and plans make.
///
/// These are assumptions, not game rules: each one describes timing the source
/// leaves to animation frames, a plan check the game applies on screen, or an
/// account bonus this build does not decode yet. The Model pane lists them.
pub const ASSUMPTIONS: &[&str] = &[
    "Wizards scan on 100 ms ticks. After a shot the cooldown runs, then a 300 ms popup precedes the ready frame, so the period is ceil(cooldown / 0.1) + 3 ticks; a failed scan keeps the wizard ready.",
    "Pulse, Frozone and Void hit when they fire; Storm bolts land 300 ms later.",
    "Fireballs land 1.8 s after firing, and the second fireball lands 0.5 s later.",
    "Kraken underlings move 3 px per tick. Their 200 ms, 1 s and 2 s timers start at spawn, and the 1 s wander roll lands on a 100 ms tick boundary.",
    "Kraken knockback tweens override velocity for 0.6 s, sampled on 100 ms ticks.",
    "A Kraken hit on an already-dead acquired target deals no damage but still consumes the hit and applies knockback.",
    "Sprite centres use the unscaled frame size.",
    "The 1 s wave timer ticks on the first 100 ms tick of each second.",
    "After an upgrade a tower keeps its raw cooldown until the next trait pass, matching the source quirk.",
    "Poisonic clouds are centred on the target at NodeY - 24.",
    "The map-100 TrialType override is ignored.",
    "Each direct hit rolls its own critical, except a Pulse volley, which rolls once for all of its hits.",
    "Planning excludes off-scene cursor zones, NPC-obscured upgrade clicks, and spawn or exit placements that change with the camera position. Bloo Radley's pre-quest click zone is reserved.",
    "Frozone fires only while its target is on the side the wizard faces; its ring then hits every mob in range.",
    "Manual upgrades use an all-A trait recipe; a plan uses its own trait string.",
    "Root casts, account start points beyond the base 5, kill-point meals and Meritocracy are neutral this build.",
    "Plan points, kills and EXP come from the simulator. Souls per hour, charge and claim rewards are not simulated here.",
    "The optimizer is a bounded best-found search over seeded plans, not a global optimum. It screens plans on the mode's budget, then compares the finalists, the coverage heuristic and the guide presets on shared fresh seeds.",
    "The optimizer's hold-template layout search and its souls-per-hour objective are neutral this build; it searches the strategy and tower plans only.",
];

use serde::Deserialize;
use std::sync::OnceLock;

const WORSHIP_JSON: &str = include_str!("../../data/worship-td.json");

/// Rows per wave cycle (`ActorEvents_340._GenINFO[62]`).
pub const WAVE_CYCLE: usize = 30;
/// Every tower type the trial maps unlock.
pub const TOWER_COUNT: usize = 9;
/// The 100 ms simulation tick.
pub const TICKS_PER_SECOND: f64 = 10.0;

/// The shipped Worship TD tables, parsed once.
pub fn data() -> &'static WorshipData {
    static DATA: OnceLock<WorshipData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(WORSHIP_JSON).expect("the shipped worship-td data is valid JSON")
    })
}

/// Everything the simulator reads from the game.
#[derive(Debug, Deserialize)]
pub struct WorshipData {
    pub trials: Vec<Trial>,
    /// Scene node tables keyed by map id.
    pub geometry: std::collections::BTreeMap<usize, Geometry>,
    pub towers: Vec<TowerDef>,
    pub traits: Vec<TraitDef>,
    /// The `N2L` tokens `superBitType` matches against `Gaming[12]`.
    #[serde(rename = "superBits")]
    pub super_bits: std::collections::BTreeMap<usize, String>,
    #[serde(rename = "menagerieBoost15PerLevel")]
    pub menagerie_boost_15_per_level: f64,
}

/// One trial map: its path, waves, mob and scene.
#[derive(Debug, Deserialize)]
pub struct Trial {
    pub trial: usize,
    pub map: usize,
    #[serde(rename = "hpBase")]
    pub hp_base: f64,
    #[serde(rename = "mobKey")]
    pub mob_key: String,
    #[serde(rename = "totemX")]
    pub totem_x: f64,
    #[serde(rename = "totemY")]
    pub totem_y: f64,
    /// Charge spent per claim.
    pub charge: f64,
    #[serde(rename = "effReq")]
    pub eff_req: f64,
    pub path: Vec<usize>,
    pub flags: Vec<u8>,
    pub scene: Scene,
    pub background: usize,
    /// The 30 wave rows `WorshipInfo[trial]` repeats with the wave tier.
    pub waves: Vec<WaveRow>,
    #[serde(rename = "mobDef")]
    pub mob_def: MobDef,
    pub sprite: MobSprite,
    pub npcs: Vec<NpcZone>,
}

/// One wave row before the tier scaling.
#[derive(Clone, Debug, Deserialize)]
pub struct WaveRow {
    pub count: f64,
    pub skull: f64,
    pub size: f64,
    pub mods: f64,
}

/// A row scaled the way `tdWaveRow` spawns it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wave {
    pub wave: u32,
    pub tier: f64,
    pub count: f64,
    pub skull: f64,
    pub size: f64,
    pub mods: f64,
}

/// The trial scene: its platform art size.
#[derive(Clone, Debug, Deserialize)]
pub struct Scene {
    pub plat: usize,
    pub w: f64,
    pub h: f64,
}

/// The mob sheet's frame grid and animation ranges.
#[derive(Clone, Debug, Deserialize)]
pub struct MobSprite {
    pub across: usize,
    pub down: usize,
    pub idle: [usize; 2],
    pub walk: [usize; 2],
}

/// The trial's mob: walk speed, height and frame size.
#[derive(Clone, Debug, Deserialize)]
pub struct MobDef {
    pub key: String,
    pub name: String,
    #[serde(rename = "moveSpeed")]
    pub move_speed: f64,
    pub height: f64,
    #[serde(rename = "frameW")]
    pub frame_w: f64,
    #[serde(rename = "frameH")]
    pub frame_h: f64,
}

/// A rectangle an NPC click zone blocks.
#[derive(Clone, Debug, Deserialize)]
pub struct NpcZone {
    pub name: String,
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
}

/// One scene node: its platform span, walk line and connector nodes.
#[derive(Clone, Debug, Deserialize)]
pub struct Node {
    pub x: f64,
    pub length: f64,
    pub y: f64,
    pub neighbors: Vec<usize>,
}

/// One map's scene geometry.
#[derive(Clone, Debug, Deserialize)]
pub struct Geometry {
    pub nodes: Vec<Node>,
    /// `[screenX, screenY, endNode, …]`, as `SceneEvents` places them.
    pub portals: Vec<Vec<f64>>,
}

/// One tower type's scaling.
#[derive(Debug, Deserialize)]
pub struct TowerDef {
    pub name: String,
    #[serde(rename = "perLvA")]
    pub per_lv_a: f64,
    #[serde(rename = "perLvB")]
    pub per_lv_b: f64,
    #[serde(rename = "baseCost")]
    pub base_cost: f64,
}

/// A trait side: its value and the stat key it feeds.
#[derive(Debug, Deserialize)]
pub struct TraitSide {
    pub value: f64,
    pub key: usize,
}

/// One tower type's two trait sides.
#[derive(Debug, Deserialize)]
pub struct TraitDef {
    pub a: TraitSide,
    pub b: TraitSide,
}

/// A trial by index, if the data declares it.
pub fn trial(index: usize) -> Option<&'static Trial> {
    data().trials.get(index)
}

/// A trial's scene geometry.
pub fn geometry(trial: &Trial) -> Option<&'static Geometry> {
    data().geometry.get(&trial.map)
}

/// A mob's HP multiplier for its size and skull.
pub fn hp_multi(size: f64, skull: f64) -> f64 {
    3f64.powf(size - 2.0) * (3.0 + 0.3 * skull).powf(skull)
}

/// A trial's base mob HP.
pub fn mob_max_hp(trial: &Trial, size: f64, skull: f64) -> f64 {
    (trial.hp_base * hp_multi(size, skull)).ceil()
}

/// The countdown the next wave waits for, from the wave number.
pub fn next_wave_countdown(t: f64) -> f64 {
    if t % 10.0 == 0.0 && t != 0.0 {
        return 9999.0;
    }
    if (t / 10.0).floor() > 19.0 {
        return 25.0 + 5.0 * ((t - 10.0) / 10.0).floor();
    }
    if t > 9.0 {
        25.0
    } else if t > 4.0 {
        20.0
    } else if t > 0.5 {
        15.0
    } else {
        6.0
    }
}

/// The wave-cycle offset the row table uses.
pub fn wave_adjust(t: f64) -> f64 {
    (t - WAVE_CYCLE as f64 * (t.max(0.0) / WAVE_CYCLE as f64).floor()).round()
}

/// The tier the wave number has climbed to.
pub fn wave_tier(t: f64) -> f64 {
    (t.max(0.0) / WAVE_CYCLE as f64).floor()
}

/// The scaled row a 1-based wave spawns.
pub fn wave_row(trial: &Trial, wave: u32) -> Wave {
    let t = f64::from(wave.saturating_sub(1));
    let tier = wave_tier(t);
    let index = wave_adjust(t).max(0.0).min((WAVE_CYCLE - 1) as f64) as usize;
    let raw = trial.waves.get(index).cloned().unwrap_or(WaveRow {
        count: 0.0,
        skull: 0.0,
        size: 1.0,
        mods: 0.0,
    });
    Wave {
        wave,
        tier,
        count: (raw.count * (1.0 + tier / 3.0)).ceil(),
        skull: (raw.skull + 2.0 * tier).round(),
        size: raw.size,
        mods: raw.mods,
    }
}

/// Skull currency for a status skull and wave counter.
pub fn skull_currency(s: f64, wave_counter: f64, salt_lick_x: f64) -> f64 {
    let x = salt_lick_x;
    let w = wave_counter;
    if s >= 6.0 {
        ((250.0 * (s.round() - 6.0).powf(1.5) + w.floor()) * x).floor()
    } else if s == 5.0 {
        ((150.0 + w) * x).floor()
    } else if s == 4.0 {
        ((70.0 + w) * x).floor()
    } else if s == 3.0 {
        ((25.0 + (w / 2.0).floor()) * x).floor()
    } else if s == 2.0 {
        ((10.0 + (w / 3.0).floor()) * x).round()
    } else if s == 1.0 {
        ((5.0 + (w / 4.0).floor()) * x).round()
    } else {
        ((2.0 + (w / 5.0).floor()) * x).round()
    }
}

/// Points one kill awards before it is rounded into the running total.
pub fn kill_credit(skull: f64, size: f64, wave_counter: f64, salt_lick_x: f64, mult: f64) -> f64 {
    (skull_currency(skull, wave_counter, salt_lick_x)
        * (2.0 * size - (size + 1.0).min(4.0)).max(0.34)
        * mult)
        .ceil()
}

/// EXP one kill awards.
pub fn kill_exp(trial: usize, status_skull: f64) -> f64 {
    let t = (trial + 1) as f64;
    t * t + (2.0 + 2.0 * t) * status_skull
}

/// Points a skipped countdown awards.
pub fn skip_reward(timer: f64, wave: u32) -> f64 {
    let wave_f = f64::from(wave);
    if !(timer > 2.0 && wave > 0 && !wave.is_multiple_of(10)) {
        return 0.0;
    }
    ((timer / 4.0).floor() + wave_f + ((wave_f + 1.0) / 10.0).floor()).min(999.0)
}

/// The trial-level damage and cost bonus (`Atom 5`).
pub fn atom5_bonus(tower_levels: &[u32; TOWER_COUNT]) -> f64 {
    tower_levels
        .iter()
        .map(|level| f64::from(level.saturating_sub(50)))
        .sum::<f64>()
        * 2.0
}

/// A tower's per-trial-level damage factor.
fn trial_gain(type_index: usize, trial_level: u32) -> f64 {
    let def = &data().towers[type_index];
    1.0 + (f64::from(trial_level) - 1.0) * def.per_lv_a / 100.0
}

/// A tower's damage at `level`.
pub fn tower_damage(type_index: usize, level: u32, trial_level: u32, multi: f64) -> f64 {
    let u = trial_gain(type_index, trial_level);
    let i = f64::from(level);
    let m = multi;
    let base = match type_index {
        0 => m * i * (1.2 - 0.193 * i / (15.0 + i)).powf(i) * u,
        1 => m * (2.0 + 1.4 * i * (1.23 - 0.225 * i / (20.0 + i)).powf(i)) * u,
        2 => 3.0 * m * i * (1.13 - 0.125 * i / (30.0 + i)).powf(i) * u,
        3 => 3.0 * m * i,
        4 => m * (13.0 + 5.0 * i) * (1.2 - 0.193 * i / (20.0 + i)).powf(i) * u,
        5 => 0.0,
        6 => 2.0 * m * i * (1.2 - 0.193 * i / (20.0 + i)).powf(i) * u,
        7 => m * (1.0 + i) * (1.2 - 0.193 * i / (20.0 + i)).powf(i) * u,
        8 => m * (8.0 + 2.0 * i) * (1.2 - 0.193 * i / (20.0 + i)).powf(i),
        _ => 0.0,
    };
    if base < 1e9 { base.floor() } else { base }
}

/// A tower's upgrade cost from `level` to the next.
pub fn tower_upgrade_cost(type_index: usize, level: u32, trial_level: u32) -> f64 {
    let i = f64::from(level);
    let f = match type_index {
        0 => (2.0 + 3.0 * i) * 1.57f64.powf(i),
        1 => (3.0 + 3.5 * i) * 1.58f64.powf(i),
        2 => (4.0 + 4.0 * i) * 1.61f64.powf(i),
        3 => (4.0 + 3.0 * i) * 1.5f64.powf(i),
        4 => (5.0 + 4.0 * i) * 1.63f64.powf(i),
        5 => (2.0 + 2.0 * i) * 1.55f64.powf(i),
        6 => (6.0 + 4.5 * i) * 1.6f64.powf(i),
        7 => (10.0 + 4.8 * i) * 1.63f64.powf(i),
        8 => (15.0 + 5.0 * i) * 1.65f64.powf(i),
        _ => f64::INFINITY,
    };
    let def = &data().towers[type_index];
    (f * (1.0 - (f64::from(trial_level) - 1.0) * def.per_lv_b / 100.0).max(0.05)).floor()
}

/// A tower's attack cooldown in seconds.
pub fn tower_atkcd(type_index: usize, level: u32, trial_level: u32) -> f64 {
    let i = f64::from(level);
    let f100 = |value: f64| (100.0 * value).floor() / 100.0;
    match type_index {
        0 => f100((4.2 - 2.65 * i / (i + 10.0)).max(1.0)),
        1 => f100(5.15 - 3.2 * i / (i + 7.0)),
        2 => f100(6.4 - 3.7 * i / (i + 7.0)),
        3 => f100(7.0 - 3.5 * i / (i + 7.0)).max(1.6),
        4 | 5 | 7 => f100(10.0 - 3.0 * i / (i + 7.0)),
        6 => ((10.0 - 3.0 * i / (i + 7.0)) / trial_gain(6, trial_level) * 100.0).floor() / 100.0,
        8 => f100(14.0 - 4.0 * i / (i + 7.0)),
        _ => 10.0,
    }
}

/// A tower's attack range in scene pixels.
pub fn tower_range(type_index: usize, level: u32, trial_level: u32) -> f64 {
    let i = f64::from(level);
    let def = &data().towers[type_index];
    match type_index {
        0..=2 => (80.0 + 150.0 * i / (i + 7.0)).floor(),
        3 => (85.0 + 25.0 * i / (i + 7.0) + (f64::from(trial_level) - 1.0) * def.per_lv_a).floor(),
        4 => (250.0 + 150.0 * i / (i + 7.0)).floor(),
        5 => (250.0 * trial_gain(5, trial_level)).floor(),
        6 | 7 => 275.0,
        8 => (50.0 + 25.0 * i / (i + 7.0) + (f64::from(trial_level) - 1.0) * def.per_lv_a).floor(),
        _ => 0.0,
    }
}

/// The cost of the next tower of a type, given how many stand.
pub fn new_tower_cost(type_index: usize, placed: usize) -> f64 {
    let n = placed as f64;
    let base = data().towers[type_index].base_cost;
    (base * ((n + 1.0) / 3.0).ceil() + 2.0 * (1.3 + 0.03 * n).powf(n)).floor()
}

/// The tower damage multiplier at a moment in the wave clock.
pub fn tower_damage_multi(inputs: &Inputs, first_tower_map9: f64, wave_clock: f64) -> f64 {
    (1.0 + inputs.atom5_bonus() / 100.0)
        * (1.0 + (inputs.dream7 * wave_clock / 100.0).max(0.0))
        * (1.0 + first_tower_map9 / 100.0)
}

/// The game's octagonal distance formula.
pub fn distance(dx: f64, dy: f64) -> f64 {
    0.9604339 * dx.abs().max(dy.abs()) + 0.397824735 * dx.abs().min(dy.abs())
}

/// A placement spot: its platform node and horizontal position.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Spot {
    pub node: usize,
    pub x: f64,
}

/// The scene coordinates a tower occupies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Centers {
    pub wx: f64,
    pub wy: f64,
    pub cx: f64,
    pub cy: f64,
    pub facing: f64,
}

/// Whether a requested spot is placeable, and why not.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub ok: bool,
    /// The snapped x the game would use.
    pub x: f64,
    pub reason: &'static str,
}

/// The node whose placement zone contains a scene point.
pub fn placement_node_at(trial: &Trial, x: f64, y: f64) -> Option<usize> {
    let geom = geometry(trial)?;
    if x < 0.0 || x > trial.scene.w || y < 0.0 || y > trial.scene.h {
        return None;
    }
    geom.nodes.iter().position(|node| {
        x > node.x - 5.0 && x < node.length + 5.0 && y > node.y - 80.0 && y < node.y + 40.0
    })
}

fn clamp_placement_x(node: &Node, x: f64) -> f64 {
    x.max(node.x + 30.0).min(node.length - 30.0).round()
}

/// The legal x spans on each platform node, computed the way the game's
/// placement bands are.
pub fn placement_bands(trial: &Trial) -> Vec<Vec<(f64, f64)>> {
    let Some(geom) = geometry(trial) else {
        return Vec::new();
    };
    let (w, h) = (trial.scene.w, trial.scene.h);
    let mut xs = vec![0.0, w];
    let mut ys = vec![0.0, h];
    for node in &geom.nodes {
        xs.push((node.x - 5.0).clamp(0.0, w));
        xs.push((node.length + 5.0).clamp(0.0, w));
        ys.push((node.y - 80.0).clamp(0.0, h));
        ys.push((node.y + 40.0).clamp(0.0, h));
    }
    let cuts = |values: &mut Vec<f64>| {
        values.sort_by(f64::total_cmp);
        values.dedup_by(|a, b| *a == *b);
        values.clone()
    };
    xs = cuts(&mut xs);
    ys = cuts(&mut ys);
    let mut probes = ys.clone();
    for j in 1..ys.len() {
        probes.push((ys[j - 1] + ys[j]) / 2.0);
    }
    let mut bands: Vec<Vec<(f64, f64)>> = vec![Vec::new(); geom.nodes.len()];
    let mut add = |left: f64, right: f64, x: f64| {
        let mut seen = vec![false; geom.nodes.len()];
        for y in &probes {
            let Some(node) = placement_node_at(trial, x, *y) else {
                continue;
            };
            if seen[node] {
                continue;
            }
            seen[node] = true;
            let def = &geom.nodes[node];
            bands[node].push((clamp_placement_x(def, left), clamp_placement_x(def, right)));
        }
    };
    for i in 0..xs.len() {
        add(xs[i], xs[i], xs[i]);
        if i > 0 {
            add(xs[i - 1] + 1e-7, xs[i] - 1e-7, (xs[i - 1] + xs[i]) / 2.0);
        }
    }
    for spans in &mut bands {
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for span in spans.iter().copied() {
            match merged.last_mut() {
                Some(last) if span.0 <= last.1 + 1.0 => last.1 = last.1.max(span.1),
                _ => merged.push(span),
            }
        }
        *spans = merged;
    }
    bands
}

/// The platform nodes a tower can stand on.
pub fn placeable_nodes(trial: &Trial) -> Vec<usize> {
    placement_bands(trial)
        .into_iter()
        .enumerate()
        .filter_map(|(node, spans)| (!spans.is_empty()).then_some(node))
        .collect()
}

/// The direction flag of a node on the trial path (`0` right, `1` left).
pub fn node_flag(trial: &Trial, node: usize) -> u8 {
    trial
        .path
        .iter()
        .position(|path_node| *path_node == node)
        .and_then(|index| trial.flags.get(index).copied())
        .unwrap_or(0)
}

/// Every legal tower x position, walking each band in `step` increments.
pub fn legal_slots(trial: &Trial, step: f64) -> Vec<Spot> {
    let mut out = Vec::new();
    for (node, spans) in placement_bands(trial).iter().enumerate() {
        for (a, b) in spans {
            let mut x = *a;
            while x <= *b {
                let check = validate_placement(trial, &[], node, x, 0.0);
                if check.ok && check.x == x {
                    out.push(Spot { node, x });
                }
                x += step;
            }
        }
    }
    out
}

/// Whether a portal blocks a placement on `node` at `x`.
fn portal_blocks(geom: &Geometry, node: usize, x: f64) -> bool {
    geom.portals.iter().any(|portal| {
        (portal[0] + 20.0 - x).abs() < 60.0
            && geom
                .nodes
                .get(portal[2] as usize)
                .and_then(|end| end.neighbors.first())
                .is_some_and(|neighbor| *neighbor == node)
    })
}

/// Validate a requested tower position, with the game's clamp and snap.
pub fn validate_placement(
    trial: &Trial,
    existing: &[Spot],
    node: usize,
    x: f64,
    screen_x: f64,
) -> Placement {
    let Some(geom) = geometry(trial) else {
        return Placement {
            ok: false,
            x,
            reason: "No scene geometry",
        };
    };
    let bands = placement_bands(trial);
    let Some(spans) = bands.get(node).filter(|spans| !spans.is_empty()) else {
        return Placement {
            ok: false,
            x,
            reason: "Not a selectable platform",
        };
    };
    let Some(def) = geom.nodes.get(node) else {
        return Placement {
            ok: false,
            x,
            reason: "Not a selectable platform",
        };
    };
    let mut px = clamp_placement_x(def, x);
    if !spans.iter().any(|(a, b)| px >= *a && px <= *b) {
        return Placement {
            ok: false,
            x: px,
            reason: "An earlier platform captures the placement cursor",
        };
    }
    for tower in existing {
        if tower.node == node && (tower.x - px).abs() < 61.0 {
            px = if px > tower.x {
                tower.x + 62.0
            } else {
                tower.x - 62.0
            };
        }
    }
    px = clamp_placement_x(def, px);
    let cursor_x = px - screen_x;
    let first = trial.path.first().copied().unwrap_or(usize::MAX);
    let last = trial.path.last().copied().unwrap_or(usize::MAX);
    let first_flag = trial.flags.first().copied().unwrap_or(0);
    let last_flag = trial.flags.last().copied().unwrap_or(0);
    if node == first
        && ((first_flag == 1 && cursor_x > def.length - 80.0)
            || (first_flag == 0 && cursor_x < def.x + 80.0))
    {
        return Placement {
            ok: false,
            x: px,
            reason: "Too close to the mob spawn",
        };
    }
    if node == last
        && ((last_flag == 0 && cursor_x > def.length - 80.0)
            || (last_flag == 1 && cursor_x < def.x + 80.0))
    {
        return Placement {
            ok: false,
            x: px,
            reason: "Too close to the totem exit",
        };
    }
    if existing
        .iter()
        .any(|tower| tower.node == node && (tower.x - px).abs() < 61.0)
    {
        return Placement {
            ok: false,
            x: px,
            reason: "Overlaps another tower",
        };
    }
    if portal_blocks(geom, node, px) {
        return Placement {
            ok: false,
            x: px,
            reason: "Blocked by a portal",
        };
    }
    Placement {
        ok: true,
        x: px,
        reason: "",
    }
}

/// A tower's scene coordinates.
pub fn tower_centers(trial: &Trial, node: usize, x: f64) -> Centers {
    let y = geometry(trial)
        .and_then(|geom| geom.nodes.get(node))
        .map(|def| def.y)
        .unwrap_or(0.0);
    Centers {
        wx: x - 5.5,
        wy: y - 9.5,
        cx: x - 2.0,
        cy: y - 9.5,
        facing: 1.0 - 2.0 * f64::from(node_flag(trial, node)),
    }
}

/// Whether a source tower's aura reaches a destination tower.
pub(crate) fn aura_applies(
    src_type: usize,
    dst_distance: f64,
    is_a: bool,
    range: f64,
    self_: bool,
) -> bool {
    if src_type == 5 && is_a {
        return dst_distance < range;
    }
    if self_ {
        return false;
    }
    if src_type == 4 && !is_a {
        return dst_distance < range;
    }
    (src_type == 5 || src_type == 8) && !is_a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_carry_every_trial() {
        assert_eq!(data().trials.len(), 8);
        assert_eq!(data().towers.len(), TOWER_COUNT);
        let first = trial(0).expect("trial 0");
        assert_eq!(first.map, 26);
        assert_eq!(first.path, vec![4, 3, 5]);
        assert_eq!(first.waves.len(), WAVE_CYCLE);
        assert!(geometry(first).is_some());
    }

    #[test]
    fn wave_rows_scale_with_the_cycle_tier() {
        let first = trial(0).expect("trial 0");
        let one = wave_row(first, 1);
        assert_eq!(one.wave, 1);
        assert_eq!(one.tier, 0.0);
        assert_eq!(one.count, 2.0);
        let thirty = wave_row(first, 30);
        assert_eq!(thirty.tier, 0.0);
        assert_eq!((thirty.count, thirty.skull, thirty.size), (1.0, 2.0, 4.0));
        let thirty_one = wave_row(first, 31);
        assert_eq!(thirty_one.tier, 1.0);
        assert_eq!(thirty_one.count, 3.0);
        assert_eq!(thirty_one.skull, 2.0);
    }

    #[test]
    fn the_wave_countdown_holds_bosses_and_climbs() {
        assert_eq!(next_wave_countdown(0.0), 6.0);
        assert_eq!(next_wave_countdown(6.0), 20.0);
        assert_eq!(next_wave_countdown(10.0), 9999.0);
        assert_eq!(next_wave_countdown(20.0), 9999.0);
        assert_eq!(next_wave_countdown(201.0), 120.0);
    }

    #[test]
    fn tower_stats_match_the_source_tables() {
        assert_eq!(tower_damage(0, 1, 1, 1.0), 1.0);
        assert_eq!(tower_range(0, 1, 1), 98.0);
        assert_eq!(tower_atkcd(0, 1, 1), 3.95);
        assert_eq!(new_tower_cost(0, 0), 4.0);
        assert_eq!(tower_upgrade_cost(0, 1, 1), 7.0);
        assert_eq!(atom5_bonus(&[55, 0, 0, 51, 0, 0, 0, 0, 0]), 12.0);
    }

    #[test]
    fn the_first_trial_has_placeable_bands() {
        let first = trial(0).expect("trial 0");
        let nodes = placeable_nodes(first);
        assert!(!nodes.is_empty(), "trial 0 has no placeable platform");
        let spot = placement_bands(first)[nodes[0]][0].0;
        let check = validate_placement(first, &[], nodes[0], spot, 0.0);
        assert!(check.ok, "{}", check.reason);
        assert_eq!(check.x, spot);
    }
}
