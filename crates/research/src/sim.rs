//! Research EXP, insight, level curves and forward projection.
//!
//! Formulas are ports of `js/stats/systems/w7/research-math.js` in the upstream
//! calculators, minus the save-specific multipliers this app leaves to the user.

use crate::data::game;
use crate::model::{GridState, Mag, MagKind, Observations, SimContext, Simulation};

/// Research EXP per hour, split into its factors for the dashboard.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExpRate {
    /// EXP/hour from observations with regular magnifiers.
    pub obs_base: f64,
    /// The multiplier the grid and account bonuses apply.
    pub multi: f64,
    /// `obs_base * multi`.
    pub total: f64,
}

/// The terms behind `simTotalExp`'s multiplier, in the order they are applied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExpTerms {
    /// Percentages that add together before the multiplier.
    pub additive: Vec<(String, f64)>,
    /// Factors that multiply the additive result.
    pub multipliers: Vec<(String, f64)>,
}

impl ExpTerms {
    /// The additive pool, as a percentage.
    pub fn additive_total(&self) -> f64 {
        self.additive.iter().map(|(_, value)| value).sum()
    }

    /// The product of the true multipliers.
    pub fn multiplier(&self) -> f64 {
        self.multipliers.iter().map(|(_, value)| value).product()
    }

    /// `(1 + additive / 100) * the true multipliers`.
    pub fn total(&self) -> f64 {
        (1.0 + self.additive_total() / 100.0) * self.multiplier()
    }
}

/// What the level curves say about a run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Projection {
    /// Hours until the target is met.
    pub hours: f64,
    /// Research level at the end.
    pub final_level: u32,
    /// Research EXP earned over the run.
    pub total_exp: f64,
    /// Insight levels gained over the run.
    pub insight_gained: u32,
    /// Rate samples, oldest first.
    pub samples: Vec<Sample>,
}

/// One point on the projection's rate curve.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub hours: f64,
    pub level: u32,
    pub rate: f64,
}

/// Grid points earned, spent and free at the current level.
#[derive(Clone, Copy, Debug, Default)]
pub struct GridPoints {
    pub earned: u32,
    pub spent: u32,
    pub free: u32,
}

/// A rate as `1.23M` or `845K`, for step descriptions.
pub fn format_rate(value: f64) -> String {
    if value >= 1e9 {
        format!("{:.2}B", value / 1e9)
    } else if value >= 1e6 {
        format!("{:.2}M", value / 1e6)
    } else if value >= 1e3 {
        format!("{:.1}K", value / 1e3)
    } else {
        format!("{value:.0}")
    }
}

/// The base EXP of one observation with a single magnifier.
pub fn obs_base_exp(obs: usize) -> f64 {
    let t = obs as f64;
    (4.0 + (t / 2.0 + (t / 4.0).floor())) * (1.0 + t.powf(1.0 + t / 15.0 * 0.4) / 10.0)
        + (t.powf(1.5) + 1.5 * t)
}

/// Insight EXP needed to take an observation from `level` to `level + 1`.
pub(crate) fn insight_exp_req(obs: usize, level: u32) -> f64 {
    let t = obs as f64;
    (2.0 + 0.7 * t) * (1.75 + t / 200.0).powi(level as i32) * (1.0 + t * t / 100.0) + level as f64
}

/// Research EXP needed to go from `level` to `level + 1`.
pub(crate) fn research_exp_req(level: u32, server_var_res_xp: f64) -> f64 {
    let t = level as f64;
    let scale = server_var_res_xp.max(1.0);
    10.0 * (t - 1.0).max(0.0)
        + 10.0
            * (1.0 + t.powf(1.0 + t / 10.0 * 0.4) / 10.0)
            * 1.1_f64.powf(t)
            * scale.powf((level as f64 - 20.0).max(0.0))
}

/// Magnifiers one observation holds at a research level.
pub fn mag_cap(research_level: u32) -> u32 {
    let lv = research_level;
    (1.0 + (lv / 40).min(1) as f64 + (lv / 70).min(1) as f64 + (lv / 120).min(1) as f64)
        .round()
        .min(4.0) as u32
}

/// How many observations exist at a research level.
pub fn observations_to_find(research_level: u32, found: &[bool]) -> usize {
    if research_level < 1 {
        return 0;
    }
    if !found.first().copied().unwrap_or(false) {
        return 1;
    }
    let lv = research_level;
    (5 * ((lv + 10) / 10) - lv / 20 - lv / 30 - lv / 50).min(game().observations.len() as u32)
        as usize
}

