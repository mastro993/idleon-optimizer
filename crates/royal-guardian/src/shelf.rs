//! The Armory shelf planner: the steps that unlock the next shelf, in the order
//! a player takes them.
//!
//! Ported from `planNextArmoryShelf`, `_armoryShelfCandidates`,
//! `_shelfLayoutStrategy`, `_connectPortfolioOutposts`, `_resourceProfessionState`,
//! `_assignIdleOutpostsToRanks`, `_applyArmoryShelfPurchase` and `_advanceTime` in
//! `js/stats/systems/w7/royal-guardian-planner.js`.
//!
//! A shelf unlocks when the account's Armory levels reach the shelf's research
//! order. Reaching it means buying levels with the currencies the resource nodes
//! pay, so the plan alternates between two kinds of step: a layout step that puts
//! the outposts on the nodes whose currency an eligible shelf costs, and a
//! purchase step that spends the balance. The simulation advances through node
//! depletions and the daily reset (Armory 70 refills a collected node, Armory 0
//! raises its grade) so the projected times match what the account would see.
//!
//! Upstream's layout strategy scores a currency portfolio across every eligible
//! shelf and beams over profession splits. The port keeps the deterministic part
//! of it: minimum Guards for reach, the rest Workers, the focused currency first
//! and the shortest projected drain to break ties, and Trader/Surveyor rank
//! fallbacks for outposts with nothing left to drain. The plan's own steps are
//! replayed through [`sandbox::apply`], so a layout step only ever contains moves
//! the game accepts and a purchase step only ever spends a balance the plan has.

use super::*;
use idleon_common::{Control, Progress};

/// The shelf whose purchase switches the whole kingdom's assignments; upstream
/// excludes it until it models the change.
const EXCLUDED_SHELF: usize = 68;
/// The hours after the saved countdown the next reset lands at.
const RESET_INTERVAL_HOURS: f64 = 24.0;
/// How far the projection may run before the shelf is called unreachable.
const HORIZON_HOURS: f64 = 24.0 * 365.0;
/// How many plan steps a loop may take before the projection stops.
const MAX_STEPS: usize = 64;

/// What one planning run spends at most.
#[derive(Clone, Debug, PartialEq)]
pub struct ShelfRequest {
    pub royal: RoyalGuardian,
    /// How many level purchases the plan may make.
    pub purchases: usize,
}

impl ShelfRequest {
    /// A run over a saved state, buying up to `purchases` levels.
    pub fn new(royal: &RoyalGuardian, purchases: usize) -> Self {
        Self {
            royal: royal.clone(),
            purchases,
        }
    }
}

/// One step the plan takes.
#[derive(Clone, Debug, PartialEq)]
pub enum ShelfAction {
    /// Move the outposts onto the nodes the shelf's currency comes from.
    Layout {
        /// `currency-portfolio` or `rank`.
        mode: &'static str,
        moves: Vec<Move>,
    },
    /// Buy one Armory level.
    Purchase {
        index: usize,
        order: usize,
        name: &'static str,
        currency_slot: i64,
        cost: f64,
        level_before: f64,
        level_after: f64,
        balance_after: f64,
    },
    /// The daily reset refilled the nodes the plan collected.
    Reset { refills: usize, grade_gains: usize },
    /// The nodes paid their currency over the hours the plan waited.
    Collect {
        hours: f64,
        /// What each currency slot gained, by slot.
        currency: Vec<(i64, f64)>,
    },
}

/// One step, at the hour the plan reaches it.
#[derive(Clone, Debug, PartialEq)]
pub struct ShelfStep {
    pub time_hours: f64,
    pub action: ShelfAction,
}

impl ShelfStep {
    /// The line a step list prints.
    pub fn label(&self) -> String {
        match &self.action {
            ShelfAction::Layout { mode, moves } => {
                format!("{} layout: {} changes", mode, moves.len())
            }
            ShelfAction::Purchase {
                name,
                cost,
                level_after,
                ..
            } => format!("Buy {name} to level {level_after} for {cost:.0}"),
            ShelfAction::Reset {
                refills,
                grade_gains,
            } => format!("Reset: {refills} refills, {grade_gains} grades"),
            ShelfAction::Collect { hours, currency } => {
                let gained: f64 = currency.iter().map(|(_, amount)| *amount).sum();
                format!("Collect {gained:.0} over {hours:.1} hours")
            }
        }
    }
}

