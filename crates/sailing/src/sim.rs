//! The fleet pile simulation: fresh boat departures, the near-full pile scan
//! and the payouts of the chests left when the run ends.
//!
//! Ported from upstream `js/stats/systems/w5/sailing-sim.js`. Every trial starts
//! with an empty pile and all boats freshly departed, so the first chest from a
//! boat arrives after one full trip. A chest that finds the pile at its
//! near-full target scans the banked chests from the first one onward: the first
//! same-island, same-tier, sufficient-value match rolls its Unending Loot Search
//! promotion and stops the scan; otherwise the first same-island chest with both
//! a strictly higher tier and a strictly greater value replaces it. Every payout
//! of an opened chest then chooses gold, primary or rare independently.
//!
//! One difference from upstream is deliberate: the random source is a seeded
//! xorshift instead of `Math.random`, so the same inputs and seed reproduce the
//! same result. `script/sailing-simulation-oracle.mjs` drives the upstream
//! simulator with the same generator, which keeps the fixture comparison exact.

use super::{
    AssignmentMode, FleetPlan, Goal, RouteContext, TreasureShares, chest_artifact_multiplier,
    chest_loot_multiplier, chest_loot_rolls, overflow_upgrade_chance,
};
use idleon_common::Control;
use idleon_common::Progress;

/// How many trials a fresh run is configured for.
pub const DEFAULT_TRIALS: usize = 5_000;
/// The default interval in hours.
pub const DEFAULT_HOURS: f64 = 8.0;
/// The shortest interval the section offers.
pub const MIN_HOURS: f64 = 0.25;
/// The longest interval the section offers.
pub const MAX_HOURS: f64 = 168.0;
/// The fewest trials the section offers.
pub const MIN_TRIALS: usize = 100;
/// The most trials the section offers.
pub const MAX_TRIALS: usize = 100_000;
/// The xorshift seed the section uses when the caller does not pick one.
pub const DEFAULT_SEED: u64 = 0x5341_494c_494e_4701;

/// The interval and precision of one simulation run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimOptions {
    /// The interval each trial covers.
    pub hours: f64,
    /// How many independent trials to run.
    pub trials: usize,
    /// The xorshift seed; the same inputs and seed reproduce the result.
    pub seed: u64,
}

impl Default for SimOptions {
    fn default() -> Self {
        Self {
            hours: DEFAULT_HOURS,
            trials: DEFAULT_TRIALS,
            seed: DEFAULT_SEED,
        }
    }
}

/// One boat/captain pairing as the simulation reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct SimRoute {
    pub boat: usize,
    pub captain: Option<usize>,
    pub trips_per_hour: f64,
    pub loot_value: f64,
    pub artifact_multiplier: f64,
    pub island: usize,
    /// The six exact chest-tier chances, which sum to one.
    pub tier_probabilities: [f64; 6],
}

/// Everything one run reads, built from a fleet plan and its route context.
#[derive(Clone, Debug, PartialEq)]
pub struct SimConfig {
    pub routes: Vec<SimRoute>,
    /// The destination, mode and goal the plan was built for, recorded with the
    /// result so it stays self-describing after the controls change.
    pub island: usize,
    pub mode: AssignmentMode,
    pub goal: Goal,
    pub hours: f64,
    pub trials: usize,
    pub seed: u64,
    pub capacity: usize,
    pub minimum_tier: u32,
    pub unending_loot_search: f64,
    pub loot_multipliers: [f64; 6],
    pub artifact_multipliers: [f64; 6],
    pub upgrade_chances: [f64; 6],
    pub payouts_by_tier: [u32; 6],
    pub currency_shares: TreasureShares,
}