/// Is an observation usable: found, and the research level high enough for it.
pub fn observation_usable(obs: usize, research_level: u32, observations: &Observations) -> bool {
    let data = &game().observations[obs];
    observations.found.get(obs).copied().unwrap_or(false) && research_level as f64 >= data.roll_req
}

/// Grid bonus of one cell, with the shapes covering it and the all-bonus multiplier.
pub fn grid_bonus(grid: &GridState, overlay: &[i32], cell: usize, ctx: &SimContext) -> f64 {
    let game = game();
    let Some(node) = game.node_at(cell) else {
        return 0.0;
    };
    let level = grid.level(cell);
    if level == 0 {
        return 0.0;
    }
    let mut value = node.bonus_per_lv * level as f64;
    let shape = overlay.get(cell).copied().unwrap_or(-1);
    if let Some(shape) = usize::try_from(shape).ok().and_then(|s| game.shapes.get(s)) {
        value *= 1.0 + shape.bonus_pct / 100.0;
    }
    value * ctx.all_bonus_multi
}

/// Kaleidoscope neighbours: the observation grid is eight wide.
pub fn kaleidoscope_map(mags: &[Mag], slots: usize) -> Vec<u32> {
    let mut map = vec![0_u32; slots.max(1)];
    for mag in mags {
        if mag.kind != MagKind::Kaleidoscope {
            continue;
        }
        let Some(slot) = mag.slot else { continue };
        let col = slot % 8;
        if col != 7 && slot + 1 < map.len() {
            map[slot + 1] += 1;
        }
        if col != 0 {
            map[slot - 1] += 1;
        }
        if slot > 7 {
            map[slot - 8] += 1;
        }
        // The last row of the observation grid has no slot below it; the map is
        // only as wide as the observations this app models.
        if slot + 8 < map.len() {
            map[slot + 8] += 1;
        }
    }
    map
}

/// The multiplier a kaleidoscope adds, from the grid's own kaleidoscope nodes
/// and the emporium and jelly terms (`getKaleiMultiBase`).
pub fn kalei_base(grid: &GridState, overlay: &[i32], ctx: &SimContext) -> f64 {
    (30.0
        + grid_bonus(grid, overlay, 52, ctx)
        + grid_bonus(grid, overlay, 72, ctx)
        + 6.0 * ctx.emporium(46)
        + ctx.jelly_kalei_pct)
        / 100.0
}

/// Insight EXP per hour one observation earns from its monocles.
pub fn insight_rate(
    obs: usize,
    sim: &Simulation,
    overlay: &[i32],
    ctx: &SimContext,
    kal_map: &[u32],
    kal_base: f64,
) -> f64 {
    let count = sim
        .mags
        .iter()
        .filter(|mag| mag.kind == MagKind::Monocle && mag.slot == Some(obs))
        .count() as f64;
    if count == 0.0 {
        return 0.0;
    }
    let bonus = grid_bonus(&sim.grid, overlay, 92, ctx) + grid_bonus(&sim.grid, overlay, 91, ctx);
    let extra = kal_map.get(obs).copied().unwrap_or(0) as f64;
    3.0 * count
        * (1.0 + bonus / 100.0)
        * (1.0 + 35.0 * ctx.emporium(46) / 100.0)
        * (1.0 + ctx.jelly_insight_pct / 100.0)
        * (1.0 + extra * kal_base)
}

/// One term of the research AFK gains rate.
#[derive(Clone, Debug, PartialEq)]
pub struct AfkTerm {
    /// What the term is called.
    pub label: &'static str,
    /// The percentage points it adds to the AFK gains sum.
    pub value: f64,
    /// The grid square it reads, when it is a grid bonus.
    pub grid_cell: Option<usize>,
    /// The upstream system or save field behind the value.
    pub source: &'static str,
    /// Why the term is missing from the sum, when this build cannot model it.
    pub unavailable: Option<&'static str>,
}

impl AfkTerm {
    fn new(label: &'static str, value: f64, source: &'static str) -> Self {
        Self {
            label,
            value,
            grid_cell: None,
            source,
            unavailable: None,
        }
    }

    fn grid(label: &'static str, cell: usize, value: f64) -> Self {
        Self {
            grid_cell: Some(cell),
            ..Self::new(label, value, "Grid")
        }
    }
}