/// The plan toward the next shelf.
#[derive(Clone, Debug, PartialEq)]
pub struct ShelfPlan {
    pub start_unlocked: usize,
    pub next_unlocked: usize,
    /// The Armory levels the next shelf still needs.
    pub levels_to_next_threshold: usize,
    pub purchases_needed: usize,
    pub complete: bool,
    pub eta_hours: f64,
    pub steps: Vec<ShelfStep>,
    /// Why the plan stopped short, when it did.
    pub reason: Option<String>,
    /// The state the steps reach.
    pub final_state: RoyalGuardian,
    pub missing: Vec<String>,
}

impl RoyalGuardian {
    /// Plan the purchases that unlock the next Armory shelf: upstream
    /// `planNextArmoryShelf`, with the purchases reported through `report`.
    /// Returns `None` when the caller stops the run.
    pub fn plan_shelf(
        &self,
        request: &ShelfRequest,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<ShelfPlan> {
        let start_unlocked = self.armory_unlocked_count();
        let thresholds = thresholds();
        let Some(next_threshold) = thresholds.get(start_unlocked).copied() else {
            // Every shelf is unlocked: there is no next one to plan for.
            return Some(ShelfPlan {
                start_unlocked,
                next_unlocked: start_unlocked,
                levels_to_next_threshold: 0,
                purchases_needed: 0,
                complete: true,
                eta_hours: 0.0,
                steps: Vec::new(),
                reason: None,
                final_state: self.clone(),
                missing: self.missing_sources().to_vec(),
            });
        };
        let levels_to_next = (next_threshold - self.armory_total_levels())
            .max(0.0)
            .ceil() as usize;
        let cap = request.purchases.clamp(1, 50).min(levels_to_next.max(1));
        let mut sandbox = Sandbox::new(self);
        let mut steps = Vec::new();
        let mut purchases = 0usize;
        let mut hours = 0.0f64;
        let mut reason = None;
        let mut iterations = 0usize;
        while purchases < cap && sandbox.state().armory_unlocked_count() == start_unlocked {
            iterations += 1;
            if iterations > MAX_STEPS {
                reason = Some("the plan did not settle".to_owned());
                break;
            }
            let (layout, moves) = shelf_layout(&sandbox);
            if !moves.is_empty() {
                steps.push(ShelfStep {
                    time_hours: hours,
                    action: ShelfAction::Layout {
                        mode: layout_mode(&moves),
                        moves,
                    },
                });
                sandbox = layout;
            }
            let candidates = shelf_candidates(sandbox.state());
            let Some(candidate) = candidates.first().cloned() else {
                reason = Some(
                    if sandbox.state().armory_level(EXCLUDED_SHELF)
                        < data::armory(EXCLUDED_SHELF)
                            .map(|shelf| shelf.max_level)
                            .unwrap_or(0.0)
                    {
                        "the next level needs Kingdom Sovereignty, which the plan excludes"
                            .to_owned()
                    } else {
                        "no eligible Armory shelf can be bought".to_owned()
                    },
                );
                break;
            };
            if candidate.hours <= f64::EPSILON {
                if let Err(errors) = buy(&mut sandbox, &candidate) {
                    reason = Some(errors.join("; "));
                    break;
                }
                steps.push(ShelfStep {
                    time_hours: hours,
                    action: ShelfAction::Purchase {
                        index: candidate.index,
                        order: candidate.order,
                        name: candidate.name,
                        currency_slot: candidate.currency_slot,
                        cost: candidate.cost,
                        level_before: candidate.level,
                        level_after: candidate.level + 1.0,
                        balance_after: sandbox
                            .state()
                            .g_cell(row::CURRENCIES, candidate.currency_slot.max(0) as usize),
                    },
                });
                purchases += 1;
                report(Progress {
                    fraction: (purchases as f32 / cap.max(1) as f32).min(1.0),
                    message: format!("Bought {} of {cap} levels", purchases),
                });
                continue;
            }
            if !candidate.hours.is_finite() {
                reason = Some("no reachable node pays this shelf's currency right now".to_owned());
                break;
            }
            let advance = advance(&mut sandbox, candidate.hours);
            for event in advance.events {
                steps.push(ShelfStep {
                    time_hours: hours + event.hours,
                    action: ShelfAction::Reset {
                        refills: event.refills,
                        grade_gains: event.grade_gains,
                    },
                });
            }
            hours += advance.hours;
            let currency: Vec<(i64, f64)> = advance.currency.into_iter().collect();
            if !currency.is_empty() {
                steps.push(ShelfStep {
                    time_hours: hours,
                    action: ShelfAction::Collect {
                        hours: advance.hours,
                        currency,
                    },
                });
            }
            if hours > HORIZON_HOURS {
                reason = Some("the shelf takes longer than a year to reach".to_owned());
                break;
            }
            if report(Progress {
                fraction: (hours / candidate.hours.max(1.0)).min(1.0) as f32,
                message: format!("Projecting {:.1} hours to the next purchase", hours),
            }) == Control::Stop
            {
                return None;
            }
        }
        let next_unlocked = sandbox.state().armory_unlocked_count();
        let complete = next_unlocked > start_unlocked;
        if !complete && reason.is_none() {
            reason = Some(
                if purchases >= cap {
                    "the purchase limit stopped the plan"
                } else {
                    "the next shelf could not be unlocked"
                }
                .to_owned(),
            );
        }
        let mut missing = sandbox.state().missing_sources().to_vec();
        if !sandbox.state().reset_timing().available {
            missing.push("TimeAway.ShopRestock".to_owned());
        }
        Some(ShelfPlan {
            start_unlocked,
            next_unlocked,
            levels_to_next_threshold: levels_to_next,
            purchases_needed: purchases,
            complete,
            eta_hours: hours,
            steps,
            reason: if complete { None } else { reason },
            final_state: sandbox.into_state(),
            missing,
        })
    }
}

impl ShelfPlan {
    /// Replays the plan's steps on a saved state, in order, and returns the state
    /// they reach. Every layout move goes through [`sandbox::apply`] and every
    /// purchase through the same spend the plan used, so a plan that cannot be
    /// replayed reports why instead of silently differing.
    pub fn replay(&self, royal: &RoyalGuardian) -> Result<Sandbox, Vec<String>> {
        let mut sandbox = Sandbox::new(royal);
        for step in &self.steps {
            match &step.action {
                ShelfAction::Layout { moves, .. } => {
                    for change in moves {
                        sandbox = sandbox::apply(&sandbox, change)?;
                    }
                }
                ShelfAction::Purchase {
                    index,
                    order,
                    currency_slot,
                    cost,
                    level_before,
                    name,
                    ..
                } => {
                    let candidate = Candidate {
                        index: *index,
                        order: *order,
                        name,
                        currency_slot: *currency_slot,
                        cost: *cost,
                        level: *level_before,
                        balance: 0.0,
                        rate: 0.0,
                        hours: 0.0,
                        efficiency: 0.0,
                    };
                    buy(&mut sandbox, &candidate)?;
                }
                ShelfAction::Reset { .. } => {}
                ShelfAction::Collect { currency, .. } => {
                    for (slot, amount) in currency {
                        let balance = sandbox
                            .state()
                            .g_cell(row::CURRENCIES, (*slot).max(0) as usize);
                        sandbox.set_currency(*slot, balance + amount);
                    }
                }
            }
        }
        Ok(sandbox)
    }
}

/// The Armory totals the shelves unlock at, in purchase order: upstream
/// `planNextArmoryShelf`'s `effectiveThresholds`.
fn thresholds() -> Vec<f64> {
    let mut values: Vec<f64> = data::data()
        .armory
        .iter()
        .map(|shelf| shelf.research_order)
        .filter(|value| value.is_finite())
        .collect();
    values.sort_by(|left, right| left.total_cmp(right));
    values.truncate(data::armory_order().len());
    values
}

/// One shelf a purchase could reach.
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    index: usize,
    order: usize,
    name: &'static str,
    currency_slot: i64,
    cost: f64,
    /// The shelf's saved level.
    level: f64,
    balance: f64,
    /// The currency the layout pays per hour.
    rate: f64,
    /// Hours until the balance covers the price.
    hours: f64,
    /// Currency per hour divided by the price.
    efficiency: f64,
}

