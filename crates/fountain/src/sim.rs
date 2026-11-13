//! FN-04: the fountain simulator.
//!
//! Ported from `runSim` in `js/fountain-sim-worker.js`. The simulation advances
//! an active cavern forward in time: it buys the upgrade that best grows the
//! marble rate (weighted three times) and the optimal desired-currency rate,
//! waits for the currency the next purchase costs, drains the marble bar as it
//! fills, and spends the marble on the best marble level whenever one becomes
//! affordable. Currency income is a steady-state estimate at the current rates,
//! so the saved ground stacks do not seed the run.
//!
//! There is no randomness and no wall clock in the loop: the same cavern and
//! [`SimRequest`] always produce the same events, balances and levels.

use super::data::UPGRADES_PER_WATER;
use super::{CURRENCIES, Fountain, Goal, WATERS};
use idleon_common::{Control, Progress};

/// How long the run simulates, and the desired currency it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimRequest {
    /// The time limit in hours, as the page's input reads it.
    pub hours: f64,
    /// The simulated desired currency; `None` reads the save's choice.
    pub desired: Option<usize>,
}

impl Default for SimRequest {
    fn default() -> Self {
        Self {
            hours: 24.0,
            desired: None,
        }
    }
}

/// One line on the simulation's timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct SimEvent {
    /// Seconds since the run started.
    pub time: f64,
    pub kind: SimEventKind,
    pub marble_rate: f64,
    pub base_currency_rate: f64,
    /// Every currency's active rate at that moment.
    pub currency_rates: [f64; CURRENCIES],
}

/// What happened on one timeline line.
#[derive(Clone, Debug, PartialEq)]
pub enum SimEventKind {
    BuyUpgrade {
        water: usize,
        upgrade: usize,
        name: &'static str,
        cost: f64,
        currency: usize,
        /// The level after the purchase.
        level: f64,
    },
    BuyMarble {
        water: usize,
        upgrade: usize,
        name: &'static str,
        cost: f64,
        /// The marble level after the purchase.
        marble_level: f64,
    },
    /// The run changed the desired currency, the Turn and Push focus, or both.
    Switch {
        desired: usize,
        focus: Option<usize>,
    },
    /// The currency the run earned while it waited.
    Claim {
        currency: usize,
        amount: f64,
        seconds: f64,
    },
    /// The marble bar filled.
    MarbleFill { marbles: f64, total: f64 },
}

/// The simulation's result: the timeline and the cavern it ended on.
#[derive(Clone, Debug, PartialEq)]
pub struct SimResult {
    pub events: Vec<SimEvent>,
    pub start_marble_rate: f64,
    pub end_marble_rate: f64,
    pub start_base_currency: f64,
    pub end_base_currency: f64,
    pub start_currency_rates: [f64; CURRENCIES],
    pub end_currency_rates: [f64; CURRENCIES],
    /// The seconds the run covered; the limit when time ran out.
    pub clock_seconds: f64,
    /// How many currency upgrades the run bought.
    pub steps: usize,
    pub currencies: [f64; CURRENCIES],
    pub marbles: f64,
    pub marble_bar_progress: f64,
    /// The upgrade levels the run ended on, `[water][upgrade]`.
    pub upgrade_levels: Vec<Vec<f64>>,
    /// The marble levels the run ended on, `[water][upgrade]`.
    pub marble_levels: Vec<Vec<f64>>,
}

/// The rates the run reads at one moment.
#[derive(Clone, Copy)]
struct Rates {
    currency: [f64; CURRENCIES],
    marble: f64,
    /// The marble bar seconds one active-play second covers; zero while locked.
    marble_progress_per_sec: f64,
}

/// The desired currency and focus one purchase target asks for.
struct Target {
    desired: usize,
    focus: Option<usize>,
    changes_desired: bool,
    changes_focus: bool,
}