/// The research AFK gains rate, ported from `js/stats/defs/research-afk-gains.js`.
#[derive(Clone, Debug, PartialEq)]
pub struct AfkRate {
    /// The share of active EXP the account earns while away, 0 to 1.
    pub rate: f64,
    /// The terms that make it up.
    pub terms: Vec<AfkTerm>,
}

/// The research AFK gains rate for a full state.
///
/// `rate = min(1, 0.01 + sum / 100)`, the upstream descriptor's combine. The
/// Gambit Milestone 15 term is listed but left out of the sum: its Hole
/// measurement chain is not modelled yet, so it is reported rather than guessed.
pub fn afk_rate(sim: &Simulation, ctx: &SimContext) -> AfkRate {
    let overlay = sim.overlay();
    let grid = |cell| grid_bonus(&sim.grid, &overlay, cell, ctx);
    let companion_153 = if ctx.companion_153 {
        20.0 + 10.0 * ctx.companion_153_level2
    } else {
        0.0
    };
    let terms = vec![
        AfkTerm::new("Companion 28", ctx.companion_28, "Companion 28"),
        AfkTerm::new("Companion 153", companion_153, "Companion 153"),
        AfkTerm {
            unavailable: Some("Hole measurements"),
            ..AfkTerm::new("Gambit Milestone 15", 0.0, "Gambit")
        },
        AfkTerm::new("Minehead Floor 1", ctx.minehead(1), "Minehead"),
        AfkTerm::new("Minehead Floor 10", ctx.minehead(10), "Minehead"),
        AfkTerm::grid("Grid 71 · Powered Down Research", 71, grid(71)),
        AfkTerm::grid("Grid 111 · Research AFK Gains", 111, grid(111)),
        AfkTerm::new("Sailing artifact 36", ctx.sailing_art36, "Sailing"),
        AfkTerm::new("Card w7b11", ctx.card_w7b11, "Cards0"),
        AfkTerm::new("Gem shop 45", 2.0 * ctx.gem_shop_45, "GemItemsPurchased"),
        AfkTerm::new("Ring of Glory 4", ctx.rog(4), "Sushi"),
        AfkTerm::new("Ring of Glory 24", ctx.rog(24), "Sushi"),
    ];
    let sum: f64 = terms
        .iter()
        .filter(|term| term.unavailable.is_none())
        .map(|term| term.value)
        .sum();
    AfkRate {
        rate: (0.01 + sum / 100.0).min(1.0),
        terms,
    }
}

/// Total research EXP per hour for a full state.
///
/// A port of `simTotalExpWith`: observation EXP times the additive pool and
/// every true multiplier the save decoded.
pub fn exp_rate(sim: &Simulation, ctx: &SimContext) -> ExpRate {
    let game = game();
    let overlay = sim.overlay();
    let to_find = observations_to_find(sim.research_level, &sim.observations.found);
    let kal_map = kaleidoscope_map(&sim.mags, game.observations.len());
    let kal_base = kalei_base(&sim.grid, &overlay, ctx);
    let insight_bonus = grid_bonus(&sim.grid, &overlay, 93, ctx);

    let mut per_slot = vec![0_u32; game.observations.len()];
    for mag in &sim.mags {
        if mag.kind == MagKind::Regular
            && let Some(slot) = mag.slot
            && slot < to_find
        {
            per_slot[slot] += 1;
        }
    }

    let mut obs_total = 0.0;
    for obs in 0..to_find {
        let count = per_slot[obs];
        if count == 0 {
            continue;
        }
        let level = sim.observations.insight(obs);
        let kal_multi = 1.0 + kal_map[obs] as f64 * kal_base;
        obs_total += count as f64
            * obs_base_exp(obs)
            * (1.0 + insight_bonus * level as f64 / 100.0)
            * kal_multi;
    }

    let terms = exp_terms_with(sim, ctx, &|cell| grid_bonus(&sim.grid, &overlay, cell, ctx));
    let multi = terms.total();
    ExpRate {
        obs_base: obs_total,
        multi,
        total: obs_total * multi,
    }
}

/// The terms behind the research EXP multiplier, for the All Stats dashboard.
pub fn exp_terms(sim: &Simulation, ctx: &SimContext) -> ExpTerms {
    let overlay = sim.overlay();
    exp_terms_with(sim, ctx, &|cell| grid_bonus(&sim.grid, &overlay, cell, ctx))
}