impl SimConfig {
    /// The upstream `buildSailingSimulationConfig`: the plan's routes, the
    /// account's capacity, payout rolls and promotion chances, and the first
    /// route's treasure-currency split.
    pub fn from_plan(plan: &FleetPlan, context: &RouteContext, options: SimOptions) -> Self {
        let minimum_tier = context.minimum_tier.min(5);
        let routes = plan
            .routes
            .iter()
            .map(|route| SimRoute {
                boat: route.boat,
                captain: route.captain,
                trips_per_hour: route.generated_chests_per_hour,
                loot_value: route.loot.value,
                artifact_multiplier: route.artifact_base.value,
                island: route.island,
                tier_probabilities: std::array::from_fn(|tier| {
                    route
                        .rarity
                        .tiers
                        .get(tier)
                        .map_or(0.0, |row| row.probability)
                }),
            })
            .collect();
        Self {
            routes,
            island: plan.island,
            mode: plan.mode,
            goal: plan.goal,
            hours: options.hours.max(0.0),
            trials: options.trials.max(1),
            seed: options.seed,
            capacity: context.chest_capacity,
            minimum_tier,
            unending_loot_search: context.unending_loot_search,
            loot_multipliers: std::array::from_fn(chest_loot_multiplier),
            artifact_multipliers: std::array::from_fn(chest_artifact_multiplier),
            upgrade_chances: std::array::from_fn(|tier| {
                overflow_upgrade_chance(tier as u32, context.unending_loot_search)
            }),
            payouts_by_tier: std::array::from_fn(|tier| chest_loot_rolls(tier, context)),
            currency_shares: plan
                .routes
                .first()
                .map_or_else(Default::default, |route| route.treasure_shares),
        }
    }

    /// The pile count the near-full scan starts at: one slot short when
    /// Unending Loot Search is active, otherwise the full capacity.
    pub fn target_count(&self) -> usize {
        if self.unending_loot_search > 0.0 {
            self.capacity.saturating_sub(1)
        } else {
            self.capacity
        }
    }
}

/// One distribution's mean and spread across trials.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimSummary {
    pub mean: f64,
    pub p10: f64,
    pub median: f64,
    pub p90: f64,
}

impl SimSummary {
    fn from(values: &mut [f64]) -> Self {
        values.sort_by(|left, right| left.total_cmp(right));
        let total: f64 = values.iter().sum();
        Self {
            mean: if values.is_empty() {
                0.0
            } else {
                total / values.len() as f64
            },
            p10: quantile(values, 0.1),
            median: quantile(values, 0.5),
            p90: quantile(values, 0.9),
        }
    }
}

/// The upstream `_quantile`: linear interpolation between the two nearest
/// sorted samples.
fn quantile(values: &[f64], probability: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let position = ((values.len() - 1) as f64 * probability).clamp(0.0, (values.len() - 1) as f64);
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        return values[lower];
    }
    values[lower] + (values[upper] - values[lower]) * (position - lower as f64)
}

/// Per-trial event averages: what the fleet generated and how the pile took it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimEvents {
    pub generated: f64,
    pub accepted: f64,
    pub upgrades: f64,
    pub replacements: f64,
    pub filter_discards: f64,
    pub overflow_discards: f64,
    pub blocked: f64,
}

/// The final pile's count and opened value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimPile {
    pub count: SimSummary,
    pub value: SimSummary,
    /// Total opened value divided by the average chest count.
    pub average_value_per_chest: f64,
}

/// One treasure currency's opened value and payout count across trials.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimCurrency {
    pub value: SimSummary,
    pub payouts: SimSummary,
}

/// The three treasure currencies.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimCurrencies {
    pub gold: SimCurrency,
    pub primary: SimCurrency,
    pub rare: SimCurrency,
}

/// One pile position across trials.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SimSlot {
    pub index: usize,
    /// How often a chest occupies this position.
    pub occupancy: f64,
    /// The average tier, conditional on occupancy.
    pub average_tier: f64,
    /// The most common tier; `None` when the slot is never occupied.
    pub most_likely_tier: Option<usize>,
    pub tier_distribution: [f64; 6],
    /// The chest's stored value per payout, before opening.
    pub average_value_per_payout: f64,
    pub average_artifact_multiplier: f64,
    pub average_payouts: f64,
    pub average_opened_value: f64,
    /// The expected currency contribution per trial.
    pub average_currency_value: [f64; 3],
}