/// One scored purchase, before it is executed.
struct Candidate {
    score: f64,
    tta: f64,
    cost: f64,
    /// `None` for a marble level, which is paid from the marble balance.
    currency: Option<usize>,
    remaining: f64,
    water: usize,
    upgrade: usize,
    name: &'static str,
    /// The upgrade or marble level after the purchase.
    level: f64,
}

/// The best currency-draining upgrade, used to value currency-only purchases.
struct MarbleTarget {
    /// The relative marble-rate gain.
    m_pct: f64,
    tta_sec: f64,
    cost_type: usize,
    cost: f64,
}

impl Fountain {
    /// `runSim` without a progress report.
    pub fn simulate(&self, request: SimRequest) -> SimResult {
        self.simulate_with(request, |_| Control::Continue)
            .expect("a run without reports is never asked to stop")
    }

    /// `runSim`: simulate active fountain play for the request's time limit.
    ///
    /// Returns `None` when the report asked the run to stop; the original
    /// cavern is untouched either way, because the run works on a copy.
    pub fn simulate_with(
        &self,
        request: SimRequest,
        report: impl FnMut(Progress) -> Control,
    ) -> Option<SimResult> {
        let desired = request.desired.unwrap_or_else(|| self.desired_currency());
        let active = self.active_currencies();
        let focus = (self.ignore_unlocked() && active.len() == 1).then(|| active[0]);
        let mut run = Run {
            fountain: self.clone(),
            desired,
            fixed_desired: desired,
            focus,
            time_limit: (request.hours * 3600.0).max(0.0),
            clock: 0.0,
            currencies: [0.0; CURRENCIES],
            marbles: self.marble(),
            marble_bar_progress: self.marble_bar_progress(),
            events: Vec::new(),
            steps: 0,
            start_marble_rate: self.marble_per_hr(),
            start_base_currency: self.base_currency_per_hr(),
            start_currency_rates: self.active_earn_rates_with(desired, focus),
            report: Box::new(report),
        };
        for (currency, balance) in run.currencies.iter_mut().enumerate() {
            *balance = self.level_currency_balance(currency);
        }
        if run.report_line() == Control::Stop {
            return None;
        }
        run.buy_affordable();
        while run.clock < run.time_limit {
            let rates = run.rates();
            let marble_target = run.find_best_marble_target(&rates);
            let best = run.best_upgrade(&rates, marble_target.as_ref());
            let Some(best) = best else {
                let remaining_time = run.time_limit - run.clock;
                if remaining_time > 0.0 {
                    let rates = run.rates();
                    let earned = run.advance_time(remaining_time, &rates);
                    run.push_claims(&earned, remaining_time);
                }
                break;
            };
            if best.tta <= 0.0 {
                run.execute(&best);
                run.steps += 1;
                run.buy_affordable();
                if run.report_line() == Control::Stop {
                    return None;
                }
                continue;
            }
            if !best.tta.is_finite() {
                break;
            }
            let target = run.target_state(
                best.currency
                    .expect("a planned target is a currency upgrade"),
            );
            if target.changes_focus && best.remaining > 0.0 {
                let flush = run.fountain.flush_time();
                if run.clock + flush > run.time_limit {
                    run.clock = run.time_limit;
                    break;
                }
                let rates = run.rates();
                let earned = run.advance_time(flush, &rates);
                run.push_claims(&earned, flush);
            }
            if (target.changes_desired || target.changes_focus) && best.remaining > 0.0 {
                run.desired = target.desired;
                run.focus = target.focus;
                run.push(SimEventKind::Switch {
                    desired: run.desired,
                    focus: run.focus,
                });
            }
            let currency = best
                .currency
                .expect("a planned target is a currency upgrade");
            let remaining = (best.cost - run.currencies[currency]).max(0.0);
            let earn_rate = run.rates().currency[currency];
            let earn_time = if earn_rate > 0.0 {
                remaining / earn_rate * 3600.0
            } else {
                f64::INFINITY
            };
            if !earn_time.is_finite() {
                break;
            }
            let advance = earn_time.min(run.time_limit - run.clock);
            if advance > 0.0 {
                let rates = run.rates();
                let earned = run.advance_time(advance, &rates);
                run.push_claims(&earned, advance);
            }
            if run.clock >= run.time_limit {
                break;
            }
            run.execute(&best);
            run.steps += 1;
            run.buy_affordable();
            if run.report_line() == Control::Stop {
                return None;
            }
        }
        let end_currency_rates = run.fountain.active_earn_rates_with(run.desired, run.focus);
        Some(SimResult {
            events: run.events,
            start_marble_rate: run.start_marble_rate,
            end_marble_rate: run.fountain.marble_per_hr(),
            start_base_currency: run.start_base_currency,
            end_base_currency: run.fountain.base_currency_per_hr(),
            start_currency_rates: run.start_currency_rates,
            end_currency_rates,
            clock_seconds: run.clock,
            steps: run.steps,
            currencies: run.currencies,
            marbles: run.marbles,
            marble_bar_progress: run.marble_bar_progress,
            upgrade_levels: run.fountain.upg_lvs.clone(),
            marble_levels: run.fountain.marble_lvs.clone(),
        })
    }
}