/// A grid cell's label, as the breakdown tree names it.
fn grid_label(cell: usize) -> String {
    let name = game()
        .node_at(cell)
        .map(|node| node.name.replace('_', " "))
        .unwrap_or_else(|| format!("Grid {cell}"));
    format!("Grid {} · {name}", crate::model::coord(cell))
}

/// `simTotalExpWith`'s multiplier, split into the terms it applies. `grid`
/// resolves one cell's bonus, so the shaped scoring path can feed the same
/// formula a hypothetical overlay.
fn exp_terms_with(sim: &Simulation, ctx: &SimContext, grid: &impl Fn(usize) -> f64) -> ExpTerms {
    let to_find = observations_to_find(sim.research_level, &sim.observations.found);
    let found = sim
        .observations
        .found
        .iter()
        .take(to_find)
        .filter(|found| **found)
        .count() as f64;
    let insight: f64 = sim
        .observations
        .insight_level
        .iter()
        .take(to_find)
        .map(|level| *level as f64)
        .sum();
    let dynamic_sticker = if ctx.sticker_fixed > 0.0 {
        (1.0 + (grid(68) * ctx.boony_count + 30.0 * ctx.event_shop_37) / 100.0) * ctx.sticker_fixed
    } else {
        0.0
    };
    let sticker_multi = if ctx.sticker_multiplicative {
        1.0 + dynamic_sticker / 100.0
    } else {
        1.0
    };
    let button0 = if ctx.button_base_no_grid > 0.0 {
        ctx.button_base_no_grid * (1.0 + grid(125) / 100.0)
    } else {
        ctx.button0
    };
    let mut additive = Vec::with_capacity(9);
    for cell in [50, 90, 110, 31, 51] {
        additive.push((grid_label(cell), grid(cell)));
    }
    if !ctx.sticker_multiplicative {
        additive.push(("Sticker Secured".into(), dynamic_sticker));
    }
    additive.push(("Account bonuses".into(), ctx.extra_pct));
    additive.push(("Research Skill Mastery".into(), ctx.mastery_additive_pct));
    additive.push((
        format!("{} × {found} observations found", grid_label(112)),
        grid(112) * found,
    ));
    additive.push((
        format!("{} × {insight} insight levels", grid_label(94)),
        grid(94) * insight,
    ));
    let multipliers = vec![
        (grid_label(70), 1.0 + grid(70) / 100.0),
        (
            "Research equipment".into(),
            ctx.research_equipment_multi.clamp(1.0, 15.0),
        ),
        ("Sticker multiplier".into(), sticker_multi),
        (
            "Research Skill Mastery multiplier".into(),
            ctx.mastery_exp_multi,
        ),
        ("Clam Work promotion 9".into(), 1.0 + 0.25 * ctx.clam_work9),
        ("Nonstop Studies".into(), 1.0 + 3.0 * ctx.dream14 / 100.0),
        ("Button 0".into(), 1.0 + button0 / 100.0),
        ("Arcade 63".into(), 1.0 + ctx.arcade63 / 100.0),
        ("Killroy 5".into(), ctx.killroy5.max(1.0)),
        ("Companions 52/153/54".into(), ctx.companion_52),
        (
            "Companion 54 enhanced".into(),
            1.0 + 0.5 * ctx.companion_54_level2,
        ),
        ("Ring of Glory 0".into(), 1.0 + ctx.rog(0) / 100.0),
        ("Cglunko 11".into(), 1.0 + ctx.cglunko11 / 100.0),
        ("Fountain 2-16".into(), 1.0 + ctx.fountain_2_16 / 100.0),
        ("Jelly Operator rewards".into(), ctx.jelly_research_multi),
        ("Menagerie boost 3".into(), 1.0 + ctx.menagerie_b3 / 100.0),
        (
            "Royal Guardian research".into(),
            ctx.royal_research.max(1.0),
        ),
        ("Account multiplier".into(), ctx.extra_multiplier.max(1.0)),
    ];
    ExpTerms {
        additive,
        multipliers,
    }
}

/// `simTotalExpWith`'s multiplier: the additive pool times every true
/// multiplier.
fn total_multiplier(sim: &Simulation, ctx: &SimContext, grid: &impl Fn(usize) -> f64) -> f64 {
    exp_terms_with(sim, ctx, grid).total()
}