/// Every shelf the plan could buy next: upstream `_armoryShelfCandidates`.
fn shelf_candidates(royal: &RoyalGuardian) -> Vec<Candidate> {
    let unlocked = royal.armory_unlocked_count();
    let income = currency_rates(royal);
    let mut candidates = Vec::new();
    for order in 0..unlocked {
        let Some(index) = data::armory_order().get(order).map(|index| *index as usize) else {
            continue;
        };
        let Some(shelf) = data::armory(index) else {
            continue;
        };
        let level = royal.armory_level(index);
        if level >= shelf.max_level || index == EXCLUDED_SHELF {
            continue;
        }
        let cost = royal.armory_upgrade_cost(order);
        if !cost.value.is_finite() || cost.currency_slot < 0 {
            continue;
        }
        let balance = royal.g_cell(row::CURRENCIES, cost.currency_slot as usize);
        let rate = income.get(&cost.currency_slot).copied().unwrap_or(0.0);
        let hours = if balance >= cost.value {
            0.0
        } else if rate > 0.0 {
            (cost.value - balance) / rate
        } else {
            f64::INFINITY
        };
        candidates.push(Candidate {
            index,
            order,
            name: shelf.name.as_str(),
            currency_slot: cost.currency_slot,
            cost: cost.value,
            level,
            balance,
            rate,
            hours,
            efficiency: if cost.value > f64::EPSILON {
                rate / cost.value
            } else {
                f64::INFINITY
            },
        });
    }
    candidates.sort_by(|left, right| {
        left.hours
            .total_cmp(&right.hours)
            .then_with(|| right.efficiency.total_cmp(&left.efficiency))
            .then_with(|| left.cost.total_cmp(&right.cost))
            .then_with(|| left.order.cmp(&right.order))
    });
    candidates
}