/// One chest tier's average contribution to the final pile.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimTier {
    pub tier: usize,
    pub average_count: f64,
    pub average_value_per_payout: f64,
    pub average_opened_value: f64,
    pub opened_value_contribution: f64,
}

/// The run's inputs as the result records them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimSetup {
    pub island: usize,
    pub mode: AssignmentMode,
    pub goal: Goal,
    pub hours: f64,
    pub trials: usize,
    pub capacity: usize,
    pub target_count: usize,
    pub minimum_tier: u32,
    pub seed: u64,
}

/// One finished simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct SimResult {
    pub setup: SimSetup,
    pub events: SimEvents,
    pub pile: SimPile,
    pub active_collection_value: SimSummary,
    /// Final pile value over the value of opening every chest immediately.
    pub retained_fraction: f64,
    pub slots: Vec<SimSlot>,
    pub tiers: Vec<SimTier>,
    pub currencies: SimCurrencies,
}

/// One scheduled chest arrival.
#[derive(Clone, Copy, Debug)]
struct Event {
    at: f64,
    route: usize,
    boat: usize,
}

/// One banked chest.
#[derive(Clone, Copy, Debug)]
struct Chest {
    value: f64,
    island: usize,
    artifact_multiplier: f64,
    tier: usize,
}

/// The upstream near-full scan's outcome for one incoming chest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NearFull {
    Upgrade,
    Replacement,
    OverflowDiscard,
}