fn shaped_rate_with_overlay(
    sim: &Simulation,
    ctx: &SimContext,
    overlay: &[i32],
    bonus_pct: f64,
) -> f64 {
    let game = game();
    let to_find = observations_to_find(sim.research_level, &sim.observations.found);
    let kal_map = kaleidoscope_map(&sim.mags, game.observations.len());
    let kal_base = kalei_base(&sim.grid, overlay, ctx);
    let insight_bonus = grid_bonus_with(&sim.grid, overlay, 93, ctx, bonus_pct);

    let mut per_slot = vec![0_u32; game.observations.len()];
    for mag in &sim.mags {
        if mag.kind == MagKind::Regular
            && let Some(slot) = mag.slot
            && slot < to_find
        {
            per_slot[slot] += 1;
        }
    }
    let mut obs_total = 0.0;
    for obs in 0..to_find {
        if per_slot[obs] == 0 {
            continue;
        }
        let level = sim.observations.insight(obs);
        obs_total += per_slot[obs] as f64
            * obs_base_exp(obs)
            * (1.0 + insight_bonus * level as f64 / 100.0)
            * (1.0 + kal_map[obs] as f64 * kal_base);
    }

    let multi = total_multiplier(sim, ctx, &|cell| {
        grid_bonus_with(&sim.grid, overlay, cell, ctx, bonus_pct)
    });
    obs_total * multi
}

/// Grid bonus under an explicit overlay and one shape bonus, used while scoring.
fn grid_bonus_with(
    grid: &GridState,
    overlay: &[i32],
    cell: usize,
    ctx: &SimContext,
    bonus_pct: f64,
) -> f64 {
    let Some(node) = game().node_at(cell) else {
        return 0.0;
    };
    let level = grid.level(cell);
    if level == 0 {
        return 0.0;
    }
    let mut value = node.bonus_per_lv * level as f64;
    if overlay.get(cell).copied().unwrap_or(-1) >= 0 {
        value *= 1.0 + bonus_pct / 100.0;
    }
    value * ctx.all_bonus_multi
}

/// EXP/hour added by putting a shape on each cell, at the 25% baseline bonus.
pub fn cell_values(sim: &Simulation, ctx: &SimContext) -> Vec<f64> {
    let game = game();
    let base = exp_rate(sim, ctx).total;
    let mut overlay = sim.overlay();
    let mut values = vec![0.0; game.cells()];
    for cell in 0..game.cells() {
        if game.node_at(cell).is_none() || sim.grid.level(cell) == 0 || overlay[cell] >= 0 {
            continue;
        }
        overlay[cell] = 0;
        let shaped = shaped_rate_with_overlay(sim, ctx, &overlay, 25.0);
        overlay[cell] = -1;
        values[cell] = shaped - base;
    }
    values
}

/// The value of putting a 25% shape on each cell as if no shape covered the
/// grid: the upstream `bareSO` scoring shapes and the tier list both read.
pub fn bare_cell_values(sim: &Simulation, ctx: &SimContext) -> Vec<f64> {
    let mut bare = sim.clone();
    bare.placements.clear();
    bare.saved_overlay = Some(vec![-1; game().cells()]);
    cell_values(&bare, ctx)
}

/// Grid points earned at a research level.
pub(crate) fn grid_points_earned(research_level: u32, points_node_level: u32, bonus: u32) -> u32 {
    let lv = research_level as f64;
    let sq50 = points_node_level as f64;
    (lv + bonus as f64 + (lv / 10.0).floor() * (1.0 + (lv / 60.0).floor().min(1.0) + sq50).round())
        .floor() as u32
}

/// Grid points earned, spent and free.
pub fn grid_points(sim: &Simulation, ctx: &SimContext) -> GridPoints {
    let earned = grid_points_earned(
        sim.research_level,
        sim.grid.level(50),
        ctx.bonus_grid_points,
    );
    let spent = sim.grid.spent();
    GridPoints {
        earned,
        spent,
        free: earned.saturating_sub(spent),
    }
}

/// Shapes owned, as `computeShapesOwnedAt` counts them.
pub fn shapes_owned(research_level: u32, ctx: &SimContext) -> usize {
    let lv = research_level as f64;
    let owned = if ctx.bundle_ban_m { 1.0 } else { 0.0 }
        + ctx.event_shop.get(36).copied().unwrap_or(0) as f64
        + (0.0f64.max(
            (lv / 20.0).floor()
                * if ctx.companion_54_level2 > 0.0 {
                    1.0
                } else {
                    0.0
                },
        ))
        .min(1.0)
        + (0.0f64.max((lv / 20.0).floor() * if ctx.spellunky_upg7 >= 1.0 { 1.0 } else { 0.0 }))
            .min(1.0)
        + (lv / 20.0).floor().min(1.0)
        + (lv / 30.0).floor().min(1.0)
        + (lv / 50.0).floor().min(1.0)
        + (lv / 80.0).floor().min(1.0)
        + (lv / 110.0).floor().min(1.0)
        + ctx.extra_shapes as f64;
    owned.round().min(game().shapes.len() as f64) as usize
}