/// The currency every normal drain pays per hour.
fn currency_rates(royal: &RoyalGuardian) -> BTreeMap<i64, f64> {
    let (streams, _) = royal.resource_streams();
    let mut rates = BTreeMap::new();
    for stream in streams {
        if stream.currency_rate > 0.0 && !stream.savage {
            *rates.entry(stream.currency_slot).or_insert(0.0) += stream.currency_rate;
        }
    }
    rates
}

/// The layout the plan keeps while it saves: every Normal outpost on a node worth
/// draining, and anything with nothing left on rank EXP.
fn shelf_layout(royal: &Sandbox) -> (Sandbox, Vec<Move>) {
    let mut sandbox = royal.clone();
    let mut moves = Vec::new();
    let candidate_slots: Vec<i64> = shelf_candidates(royal.state())
        .iter()
        .map(|candidate| candidate.currency_slot)
        .collect();
    let focus = candidate_slots.first().copied();
    for map in 0..sandbox.state().maps.len() {
        if !sandbox.state().outpost(map).built || sandbox.state().outpost_type(map) != 0.0 {
            continue;
        }
        let (connected, changed) = connect_outpost(&sandbox, map, &candidate_slots, focus);
        sandbox = connected;
        moves.extend(changed);
    }
    let (idle, changed) = idle_to_ranks(&sandbox);
    sandbox = idle;
    moves.extend(changed);
    (sandbox, moves)
}

/// Puts one Normal outpost on the best node its reach allows: upstream
/// `_connectPortfolioOutposts`, narrowed to one outpost at a time.
fn connect_outpost(
    royal: &Sandbox,
    map: usize,
    slots: &[i64],
    focus: Option<i64>,
) -> (Sandbox, Vec<Move>) {
    let mut best: Option<(Sandbox, Vec<Move>, bool, f64, f64)> = None;
    let units = royal.state().units(map).len();
    for guards in 0..=units {
        let (professions, profession_moves) = professions(royal, map, guards);
        for node in professions.state().reachable_resources(map) {
            if !node.reachable || professions.state().resource_remaining(node.resource) <= 0.0 {
                continue;
            }
            let slot = resource_currency(node.resource);
            if !slots.contains(&slot) {
                continue;
            }
            let production = professions
                .state()
                .resource_production_with(
                    map,
                    node.resource,
                    &professions.state().collection_inputs(),
                )
                .value;
            let focused = focus == Some(slot);
            let gain =
                production * (1.0 + professions.state().resource_grade(node.resource) * 0.25);
            let drain_hours =
                professions.state().resource_remaining(node.resource) / gain.max(1e-9);
            let better = match &best {
                None => true,
                Some((_, _, best_focus, best_gain, best_hours)) => {
                    (*best_focus, *best_gain, *best_hours).partial_cmp(&(
                        focused,
                        gain,
                        -drain_hours,
                    )) == Some(std::cmp::Ordering::Less)
                }
            };
            if better {
                let saved = professions
                    .state()
                    .outpost_connections(map)
                    .first()
                    .map(|connection| connection.endpoint)
                    .unwrap_or(Endpoint::Empty);
                if saved != Endpoint::Resource(node.resource) {
                    let change = Move::Connection {
                        map,
                        endpoint: Endpoint::Resource(node.resource),
                    };
                    let Ok(next) = sandbox::apply(&professions, &change) else {
                        continue;
                    };
                    let mut moves = profession_moves.clone();
                    moves.push(change);
                    best = Some((next, moves, focused, gain, drain_hours));
                } else {
                    best = Some((
                        professions.clone(),
                        profession_moves.clone(),
                        focused,
                        gain,
                        drain_hours,
                    ));
                }
            }
        }
    }
    match best {
        Some((sandbox, moves, _, _, _)) => (sandbox, moves),
        None => (royal.clone(), Vec::new()),
    }
}