/// One simulation in progress.
struct Run<'a> {
    fountain: Fountain,
    desired: usize,
    /// The desired currency the page's goal measurements hold fixed.
    fixed_desired: usize,
    focus: Option<usize>,
    time_limit: f64,
    clock: f64,
    currencies: [f64; CURRENCIES],
    marbles: f64,
    marble_bar_progress: f64,
    events: Vec<SimEvent>,
    steps: usize,
    start_marble_rate: f64,
    start_base_currency: f64,
    start_currency_rates: [f64; CURRENCIES],
    report: Box<dyn FnMut(Progress) -> Control + 'a>,
}

impl Run<'_> {
    /// `_rates`: every currency's active rate and the marble bar speed.
    fn rates(&self) -> Rates {
        Rates {
            currency: self
                .fountain
                .active_earn_rates_with(self.desired, self.focus),
            marble: self.fountain.marble_per_hr(),
            marble_progress_per_sec: if self.fountain.marble_bar_unlocked() {
                self.fountain.active_speed()
            } else {
                0.0
            },
        }
    }

    /// One timeline line at the current clock.
    fn push(&mut self, kind: SimEventKind) {
        self.events.push(SimEvent {
            time: self.clock,
            kind,
            marble_rate: self.fountain.marble_per_hr(),
            base_currency_rate: self.fountain.base_currency_per_hr(),
            currency_rates: self
                .fountain
                .active_earn_rates_with(self.desired, self.focus),
        });
    }

    /// Report the simulated fraction of the time limit.
    fn report_line(&mut self) -> Control {
        let fraction = if self.time_limit > 0.0 {
            (self.clock / self.time_limit).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };
        (self.report)(Progress {
            fraction,
            message: format!(
                "Simulated {}h of {}h.",
                super::short_number(self.clock / 3600.0),
                super::short_number(self.time_limit / 3600.0)
            ),
        })
    }

    /// `_targetState`: what a purchase on `kind` would switch to.
    fn target_state(&self, kind: usize) -> Target {
        let can_desire = self.fountain.bon_tot(1, 11) > 0.0;
        let can_focus = self.fountain.ignore_unlocked();
        Target {
            desired: if can_desire { kind } else { self.desired },
            focus: if can_focus { Some(kind) } else { self.focus },
            changes_desired: can_desire && self.desired != kind,
            changes_focus: can_focus && self.focus != Some(kind),
        }
    }

    /// `_targetRateFor`: the rate of `kind` once the run switches to it.
    fn target_rate(&self, kind: usize) -> f64 {
        let target = self.target_state(kind);
        self.fountain
            .active_earn_rates_with(target.desired, target.focus)[kind]
    }

    /// `_targetRateFor` at the levels a candidate purchase would reach.
    fn target_rate_at(&self, kind: usize, fountain: &Fountain) -> f64 {
        let can_desire = fountain.bon_tot(1, 11) > 0.0;
        let can_focus = fountain.ignore_unlocked();
        let desired = if can_desire { kind } else { self.desired };
        let focus = if can_focus { Some(kind) } else { self.focus };
        fountain.active_earn_rates_with(desired, focus)[kind]
    }

    /// `_relativeGain`: the relative gain from `current` to `next`.
    fn relative_gain(&self, current: f64, next: f64) -> f64 {
        if next <= current {
            return 0.0;
        }
        if current > 0.0 {
            (next - current) / current
        } else {
            1.0
        }
    }

    /// `_marbleTargetTimeSaved`: how much of the target's wait a purchase saves.
    #[allow(clippy::too_many_arguments)]
    fn marble_target_time_saved(
        &self,
        marble_target: Option<&MarbleTarget>,
        candidate_tta: f64,
        candidate_currency: Option<usize>,
        candidate_cost: f64,
        next: &Fountain,
        candidate_changes_focus: bool,
    ) -> f64 {
        let Some(target) = marble_target else {
            return 0.0;
        };
        if target.tta_sec <= 0.0 || !candidate_tta.is_finite() {
            return 0.0;
        }
        let mut balance = self.currencies[target.cost_type];
        if candidate_currency == Some(target.cost_type) {
            balance = (balance - candidate_cost).max(0.0);
        }
        let remaining = (target.cost - balance).max(0.0);
        let target_rate = self.target_rate_at(target.cost_type, next);
        if target_rate <= 0.0 {
            return 0.0;
        }
        let mut target_tta = remaining / target_rate * 3600.0;
        let focus_after = if candidate_changes_focus {
            candidate_currency
        } else {
            self.focus
        };
        if remaining > 0.0 && next.ignore_unlocked() && focus_after != Some(target.cost_type) {
            target_tta += next.flush_time();
        }
        let route_tta = candidate_tta + target_tta;
        if route_tta < target.tta_sec {
            1.0 - route_tta / target.tta_sec
        } else {
            0.0
        }
    }

    /// `_findBestMarbleTarget`: the currency upgrade that grows marble most.
    fn find_best_marble_target(&self, rates: &Rates) -> Option<MarbleTarget> {
        let mut best: Option<MarbleTarget> = None;
        let mut best_score = 0.0;
        for water in 0..WATERS {
            for upgrade in 0..UPGRADES_PER_WATER {
                if !self.fountain.upgrade_accessible(water, upgrade) {
                    continue;
                }
                let level = self.fountain.upgrade_level(water, upgrade);
                let candidate = self
                    .fountain
                    .with_upgrade_level(water, upgrade, level + 1.0);
                let new_marble = candidate.marble_per_hr();
                let gain = new_marble - rates.marble;
                if gain <= 0.0 {
                    continue;
                }
                let pct = self.relative_gain(rates.marble, new_marble);
                let Some(entry) = super::upgrade(water, upgrade) else {
                    continue;
                };
                let cost_type = entry.currency;
                let cost = Fountain::upgrade_cost(water, upgrade, level);
                let remaining = (cost - self.currencies[cost_type]).max(0.0);
                let earn_rate = self.target_rate(cost_type);
                let earn_sec = if earn_rate > 0.0 {
                    remaining / earn_rate * 3600.0
                } else {
                    f64::INFINITY
                };
                let target = self.target_state(cost_type);
                let switch = if target.changes_focus && remaining > 0.0 {
                    self.fountain.flush_time()
                } else {
                    0.0
                };
                let tta = earn_sec + switch;
                if !tta.is_finite() {
                    continue;
                }
                let hours = (tta / 3600.0).max(0.001);
                let score = pct / hours;
                if score > best_score {
                    best_score = score;
                    best = Some(MarbleTarget {
                        m_pct: pct,
                        tta_sec: tta,
                        cost_type,
                        cost,
                    });
                }
            }
        }
        best
    }

    /// The best currency purchase, as the page's main loop scores them.
    fn best_upgrade(&self, rates: &Rates, target: Option<&MarbleTarget>) -> Option<Candidate> {
        let mut best: Option<Candidate> = None;
        for water in 0..WATERS {
            for upgrade in 0..UPGRADES_PER_WATER {
                if !self.fountain.upgrade_accessible(water, upgrade) {
                    continue;
                }
                let candidate = self.score_upgrade(water, upgrade, rates, target);
                if candidate.score > 0.0
                    && best
                        .as_ref()
                        .is_none_or(|best| candidate.score > best.score)
                {
                    best = Some(candidate);
                }
            }
        }
        best
    }

    /// `_scoreUpgrade`: value a currency upgrade by the marble rate it grows
    /// (weighted three times) plus the optimal rate it adds.
    fn score_upgrade(
        &self,
        water: usize,
        upgrade: usize,
        rates: &Rates,
        marble_target: Option<&MarbleTarget>,
    ) -> Candidate {
        let Some(entry) = super::upgrade(water, upgrade) else {
            return self.empty_candidate(water, upgrade);
        };
        let level = self.fountain.upgrade_level(water, upgrade);
        let cost = Fountain::upgrade_cost(water, upgrade, level);
        let currency = entry.currency;
        let have = self.currencies[currency];
        let remaining = (cost - have).max(0.0);
        let earn_rate = self.target_rate(currency);
        let earn_time = if earn_rate > 0.0 {
            remaining / earn_rate * 3600.0
        } else {
            f64::INFINITY
        };
        let target = self.target_state(currency);
        let switch_cost = if target.changes_focus && remaining > 0.0 {
            self.fountain.flush_time()
        } else {
            0.0
        };
        let mut tta = earn_time + switch_cost;
        if !tta.is_finite() || tta < 0.0 {
            tta = f64::INFINITY;
        }
        let candidate = self
            .fountain
            .with_upgrade_level(water, upgrade, level + 1.0);
        let new_marble = candidate.marble_per_hr();
        let marble_pct = self.relative_gain(rates.marble, new_marble);
        let desired = self.fixed_desired;
        let current_optimal = self.fountain.measure_goal(Goal::Optimal, desired);
        let new_optimal = candidate.measure_goal(Goal::Optimal, desired);
        let optimal_gain = self.relative_gain(current_optimal, new_optimal);
        let time_saved = self.marble_target_time_saved(
            marble_target,
            tta,
            Some(currency),
            cost,
            &candidate,
            target.changes_focus && remaining > 0.0,
        );
        let effective_gain = if marble_pct > 0.0 {
            marble_pct * 3.0 + optimal_gain.max(0.0)
        } else if time_saved > 0.0 && marble_target.is_some() {
            marble_target.map_or(0.0, |target| target.m_pct * 3.0 * time_saved)
        } else {
            optimal_gain.max(0.0)
        };
        let invest_hours = if remaining > 0.0 {
            tta / 3600.0
        } else {
            let recoup = if earn_rate > 0.0 {
                cost / earn_rate * 3600.0
            } else {
                f64::INFINITY
            };
            recoup / 3600.0
        };
        let mut score = if effective_gain > 0.0 && invest_hours > 0.0 && invest_hours.is_finite() {
            effective_gain / invest_hours
        } else {
            0.0
        };
        if tta > self.time_limit - self.clock {
            score = 0.0;
        }
        Candidate {
            score,
            tta,
            cost,
            currency: Some(currency),
            remaining,
            water,
            upgrade,
            name: entry.name.as_str(),
            level: level + 1.0,
        }
    }

    /// `_scoreMarble`: value a marble level the same way.
    fn score_marble(
        &self,
        water: usize,
        upgrade: usize,
        rates: &Rates,
        marble_target: Option<&MarbleTarget>,
    ) -> Candidate {
        let Some(entry) = super::upgrade(water, upgrade) else {
            return self.empty_candidate(water, upgrade);
        };
        let level = self.fountain.marble_level(water, upgrade);
        let cost = Fountain::marble_cost(water, upgrade, level);
        let remaining = (cost - self.marbles).max(0.0);
        let tta = if rates.marble > 0.0 {
            remaining / rates.marble * 3600.0
        } else {
            f64::INFINITY
        };
        let candidate = self.fountain.with_marble_level(water, upgrade, level + 1.0);
        let new_marble = candidate.marble_per_hr();
        let marble_pct = self.relative_gain(rates.marble, new_marble);
        let desired = self.fixed_desired;
        let current_optimal = self.fountain.measure_goal(Goal::Optimal, desired);
        let new_optimal = candidate.measure_goal(Goal::Optimal, desired);
        let optimal_gain = self.relative_gain(current_optimal, new_optimal);
        let time_saved =
            self.marble_target_time_saved(marble_target, tta, None, cost, &candidate, false);
        let effective_gain = if marble_pct > 0.0 {
            marble_pct * 3.0 + optimal_gain.max(0.0)
        } else if time_saved > 0.0 && marble_target.is_some() {
            marble_target.map_or(0.0, |target| target.m_pct * 3.0 * time_saved)
        } else {
            optimal_gain.max(0.0)
        };
        let invest_hours = if remaining > 0.0 {
            tta / 3600.0
        } else {
            let recoup = if rates.marble > 0.0 {
                cost / rates.marble * 3600.0
            } else {
                f64::INFINITY
            };
            recoup / 3600.0
        };
        let mut score = if effective_gain > 0.0 && invest_hours > 0.0 && invest_hours.is_finite() {
            effective_gain / invest_hours
        } else {
            0.0
        };
        if tta > self.time_limit - self.clock {
            score = 0.0;
        }
        Candidate {
            score,
            tta,
            cost,
            currency: None,
            remaining,
            water,
            upgrade,
            name: entry.name.as_str(),
            level: level + 1.0,
        }
    }

    fn empty_candidate(&self, water: usize, upgrade: usize) -> Candidate {
        Candidate {
            score: 0.0,
            tta: f64::INFINITY,
            cost: 0.0,
            currency: None,
            remaining: 0.0,
            water,
            upgrade,
            name: "Unknown",
            level: 0.0,
        }
    }

    /// `_buyAffordable`: buy every affordable upgrade, best score first.
    fn buy_affordable(&mut self) {
        loop {
            let rates = self.rates();
            let target = self.find_best_marble_target(&rates);
            let mut best: Option<Candidate> = None;
            let mut best_score = 0.0;
            for water in 0..WATERS {
                for upgrade in 0..UPGRADES_PER_WATER {
                    if !self.fountain.upgrade_accessible(water, upgrade) {
                        continue;
                    }
                    let Some(entry) = super::upgrade(water, upgrade) else {
                        continue;
                    };
                    let level = self.fountain.upgrade_level(water, upgrade);
                    let cost = Fountain::upgrade_cost(water, upgrade, level);
                    if self.currencies[entry.currency] < cost {
                        continue;
                    }
                    let candidate = self.score_upgrade(water, upgrade, &rates, target.as_ref());
                    if candidate.score > best_score {
                        best_score = candidate.score;
                        best = Some(candidate);
                    }
                }
            }
            for water in 0..WATERS {
                for upgrade in 0..UPGRADES_PER_WATER {
                    if self.fountain.upgrade_level(water, upgrade) <= 0.0 {
                        continue;
                    }
                    if !Fountain::can_marble(water, upgrade) {
                        continue;
                    }
                    let level = self.fountain.marble_level(water, upgrade);
                    let cost = Fountain::marble_cost(water, upgrade, level);
                    if self.marbles < cost {
                        continue;
                    }
                    let candidate = self.score_marble(water, upgrade, &rates, target.as_ref());
                    if candidate.score > best_score {
                        best_score = candidate.score;
                        best = Some(candidate);
                    }
                }
            }
            let Some(best) = best else {
                break;
            };
            self.execute(&best);
        }
    }

    /// Buy one scored candidate and record it.
    fn execute(&mut self, candidate: &Candidate) {
        if let Some(currency) = candidate.currency {
            self.currencies[currency] -= candidate.cost;
            self.fountain = self.fountain.with_upgrade_level(
                candidate.water,
                candidate.upgrade,
                candidate.level,
            );
            self.push(SimEventKind::BuyUpgrade {
                water: candidate.water,
                upgrade: candidate.upgrade,
                name: candidate.name,
                cost: candidate.cost,
                currency,
                level: candidate.level,
            });
        } else {
            self.marbles -= candidate.cost;
            self.fountain = self.fountain.with_marble_level(
                candidate.water,
                candidate.upgrade,
                candidate.level,
            );
            self.push(SimEventKind::BuyMarble {
                water: candidate.water,
                upgrade: candidate.upgrade,
                name: candidate.name,
                cost: candidate.cost,
                marble_level: candidate.level,
            });
        }
    }

    /// `_advanceTime`: earn for `seconds`, draining the marble bar as it fills.
    ///
    /// After each fill the newly affordable marble levels are bought, so the
    /// marble rate compounds inside the advance.
    fn advance_time(&mut self, seconds: f64, rates: &Rates) -> [f64; CURRENCIES] {
        let mut rates = *rates;
        let mut remaining = seconds;
        let mut earned = [0.0; CURRENCIES];
        while remaining > 0.0 {
            if rates.marble_progress_per_sec <= 0.0 {
                self.earn(remaining, &rates, &mut earned);
                self.clock += remaining;
                break;
            }
            let required = Fountain::marble_bar_fill_time();
            if self.marble_bar_progress >= required {
                self.marble_bar_progress -= required;
            } else {
                let time_to_fill =
                    (required - self.marble_bar_progress) / rates.marble_progress_per_sec;
                if time_to_fill > remaining {
                    self.earn(remaining, &rates, &mut earned);
                    self.marble_bar_progress += rates.marble_progress_per_sec * remaining;
                    self.clock += remaining;
                    break;
                }
                self.earn(time_to_fill, &rates, &mut earned);
                self.marble_bar_progress += rates.marble_progress_per_sec * time_to_fill;
                self.clock += time_to_fill;
                remaining -= time_to_fill;
                self.marble_bar_progress -= required;
            }
            let gained = self.fountain.marble_per_fill();
            self.marbles += gained;
            self.push(SimEventKind::MarbleFill {
                marbles: gained,
                total: self.marbles,
            });
            loop {
                let mut best: Option<Candidate> = None;
                let mut best_score = 0.0;
                for water in 0..WATERS {
                    for upgrade in 0..UPGRADES_PER_WATER {
                        if self.fountain.upgrade_level(water, upgrade) <= 0.0 {
                            continue;
                        }
                        if !Fountain::can_marble(water, upgrade) {
                            continue;
                        }
                        let level = self.fountain.marble_level(water, upgrade);
                        let cost = Fountain::marble_cost(water, upgrade, level);
                        if self.marbles < cost {
                            continue;
                        }
                        let candidate = self.score_marble(water, upgrade, &rates, None);
                        if candidate.score > best_score {
                            best_score = candidate.score;
                            best = Some(candidate);
                        }
                    }
                }
                let Some(best) = best else {
                    break;
                };
                self.execute(&best);
                rates = self.rates();
            }
        }
        earned
    }

    /// The currency every active type earns over `seconds`.
    fn earn(&mut self, seconds: f64, rates: &Rates, earned: &mut [f64; CURRENCIES]) {
        for (currency, rate) in rates.currency.iter().enumerate() {
            let amount = rate / 3600.0 * seconds;
            self.currencies[currency] += amount;
            earned[currency] += amount;
        }
    }

    /// `_pushClaims`: record the currency an advance earned.
    fn push_claims(&mut self, earned: &[f64; CURRENCIES], seconds: f64) {
        for (currency, amount) in earned.iter().enumerate() {
            if *amount <= 0.0 {
                continue;
            }
            self.push(SimEventKind::Claim {
                currency,
                amount: *amount,
                seconds,
            });
        }
    }
}