/// Magnifiers owned, as `computeMagnifiersOwnedWith` counts them.
pub fn magnifiers_owned(sim: &Simulation, ctx: &SimContext) -> u32 {
    let lv = sim.research_level;
    let kaleidoscopes = (sim.grid.level(72) as f64 + ctx.event_shop(33)).round();
    let monocles = sim.grid.level(91) as f64;
    let level_bonus = (lv / 10).min(1) + (lv / 100).min(1) + (lv / 130).min(1) + (lv / 140).min(1);
    let owned = 1.0
        + kaleidoscopes
        + monocles
        + ctx.minehead(2)
        + ctx.minehead(12)
        + ctx.minehead(20)
        + ctx.event_shop(34)
        + level_bonus as f64
        + if ctx.companion_153 { 1.0 } else { 0.0 }
        + ctx.rog(8)
        + ctx.extra_magnifiers as f64;
    owned.round().min(80.0) as u32
}

/// Rotations unlock at research level 90.
pub fn can_rotate(research_level: u32) -> bool {
    research_level >= 90
}

/// Advance a level while the accumulated EXP covers the next requirement.
///
/// The `1e-9` slack and the clamp are upstream `advanceResearchLevel`'s: a jump
/// that lands exactly on the requirement can undershoot by a rounding error, and
/// without the slack the run would spend its next jumps crawling the residual.
pub(crate) fn advance_level(level: &mut u32, exp: &mut f64, server_var_res_xp: f64) {
    while research_level_affordable(*level, *exp, server_var_res_xp) {
        *exp = (*exp - research_exp_req(*level, server_var_res_xp)).max(0.0);
        *level += 1;
    }
}

fn research_level_affordable(level: u32, exp: f64, server_var_res_xp: f64) -> bool {
    exp >= research_exp_req(level, server_var_res_xp) - 1e-9
}

/// Upstream's daily tournament window: 12h of research time per registration.
pub(crate) const TOURNAMENT_BONUS_HOURS: f64 = 12.0;
/// How many hours apart two tournament registrations are.
pub(crate) const TOURNAMENT_INTERVAL_HOURS: f64 = 24.0;
/// Register half an hour before the daily reset, at peak rates.
const TOURNAMENT_BUFFER_HOURS: f64 = 0.5;
/// The reset happens when `(GlobalTime + 14400) % 86400 == 0`, i.e. 20:00 UTC.
const TOURNAMENT_RESET_OFFSET: f64 = 14_400.0;
const DAY_SECONDS: f64 = 86_400.0;

/// Hours until the first tournament registration fires.
///
/// A port of the `_firstTournamentHrs` block in upstream `unifiedSim`: from the
/// save's `GlobalTime`, minus the half-hour buffer, skipped a day when the save
/// already registered in the cached tournament day. Without a save clock the
/// fallback is one full day, as upstream.
pub(crate) fn first_tournament_hours(ctx: &SimContext) -> f64 {
    if ctx.save_global_time <= 0.0 {
        return TOURNAMENT_INTERVAL_HOURS;
    }
    let shifted = ctx.save_global_time + TOURNAMENT_RESET_OFFSET;
    let seconds = DAY_SECONDS - shifted.rem_euclid(DAY_SECONDS);
    let mut first = (seconds / 3600.0 - TOURNAMENT_BUFFER_HOURS).max(0.0);
    if first < 1.0 / 60.0 {
        first += TOURNAMENT_INTERVAL_HOURS;
    }
    if ctx.tournament_cached_day > 0.0 && ctx.tournament_last_day >= ctx.tournament_cached_day + 1.0
    {
        first += TOURNAMENT_INTERVAL_HOURS;
    }
    first
}

/// Mark every observation whose research requirement is met as found.
pub(crate) fn unlock_observations(state: &mut Simulation) {
    let level = state.research_level as f64;
    for (obs, found) in state.observations.found.iter_mut().enumerate() {
        if !*found && level >= game().observations[obs].roll_req {
            *found = true;
        }
    }
}