/// The profession split one Guard count leaves: upstream `_resourceProfessionState`.
fn professions(royal: &Sandbox, map: usize, guards: usize) -> (Sandbox, Vec<Move>) {
    let mut sandbox = royal.clone();
    let mut moves = Vec::new();
    let units = sandbox.state().units(map);
    for (index, (slot, profession)) in units.into_iter().enumerate() {
        let target = if index < guards { 2 } else { 0 };
        if profession == target {
            continue;
        }
        let change = Move::Profession {
            map,
            slot,
            profession: target,
        };
        let Ok(next) = sandbox::apply(&sandbox, &change) else {
            continue;
        };
        sandbox = next;
        moves.push(change);
    }
    (sandbox, moves)
}

/// Moves the units of outposts with nothing left to drain onto rank EXP:
/// upstream `_assignIdleOutpostsToRanks`, narrowed to a deterministic split.
fn idle_to_ranks(royal: &Sandbox) -> (Sandbox, Vec<Move>) {
    let mut sandbox = royal.clone();
    let mut moves = Vec::new();
    let (streams, _) = sandbox.state().resource_streams();
    let mut draining = vec![false; sandbox.state().maps.len()];
    for stream in &streams {
        if sandbox.state().resource_remaining(stream.resource) > 0.0
            && sandbox.state().outpost_connections(stream.map)[0].endpoint
                == Endpoint::Resource(stream.resource)
        {
            draining[stream.map] = true;
        }
    }
    for (map, draining) in draining.iter().enumerate() {
        if !sandbox.state().outpost(map).built || *draining {
            continue;
        }
        let kind = sandbox.state().outpost_type(map);
        if kind == 1.0 {
            continue;
        }
        for (index, (slot, profession)) in sandbox.state().units(map).into_iter().enumerate() {
            let target = if index % 2 == 0 { 1 } else { 3 };
            if profession == target {
                continue;
            }
            let change = Move::Profession {
                map,
                slot,
                profession: target,
            };
            let Ok(next) = sandbox::apply(&sandbox, &change) else {
                continue;
            };
            sandbox = next;
            moves.push(change);
        }
    }
    (sandbox, moves)
}

/// What the layout step is for: a currency portfolio, or rank EXP when nothing
/// pays.
fn layout_mode(moves: &[Move]) -> &'static str {
    if moves.iter().any(|change| {
        matches!(
            change,
            Move::Connection {
                endpoint: Endpoint::Resource(_),
                ..
            }
        )
    }) {
        return "currency-portfolio";
    }
    "rank"
}

/// Spends one purchase on the sandbox: upstream `_applyArmoryShelfPurchase`.
fn buy(sandbox: &mut Sandbox, candidate: &Candidate) -> Result<(), Vec<String>> {
    let balance = sandbox
        .state()
        .g_cell(row::CURRENCIES, candidate.currency_slot.max(0) as usize);
    if candidate.cost > balance + f64::EPSILON {
        return Err(vec![format!(
            "{} costs {:.0} but the balance is {:.0}",
            candidate.name, candidate.cost, balance
        )]);
    }
    if sandbox.state().armory_level(candidate.index) != candidate.level {
        return Err(vec![format!(
            "{} is at level {}, not {}",
            candidate.name,
            sandbox.state().armory_level(candidate.index),
            candidate.level
        )]);
    }
    let level = candidate.level + 1.0;
    sandbox.set_currency(candidate.currency_slot, balance - candidate.cost);
    sandbox.set_armory(candidate.index, level);
    let counter = sandbox.state().daily_purchase_counter();
    sandbox.set_option(480, counter + 1.0);
    // Armory 30 is the shelf that banks time; buying it switches the flag on.
    if candidate.index == 30 && sandbox.state().g_cell(row::BANKED, 3) == 0.0 {
        sandbox.state_mut().write_g_cell(row::BANKED, 3, 1.0);
    }
    // A world shelf adds the Militia assignment it grants. Armory 80 also bumps
    // `OptLacc[383]`, a Spelunk shop counter the section does not read.
    if (60..=67).contains(&candidate.index) {
        let world = candidate.index - 60;
        sandbox.push_assignment(world, 4.0, (1 + 50 * world) as f64);
    }
    Ok(())
}