/// A seeded xorshift64* generator, so a run is reproducible.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A uniform number in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Run the simulation, reporting progress about every one percent of the
/// trials. A report that returns [`Control::Stop`] ends the run with `None` and
/// leaves the caller's previous result untouched.
pub fn simulate(
    config: &SimConfig,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<SimResult> {
    let trials = config.trials.max(1);
    let hours = config.hours.max(0.0);
    let capacity = config.capacity;
    let minimum_tier = config.minimum_tier.min(5);
    let target_count = config.target_count();
    let events = events_for_trial(config, hours);
    let report_every = (trials / 100).max(1);

    let mut slot_sums = vec![SlotAccum::default(); capacity];
    let mut tier_sums = [TierAccum::default(); 6];
    let mut totals = EventTotals::default();
    let mut pile_values = Vec::with_capacity(trials);
    let mut active_values = Vec::with_capacity(trials);
    let mut counts = Vec::with_capacity(trials);
    let mut currency_values = [
        Vec::with_capacity(trials),
        Vec::with_capacity(trials),
        Vec::with_capacity(trials),
    ];
    let mut currency_payouts = [
        Vec::with_capacity(trials),
        Vec::with_capacity(trials),
        Vec::with_capacity(trials),
    ];
    let mut rng = Rng::new(config.seed);

    for trial in 0..trials {
        let mut pile: Vec<Chest> = Vec::with_capacity(capacity);
        let mut active_value = 0.0;
        let mut trial_events = TrialEvents::default();

        for event in &events {
            let route = &config.routes[event.route];
            let mut incoming = incoming_chest(route, config, &mut rng);
            trial_events.generated += 1.0;
            active_value += opened_value(&incoming, &config.payouts_by_tier);
            if pile.len() >= capacity {
                trial_events.blocked += 1.0;
            } else if config.unending_loot_search > 0.0 && pile.len() == target_count {
                match process_near_full(&mut pile, &mut incoming, config, &mut rng) {
                    NearFull::Upgrade => trial_events.upgrades += 1.0,
                    NearFull::Replacement => trial_events.replacements += 1.0,
                    NearFull::OverflowDiscard => trial_events.overflow_discards += 1.0,
                }
            } else if incoming.tier as u32 >= minimum_tier {
                pile.push(incoming);
                trial_events.accepted += 1.0;
            } else {
                trial_events.filter_discards += 1.0;
            }
        }

        let mut pile_value = 0.0;
        let mut trial_currency_values = [0.0_f64; 3];
        let mut trial_currency_payouts = [0.0_f64; 3];
        for (position, chest) in pile.iter().enumerate() {
            let payouts = config.payouts_by_tier[chest.tier].max(1);
            let opened = chest.value * f64::from(payouts);
            let slot = &mut slot_sums[position];
            let tier_summary = &mut tier_sums[chest.tier];
            slot.occupied += 1;
            slot.value += chest.value;
            slot.artifact += chest.artifact_multiplier;
            slot.payouts += f64::from(payouts);
            slot.opened += opened;
            slot.tier_counts[chest.tier] += 1;
            tier_summary.count += 1;
            tier_summary.value += chest.value;
            tier_summary.opened += opened;
            pile_value += opened;
            for _ in 0..payouts {
                let currency = currency_pick(config.currency_shares, &mut rng);
                trial_currency_payouts[currency] += 1.0;
                trial_currency_values[currency] += chest.value;
                slot.currencies[currency] += chest.value;
            }
        }

        pile_values.push(pile_value);
        active_values.push(active_value);
        counts.push(pile.len() as f64);
        for currency in 0..3 {
            currency_values[currency].push(trial_currency_values[currency]);
            currency_payouts[currency].push(trial_currency_payouts[currency]);
        }
        totals.add(&trial_events);

        let done = trial + 1;
        if done % report_every == 0 || done == trials {
            let control = report(Progress {
                fraction: done as f32 / trials as f32,
                message: format!("{done} / {trials} trials"),
            });
            if control == Control::Stop {
                return None;
            }
        }
    }

    let trial_count = trials as f64;
    let pile_count = SimSummary::from(&mut counts);
    let pile_value = SimSummary::from(&mut pile_values);
    let active_collection_value = SimSummary::from(&mut active_values);
    let slots = slot_sums
        .into_iter()
        .enumerate()
        .map(|(index, slot)| slot.finish(index, trials))
        .collect();
    let tiers = tier_sums
        .into_iter()
        .enumerate()
        .map(|(tier, summary)| SimTier {
            tier,
            average_count: summary.count as f64 / trial_count,
            average_value_per_payout: if summary.count > 0 {
                summary.value / summary.count as f64
            } else {
                0.0
            },
            average_opened_value: if summary.count > 0 {
                summary.opened / summary.count as f64
            } else {
                0.0
            },
            opened_value_contribution: summary.opened / trial_count,
        })
        .collect();
    let mut currencies = [SimCurrency::default(); 3];
    for (currency, summary) in currencies.iter_mut().enumerate() {
        *summary = SimCurrency {
            value: SimSummary::from(&mut currency_values[currency]),
            payouts: SimSummary::from(&mut currency_payouts[currency]),
        };
    }

    Some(SimResult {
        setup: SimSetup {
            island: config.island,
            mode: config.mode,
            goal: config.goal,
            hours,
            trials,
            capacity,
            target_count,
            minimum_tier,
            seed: config.seed,
        },
        events: totals.finish(trial_count),
        pile: SimPile {
            count: pile_count,
            value: pile_value,
            average_value_per_chest: pile_value.mean / pile_count.mean.max(1.0),
        },
        active_collection_value,
        retained_fraction: if active_collection_value.mean > 0.0 {
            pile_value.mean / active_collection_value.mean
        } else {
            0.0
        },
        slots,
        tiers,
        currencies: SimCurrencies {
            gold: currencies[0],
            primary: currencies[1],
            rare: currencies[2],
        },
    })
}

/// Every chest arrival one trial sees, sorted by time and then boat as the
/// upstream `_eventsForTrial` sorts it.
fn events_for_trial(config: &SimConfig, hours: f64) -> Vec<Event> {
    let mut events = Vec::new();
    for (route_index, route) in config.routes.iter().enumerate() {
        let rate = route.trips_per_hour.max(0.0);
        if rate <= 0.0 || !rate.is_finite() {
            continue;
        }
        let period = 1.0 / rate;
        let mut at = period;
        while at <= hours + 1e-12 {
            events.push(Event {
                at,
                route: route_index,
                boat: route.boat,
            });
            at += period;
        }
    }
    events.sort_by(|left, right| {
        left.at
            .total_cmp(&right.at)
            .then(left.boat.cmp(&right.boat))
    });
    events
}

fn incoming_chest(route: &SimRoute, config: &SimConfig, rng: &mut Rng) -> Chest {
    let tier = sample_tier(&route.tier_probabilities, rng);
    Chest {
        value: route.loot_value * config.loot_multipliers[tier],
        island: route.island,
        artifact_multiplier: route.artifact_multiplier * config.artifact_multipliers[tier],
        tier,
    }
}

fn sample_tier(probabilities: &[f64; 6], rng: &mut Rng) -> usize {
    let roll = rng.unit();
    let mut cumulative = 0.0;
    for (tier, probability) in probabilities.iter().enumerate() {
        cumulative += probability.max(0.0);
        if roll < cumulative {
            return tier;
        }
    }
    5
}

fn opened_value(chest: &Chest, payouts_by_tier: &[u32; 6]) -> f64 {
    chest.value * f64::from(payouts_by_tier[chest.tier])
}

/// The upstream `_processNearFull`: scan from the first banked chest and stop at
/// the first eligible same-tier match or the first strictly better replacement.
fn process_near_full(
    pile: &mut [Chest],
    incoming: &mut Chest,
    config: &SimConfig,
    rng: &mut Rng,
) -> NearFull {
    for existing in pile.iter_mut() {
        if incoming.island == existing.island
            && incoming.tier == existing.tier
            && incoming.value >= existing.value
        {
            let chance = config.upgrade_chances[incoming.tier];
            if chance > 0.0 && rng.unit() < chance {
                incoming.value *= 1.85;
                incoming.artifact_multiplier *= 1.4;
                incoming.tier += 1;
                *existing = *incoming;
                return NearFull::Upgrade;
            }
            return NearFull::OverflowDiscard;
        }
        if incoming.island == existing.island
            && incoming.tier > existing.tier
            && incoming.value > existing.value
        {
            *existing = *incoming;
            return NearFull::Replacement;
        }
    }
    NearFull::OverflowDiscard
}

fn currency_pick(shares: TreasureShares, rng: &mut Rng) -> usize {
    let gold = shares.gold.max(0.0);
    let primary = shares.primary.max(0.0);
    let roll = rng.unit();
    if roll < gold {
        0
    } else if roll < gold + primary {
        1
    } else {
        2
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct TrialEvents {
    generated: f64,
    accepted: f64,
    upgrades: f64,
    replacements: f64,
    filter_discards: f64,
    overflow_discards: f64,
    blocked: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct EventTotals {
    generated: f64,
    accepted: f64,
    upgrades: f64,
    replacements: f64,
    filter_discards: f64,
    overflow_discards: f64,
    blocked: f64,
}

impl EventTotals {
    fn add(&mut self, trial: &TrialEvents) {
        self.generated += trial.generated;
        self.accepted += trial.accepted;
        self.upgrades += trial.upgrades;
        self.replacements += trial.replacements;
        self.filter_discards += trial.filter_discards;
        self.overflow_discards += trial.overflow_discards;
        self.blocked += trial.blocked;
    }

    fn finish(self, trials: f64) -> SimEvents {
        SimEvents {
            generated: self.generated / trials,
            accepted: self.accepted / trials,
            upgrades: self.upgrades / trials,
            replacements: self.replacements / trials,
            filter_discards: self.filter_discards / trials,
            overflow_discards: self.overflow_discards / trials,
            blocked: self.blocked / trials,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SlotAccum {
    occupied: usize,
    value: f64,
    artifact: f64,
    payouts: f64,
    opened: f64,
    currencies: [f64; 3],
    tier_counts: [usize; 6],
}

impl SlotAccum {
    fn finish(self, index: usize, trials: usize) -> SimSlot {
        let occupied = self.occupied as f64;
        let most_likely = if self.occupied == 0 {
            None
        } else {
            let mut best = 0;
            for (tier, count) in self.tier_counts.iter().enumerate() {
                if *count > self.tier_counts[best] {
                    best = tier;
                }
            }
            Some(best)
        };
        SimSlot {
            index,
            occupancy: occupied / trials as f64,
            average_tier: if self.occupied > 0 {
                self.tier_counts
                    .iter()
                    .enumerate()
                    .map(|(tier, count)| tier as f64 * *count as f64)
                    .sum::<f64>()
                    / occupied
            } else {
                0.0
            },
            most_likely_tier: most_likely,
            tier_distribution: std::array::from_fn(|tier| {
                if self.occupied > 0 {
                    self.tier_counts[tier] as f64 / occupied
                } else {
                    0.0
                }
            }),
            average_value_per_payout: if self.occupied > 0 {
                self.value / occupied
            } else {
                0.0
            },
            average_artifact_multiplier: if self.occupied > 0 {
                self.artifact / occupied
            } else {
                0.0
            },
            average_payouts: if self.occupied > 0 {
                self.payouts / occupied
            } else {
                0.0
            },
            average_opened_value: if self.occupied > 0 {
                self.opened / occupied
            } else {
                0.0
            },
            average_currency_value: std::array::from_fn(|currency| {
                self.currencies[currency] / trials as f64
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct TierAccum {
    count: usize,
    value: f64,
    opened: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Boat, Captain, Fleet, Goal, RouteContext, artifact_count, optimize};

    fn fleet() -> Fleet {
        Fleet {
            boats: vec![
                Boat {
                    captain: Some(1),
                    loot_level: 100,
                    speed_level: 100,
                    ..Boat::default()
                },
                Boat {
                    captain: Some(0),
                    loot_level: 80,
                    speed_level: 80,
                    ..Boat::default()
                },
            ],
            captains: vec![
                Captain {
                    rarity: 3,
                    bonus_types: [0, 1],
                    level: 10,
                    exp: 0.0,
                    rolls: [20.0, 10.0],
                },
                Captain {
                    rarity: 6,
                    bonus_types: [4, 3],
                    level: 20,
                    exp: 0.0,
                    rolls: [30.0, 15.0],
                },
            ],
            sailing_level: 10,
            artifact_tiers: vec![0; artifact_count()],
            island_progress: vec![-1.0; 20],
            currency_balances: Vec::new(),
        }
    }

    fn config(trials: usize, seed: u64) -> SimConfig {
        let fleet = fleet();
        let context = RouteContext {
            unending_loot_search: 120.0,
            chest_capacity: 12,
            ..RouteContext::default()
        };
        let plan = optimize(&fleet, 0, Goal::Treasure, &context);
        SimConfig::from_plan(
            &plan,
            &context,
            SimOptions {
                hours: 8.0,
                trials,
                seed,
            },
        )
    }

    fn report_continue(_: Progress) -> Control {
        Control::Continue
    }

    #[test]
    fn the_same_seed_and_inputs_reproduce_the_run() {
        let config = config(200, 42);
        let first = simulate(&config, &mut report_continue).expect("the run completes");
        let second = simulate(&config, &mut report_continue).expect("the run completes");
        assert_eq!(first, second);
        let other = simulate(
            &SimConfig {
                seed: 43,
                ..config.clone()
            },
            &mut report_continue,
        )
        .expect("the run completes");
        assert_ne!(first, other, "a new seed moves the samples");
    }

    #[test]
    fn a_cancelled_run_stops_at_its_next_report() {
        let mut reports = 0;
        let outcome = simulate(&config(1_000, 7), &mut |progress| {
            reports += 1;
            assert!(progress.fraction <= 1.0);
            Control::Stop
        });
        assert!(outcome.is_none());
        assert_eq!(reports, 1);
    }

    #[test]
    fn the_trial_tier_chances_sum_to_one() {
        for route in config(1, 1).routes {
            let sum: f64 = route.tier_probabilities.iter().sum();
            assert!((sum - 1.0).abs() < 1e-12, "chances sum to {sum}");
        }
    }

    #[test]
    fn unending_loot_search_targets_one_slot_short() {
        let mut config = config(1, 1);
        assert_eq!(config.target_count(), 11);
        config.unending_loot_search = 0.0;
        assert_eq!(config.target_count(), 12);
        config.capacity = 0;
        config.unending_loot_search = 100.0;
        assert_eq!(config.target_count(), 0);
    }
}