/// Add `hours` of insight collection to every observation with a monocle.
///
/// One level per observation, as upstream `advanceInsightLevels` reads its
/// single `if`; the tournament bonus is one collection event, not a loop.
pub(crate) fn advance_insight_hours(state: &mut Simulation, ctx: &SimContext, hours: f64) -> u32 {
    let overlay = state.overlay();
    let kal_map = kaleidoscope_map(&state.mags, game().observations.len());
    let kal_base = kalei_base(&state.grid, &overlay, ctx);
    let mut gained = 0;
    for obs in 0..game().observations.len() {
        let rate = insight_rate(obs, state, &overlay, ctx, &kal_map, kal_base);
        if rate <= 0.0 {
            continue;
        }
        state.observations.insight_exp[obs] += rate * hours;
        let req = insight_exp_req(obs, state.observations.insight_level[obs]);
        if state.observations.insight_exp[obs] >= req - 1e-9 {
            state.observations.insight_exp[obs] =
                (state.observations.insight_exp[obs] - req).max(0.0);
            state.observations.insight_level[obs] += 1;
            gained += 1;
        }
    }
    gained
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mag, MagKind};

    fn filled() -> (Simulation, SimContext) {
        let mut sim = Simulation {
            research_level: 60,
            grid: GridState::seeded(2),
            observations: Observations::all_at(0),
            ..Simulation::default()
        };
        sim.grid.set(50, 1);
        let ctx = SimContext::default();
        (sim, ctx)
    }

    #[test]
    fn observation_curve_matches_the_upstream_values() {
        assert!((obs_base_exp(0) - 4.0).abs() < 1e-9);
        // Upstream: (4 + (t/2 + floor(t/4))) * (1 + t^(1+t/15*0.4)/10) + t^1.5 + 1.5t
        let t = 10.0_f64;
        let expected = (4.0 + (t / 2.0 + (t / 4.0).floor()))
            * (1.0 + t.powf(1.0 + t / 15.0 * 0.4) / 10.0)
            + t.powf(1.5)
            + 1.5 * t;
        assert!((obs_base_exp(10) - expected).abs() < 1e-9);
        assert!(obs_base_exp(30) > obs_base_exp(10));
    }

    #[test]
    fn research_curve_grows_per_level() {
        assert!(research_exp_req(1, 1.0) < research_exp_req(2, 1.0));
        assert!(research_exp_req(30, 1.0) > research_exp_req(29, 1.0));
        assert!(research_exp_req(30, 2.0) > research_exp_req(30, 1.0));
    }

    #[test]
    fn a_kaleidoscope_on_the_last_row_has_no_southern_neighbour() {
        let last = game().observations.len() - 1;
        let map = kaleidoscope_map(&[Mag::new(MagKind::Kaleidoscope, Some(last))], last + 1);
        assert_eq!(map.len(), last + 1);
        assert_eq!(map[last], 0, "the bottom row must not boost itself");
        assert_eq!(map[last - 1], 1, "its left neighbour is boosted");
        assert_eq!(map[last - 8], 1, "its northern neighbour is boosted");
        // A slot one past the data must also be safe.
        let wide = kaleidoscope_map(&[Mag::new(MagKind::Kaleidoscope, Some(41))], 43);
        assert_eq!(wide[41], 0);
    }

    #[test]
    fn magnifier_caps_follow_the_level_gates() {
        assert_eq!(mag_cap(1), 1);
        assert_eq!(mag_cap(40), 2);
        assert_eq!(mag_cap(70), 3);
        assert_eq!(mag_cap(120), 4);
    }

    #[test]
    fn exp_rate_needs_magnifiers() {
        let (mut sim, ctx) = filled();
        assert_eq!(exp_rate(&sim, &ctx).total, 0.0);
        sim.mags = vec![Mag::new(MagKind::Regular, Some(3))];
        let rate = exp_rate(&sim, &ctx);
        assert!(rate.obs_base > 0.0);
        assert!(rate.multi >= 1.0);
        assert!(rate.total > 0.0);
    }

    #[test]
    fn shapes_and_bonus_nodes_raise_the_rate() {
        let (mut sim, ctx) = filled();
        sim.mags = vec![Mag::new(MagKind::Regular, Some(3))];
        let plain = exp_rate(&sim, &ctx).total;
        let mut values = cell_values(&sim, &ctx);
        let best = values
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(cell, _)| cell)
            .expect("a cell");
        assert!(values[best] > 0.0, "some cell must be worth shaping");
        values.clear();
        let mut shaped = sim.clone();
        shaped.placements = vec![crate::model::Placement {
            shape: 3,
            x: 0.0,
            y: 0.0,
            rot: 0.0,
        }];
        let with_shape = exp_rate(&shaped, &ctx).total;
        assert!(with_shape >= plain);
    }

    #[test]
    fn grid_points_count_what_the_level_grants() {
        let (sim, ctx) = filled();
        let points = grid_points(&sim, &ctx);
        assert!(points.earned > 0);
        assert_eq!(points.spent, sim.grid.spent());
        assert_eq!(points.free, points.earned.saturating_sub(points.spent));
    }

    #[test]
    fn the_afk_rate_starts_at_one_percent_and_sums_its_terms() {
        let (mut sim, mut ctx) = filled();
        sim.grid = crate::model::GridState::empty();
        let bare = afk_rate(&sim, &ctx);
        assert!((bare.rate - 0.01).abs() < 1e-12);
        assert!(
            bare.terms
                .iter()
                .any(|term| term.label == "Gambit Milestone 15" && term.unavailable.is_some())
        );
        ctx.companion_28 = 30.0;
        ctx.minehead = vec![0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0];
        ctx.sailing_art36 = 5.0;
        ctx.card_w7b11 = 1.0;
        ctx.gem_shop_45 = 10.0;
        let rate = afk_rate(&sim, &ctx);
        let sum: f64 = rate
            .terms
            .iter()
            .filter(|term| term.unavailable.is_none())
            .map(|term| term.value)
            .sum();
        assert!((rate.rate - (0.01 + sum / 100.0).min(1.0)).abs() < 1e-12);
        let term = |label: &str| {
            rate.terms
                .iter()
                .find(|term| term.label == label)
                .map(|term| term.value)
        };
        assert_eq!(term("Companion 28"), Some(30.0));
        assert_eq!(term("Minehead Floor 1"), Some(2.0));
        assert_eq!(term("Minehead Floor 10"), Some(3.0));
        assert_eq!(term("Sailing artifact 36"), Some(5.0));
        assert_eq!(term("Card w7b11"), Some(1.0));
        assert_eq!(term("Gem shop 45"), Some(20.0));
    }

    #[test]
    fn the_user_supplements_reach_the_rate_and_the_owned_pools() {
        let (mut sim, mut ctx) = filled();
        sim.mags = vec![Mag::new(MagKind::Regular, Some(3))];
        let plain = exp_rate(&sim, &ctx).total;
        let shapes = shapes_owned(sim.research_level, &ctx);
        let magnifiers = magnifiers_owned(&sim, &ctx);
        let earned = grid_points(&sim, &ctx).earned;

        ctx.extra_multiplier = 2.0;
        assert!((exp_rate(&sim, &ctx).total - plain * 2.0).abs() < 1e-6);
        ctx.extra_multiplier = 1.0;
        ctx.extra_pct = 100.0;
        assert!(exp_rate(&sim, &ctx).total > plain);
        ctx.extra_shapes = 2;
        assert_eq!(shapes_owned(sim.research_level, &ctx), shapes + 2);
        ctx.extra_magnifiers = 2;
        assert_eq!(magnifiers_owned(&sim, &ctx), magnifiers + 2);
        ctx.bonus_grid_points = 10;
        assert_eq!(grid_points(&sim, &ctx).earned, earned + 10);
    }

    #[test]
    fn the_afk_rate_caps_at_one() {
        let (sim, mut ctx) = filled();
        ctx.companion_28 = 1_000.0;
        assert_eq!(afk_rate(&sim, &ctx).rate, 1.0);
    }

    #[test]
    fn the_first_tournament_lands_half_an_hour_before_the_reset() {
        let mut ctx = SimContext {
            // Ten days in, at 19:00 UTC: one hour until the 20:00 reset.
            save_global_time: 86_400.0 * 10.0 + 19.0 * 3_600.0,
            ..SimContext::default()
        };
        assert!((first_tournament_hours(&ctx) - 0.5).abs() < 1e-9);

        ctx.tournament_cached_day = 100.0;
        ctx.tournament_last_day = 101.0;
        assert!((first_tournament_hours(&ctx) - 24.5).abs() < 1e-9);

        ctx.save_global_time = 0.0;
        assert_eq!(first_tournament_hours(&ctx), 24.0);
    }
}