/// The hours, currency and reset events one advancement crosses.
struct Advance {
    hours: f64,
    currency: BTreeMap<i64, f64>,
    events: Vec<ResetEvent>,
}

/// A daily reset the plan crosses.
struct ResetEvent {
    hours: f64,
    refills: usize,
    grade_gains: usize,
}

/// Runs the clock forward `hours`, draining the linked nodes and processing the
/// resets it crosses: upstream `_advanceTime` and `_processReset`.
fn advance(sandbox: &mut Sandbox, hours: f64) -> Advance {
    let mut remaining = hours;
    let mut elapsed = 0.0;
    let mut events = Vec::new();
    let mut currency: BTreeMap<i64, f64> = BTreeMap::new();
    let reset_hours = sandbox
        .state()
        .reset_timing()
        .hours_remaining
        .filter(|value| *value > 0.0);
    // Without the countdown the plan still projects income, it just cannot
    // claim a refill; upstream's reset projection is unavailable in the same way.
    let reset_available = reset_hours.is_some();
    let mut reset_in = reset_hours.unwrap_or(f64::INFINITY);
    for _ in 0..MAX_STEPS {
        if remaining <= f64::EPSILON {
            break;
        }
        let (streams, _) = sandbox.state().resource_streams();
        let next_depletion = streams
            .iter()
            .filter(|stream| stream.drain_rate > 0.0)
            .map(|stream| sandbox.state().resource_remaining(stream.resource) / stream.drain_rate)
            .fold(f64::INFINITY, f64::min);
        let chunk = remaining
            .min(reset_in.max(f64::EPSILON))
            .min(next_depletion.max(f64::EPSILON));
        let mut gains: BTreeMap<i64, f64> = BTreeMap::new();
        for stream in &streams {
            if stream.drain_rate <= 0.0 {
                continue;
            }
            let collected = sandbox
                .state()
                .resource_remaining(stream.resource)
                .min(stream.drain_rate * chunk);
            if collected <= 0.0 {
                continue;
            }
            if stream.currency_slot >= 0 && !stream.savage {
                let share = collected * stream.currency_rate / stream.drain_rate;
                *gains.entry(stream.currency_slot).or_insert(0.0) += share;
            }
            let capacity = sandbox.state().resource_capacity(stream.resource);
            let raw = sandbox.state().resource_raw_progress(stream.resource);
            let next = if raw < 0.0 {
                raw
            } else {
                (raw + collected).min(capacity)
            };
            if raw >= 0.0 {
                sandbox.set_progress(stream.resource, next);
            }
        }
        for (slot, gain) in gains {
            let balance = sandbox
                .state()
                .g_cell(row::CURRENCIES, slot.max(0) as usize);
            sandbox.set_currency(slot, balance + gain);
            *currency.entry(slot).or_insert(0.0) += gain;
        }
        elapsed += chunk;
        remaining -= chunk;
        reset_in -= chunk;
        if reset_in <= f64::EPSILON && reset_available {
            let event = reset(sandbox, elapsed);
            events.push(event);
            reset_in = RESET_INTERVAL_HOURS;
        }
    }
    Advance {
        hours: elapsed,
        currency,
        events,
    }
}

/// Collects the full nodes and refills them: upstream `_processReset`, narrowed
/// to the nodes the plan drains.
fn reset(sandbox: &mut Sandbox, hours: f64) -> ResetEvent {
    let refill = sandbox.state().armory_bonus(70) >= 1.0;
    let grade = sandbox.state().armory_bonus(0) >= 1.0;
    let mut refills = 0;
    let mut grade_gains = 0;
    for resource in 0..data::resource_count() {
        if data::resource(resource)
            .map(|node| node.base_capacity <= 0.0 || node.currency_slot < 0)
            .unwrap_or(true)
        {
            continue;
        }
        let raw = sandbox.state().resource_raw_progress(resource);
        let capacity = sandbox.state().resource_capacity(resource);
        if raw < 0.0 {
            if !refill {
                continue;
            }
        } else if raw < capacity || !refill {
            continue;
        }
        refills += 1;
        sandbox.set_progress(resource, 0.0);
        if grade {
            let next = sandbox.state().resource_grade(resource) + 1.0;
            sandbox
                .state_mut()
                .write_g_cell(row::GRADES, resource, next);
            grade_gains += 1;
        }
    }
    ResetEvent {
        hours,
        refills,
        grade_gains,
    }
}
