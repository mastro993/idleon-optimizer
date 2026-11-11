//! The search driver: the beam's queue, its progress reports, its cancellation
//! and the shared final comparison.
//!
//! Ported from `_searchTdLayouts` (free mode) in
//! `js/stats/systems/w3/worship-td.js`.

use super::edits::{mutate, pick, plan_neighbors, random_plan, unlocked_types};
use super::plans::{Ctx, legalize_plan};
use super::{
    Candidate, Finalist, Neighborhood, Options, Outcome, Score, Search, Trials, better, simulate,
    simulate_trials,
};
use crate::model::{Control, Progress};
use crate::worship::plan::{legal_plan_slots, slot_coverage};
use crate::worship::sim::Rng;
use crate::worship::{GuidePreset, Inputs, Trial, guide_presets_for_trial, heuristic_plan};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

/// How many entries reach the shared final comparison, beside the baselines.
const FINALISTS: usize = 4;

/// Entry indices ranked best first.
fn ranked(entries: &[Candidate]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    indices.sort_by(|a, b| {
        if better(&entries[*a].score, &entries[*b].score) {
            std::cmp::Ordering::Less
        } else if better(&entries[*b].score, &entries[*a].score) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    indices
}

/// The best entries with one per family first, then the rest by rank.
fn diverse(entries: &[Candidate], count: usize) -> Vec<usize> {
    let order = ranked(entries);
    let mut picked: Vec<usize> = Vec::new();
    let mut used: Vec<&str> = Vec::new();
    for index in &order {
        if picked.len() >= count {
            break;
        }
        let family = entries[*index].family.as_str();
        if !used.contains(&family) {
            picked.push(*index);
            used.push(family);
        }
    }
    for index in order {
        if picked.len() >= count {
            break;
        }
        if !picked.contains(&index) {
            picked.push(index);
        }
    }
    picked
}

/// The search, its bookkeeping and the line it reports.
struct SearchRun<'a> {
    inputs: &'a Inputs,
    trial: &'static Trial,
    options: &'a Options,
    ctx: Ctx,
    rng: Rng,
    screening_seeds: Vec<u64>,
    final_seeds: Vec<u64>,
    budget: usize,
    final_reserve: usize,
    entries: Vec<Candidate>,
    seen: HashSet<String>,
    baselines: Vec<usize>,
    evaluations: usize,
    duplicates: usize,
    final_evaluations: usize,
    best: Option<usize>,
    families: HashSet<String>,
    neighborhoods: HashMap<String, Neighborhood>,
}

impl SearchRun<'_> {
    fn total(&self) -> usize {
        self.budget + self.final_reserve
    }

    fn wave(&self) -> f64 {
        self.best
            .map(|index| self.entries[index].score.wave)
            .unwrap_or(0.0)
    }

    fn report(&self, report: &mut dyn FnMut(Progress) -> Control, phase: &str) -> Result<(), ()> {
        let completed = self.evaluations + self.final_evaluations;
        let progress = Progress {
            fraction: (completed as f32 / self.total().max(1) as f32).min(0.99),
            message: format!(
                "{phase} · {completed}/{} plans · best wave {:.0}",
                self.total(),
                self.wave()
            ),
        };
        match report(progress) {
            Control::Stop => Err(()),
            Control::Continue => Ok(()),
        }
    }

    /// Legalize and dedupe a candidate, reserving `reserved` earlier accepts.
    fn accept(&mut self, mut candidate: Candidate, reserved: usize) -> Option<Candidate> {
        if self.evaluations + reserved >= self.budget {
            return None;
        }
        let plan = legalize_plan(self.trial, &candidate.plan, &self.ctx.slots);
        let key = serde_json::to_string(&plan).unwrap_or_default();
        if !self.seen.insert(key) {
            self.duplicates += 1;
            return None;
        }
        candidate.plan = plan;
        Some(candidate)
    }

    /// Record one screened candidate and publish its line.
    fn record(
        &mut self,
        mut candidate: Candidate,
        agg: Trials,
        phase: &str,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Result<usize, ()> {
        let score = Score::of(&agg);
        self.families.insert(candidate.family.clone());
        candidate.score = score;
        candidate.agg = agg;
        candidate.final_score = None;
        let index = self.entries.len();
        let best = self
            .best
            .is_none_or(|best| better(&score, &self.entries[best].score));
        self.entries.push(candidate);
        if best {
            self.best = Some(index);
        }
        self.report(report, phase)?;
        Ok(index)
    }

    /// Screen one candidate. `None` when the budget is spent or the plan was
    /// already screened.
    fn evaluate(
        &mut self,
        candidate: Candidate,
        phase: &str,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Result<Option<usize>, ()> {
        let Some(candidate) = self.accept(candidate, 0) else {
            return Ok(None);
        };
        self.evaluations += 1;
        let agg = simulate_trials(
            self.inputs,
            &candidate.plan,
            self.options,
            &self.screening_seeds,
        );
        self.record(candidate, agg, phase, report).map(Some)
    }

    /// Screen a batch in parallel and record it in batch order, so the plan
    /// set, its order and every reported line stay the same on any machine.
    fn evaluate_batch(
        &mut self,
        batch: Vec<Candidate>,
        phase: &str,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Result<(), ()> {
        let mut accepted: Vec<Candidate> = Vec::new();
        for candidate in batch {
            if let Some(candidate) = self.accept(candidate, accepted.len()) {
                accepted.push(candidate);
            }
        }
        self.evaluations += accepted.len();
        let inputs = self.inputs;
        let options = self.options;
        let seeds = self.screening_seeds.clone();
        let screened: Vec<(Candidate, Trials)> = accepted
            .into_par_iter()
            .map(|candidate| {
                let agg = simulate_trials(inputs, &candidate.plan, options, &seeds);
                (candidate, agg)
            })
            .collect();
        for (candidate, agg) in screened {
            self.record(candidate, agg, phase, report)?;
        }
        Ok(())
    }
}

/// Search tower strategies for one trial.
///
/// The run reports one line per screened plan and one per finalist; returning
/// [`Control::Stop`] from the reporter ends it with [`Outcome::Cancelled`]. Two
/// runs with the same inputs and [`Options`] return the same plan.
pub fn optimize(
    inputs: &Inputs,
    options: &Options,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Outcome {
    let Some(trial) = crate::worship::trial(options.trial) else {
        return Outcome::Refused("The trial is not in the shipped data");
    };
    let types = unlocked_types(inputs);
    if types.is_empty() {
        return Outcome::Refused("No unlocked tower types; import a save first");
    }
    let slots = legal_plan_slots(trial, 10.0);
    if slots.is_empty() {
        return Outcome::Refused("No legal tower slots on this trial");
    }
    let coverage: Vec<f64> = slots
        .iter()
        .map(|slot| slot_coverage(trial, *slot, 160.0) as f64)
        .collect();
    let saved_best = inputs.best_waves.get(options.trial).copied().unwrap_or(0);
    let ctx = Ctx {
        slots,
        coverage,
        types,
        max_towers: options.max_towers.max(1),
        wave_lo: 10,
        wave_hi: saved_best.saturating_add(40),
    };
    let budget = options.budget().max(1);
    let presets: Vec<&'static GuidePreset> = guide_presets_for_trial(options.trial).collect();
    let final_count = FINALISTS;
    let final_reserve = final_count + presets.len() + 1;
    let parallel = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, 8);
    let mut run = SearchRun {
        inputs,
        trial,
        options,
        ctx,
        rng: Rng::new(options.base_seed ^ 0x9e37_79b9),
        screening_seeds: options.screening_seeds(),
        final_seeds: options.final_seeds(),
        budget,
        final_reserve,
        entries: Vec::new(),
        seen: HashSet::new(),
        baselines: Vec::new(),
        evaluations: 0,
        duplicates: 0,
        final_evaluations: 0,
        best: None,
        families: HashSet::new(),
        neighborhoods: HashMap::new(),
    };
    if run.report(report, "Construction").is_err() {
        return Outcome::Cancelled;
    }

    // The plan on screen, the coverage heuristic and every guide preset are the
    // baselines the final comparison must beat. The heuristic is always listed,
    // even when another plan anchors the comparison, so the pane can name the
    // plan the section builds without an optimizer.
    if let Some(seed) = &options.seed_plan
        && !seed.towers.is_empty()
    {
        let candidate = Candidate::new(
            "seed",
            "Current plan".into(),
            "current".into(),
            seed.clone(),
        );
        match run.evaluate(candidate, "Construction", report) {
            Err(()) => return Outcome::Cancelled,
            Ok(Some(index)) => run.baselines.push(index),
            Ok(None) => {}
        }
    }
    let heuristic = Candidate::new(
        "heuristic",
        "Coverage heuristic".into(),
        "mixed".into(),
        heuristic_plan(inputs, trial),
    );
    match run.evaluate(heuristic, "Construction", report) {
        Err(()) => return Outcome::Cancelled,
        Ok(Some(index)) => run.baselines.push(index),
        Ok(None) => {}
    }
    for preset in &presets {
        let candidate = Candidate::new(
            "preset",
            preset.label.to_string(),
            format!("preset:{}", preset.id),
            preset.plan(),
        );
        match run.evaluate(candidate, "Construction", report) {
            Err(()) => return Outcome::Cancelled,
            Ok(Some(index)) => run.baselines.push(index),
            Ok(None) => {}
        }
    }

    // Refine several competing families, consuming every family's neighbors two
    // at a time so the first family cannot spend the whole budget.
    let mut queue: Vec<Candidate> = Vec::new();
    let mut attempts = 0usize;
    while run.evaluations < run.budget && attempts < run.budget * 20 {
        attempts += 1;
        if queue.is_empty() {
            let leaders = diverse(&run.entries, run.options.mode.restarts().max(2));
            let mut batches: Vec<Vec<Candidate>> = Vec::new();
            for leader in &leaders {
                let candidate = run.entries[*leader].clone();
                let key = serde_json::to_string(&candidate.plan).unwrap_or_default();
                if !run.neighborhoods.contains_key(&key) {
                    let family = candidate.family.clone();
                    let mut neighbors: Vec<Candidate> = plan_neighbors(&candidate, trial, &run.ctx)
                        .into_iter()
                        .map(|plan| {
                            Candidate::new(
                                "refined",
                                format!("Tower refinement: {family}"),
                                family.clone(),
                                plan,
                            )
                        })
                        .collect();
                    for slot in 0..4 {
                        let at = (2 + slot * 8).min(neighbors.len());
                        let plan = mutate(&candidate.plan, trial, &mut run.rng, &run.ctx);
                        neighbors.insert(
                            at,
                            Candidate::new(
                                "refined",
                                format!("Beam mutation: {family}"),
                                family.clone(),
                                plan,
                            ),
                        );
                    }
                    run.neighborhoods.insert(
                        key.clone(),
                        Neighborhood {
                            list: neighbors,
                            at: 0,
                        },
                    );
                }
                let Some(neighborhood) = run.neighborhoods.get_mut(&key) else {
                    continue;
                };
                let end = (neighborhood.at + 2).min(neighborhood.list.len());
                batches.push(neighborhood.list[neighborhood.at..end].to_vec());
                neighborhood.at = end;
            }
            let mut round = 0;
            while batches.iter().any(|batch| round < batch.len()) {
                for batch in &batches {
                    if let Some(candidate) = batch.get(round) {
                        queue.push(candidate.clone());
                    }
                }
                round += 1;
            }
            if queue.is_empty() {
                let parent = *pick(&mut run.rng, &leaders);
                let candidate = run.entries[parent].clone();
                let plan = mutate(&candidate.plan, trial, &mut run.rng, &run.ctx);
                queue.push(Candidate::new(
                    "refined",
                    "Beam mutation".into(),
                    candidate.family,
                    plan,
                ));
                let plan = random_plan(trial, &mut run.rng, &run.ctx);
                queue.push(Candidate::new(
                    "random",
                    "Independent construction".into(),
                    "random".into(),
                    plan,
                ));
            }
        }
        let take = parallel.min(queue.len());
        let batch: Vec<Candidate> = queue.drain(..take).collect();
        if run.evaluate_batch(batch, "Refinement", report).is_err() {
            return Outcome::Cancelled;
        }
    }

    // Compare the same final seeds across finalists and baselines, so fresh
    // randomness cannot erase a baseline that screened poorly.
    let mut finals = ranked(&run.entries);
    finals.truncate(final_count.min(2));
    for index in diverse(&run.entries, final_count) {
        if finals.len() < final_count && !finals.contains(&index) {
            finals.push(index);
        }
    }
    for index in run.baselines.clone() {
        if !finals.contains(&index) {
            finals.push(index);
        }
    }
    let mut winner: Option<usize> = None;
    let rescreened: Vec<Trials> = finals
        .par_iter()
        .map(|index| {
            let entry = run.entries[*index].clone();
            let plan = legalize_plan(trial, &entry.plan, &run.ctx.slots);
            simulate_trials(inputs, &plan, options, &run.final_seeds)
        })
        .collect();
    for (position, index) in finals.iter().enumerate() {
        let agg = rescreened[position].clone();
        let score = Score::of(&agg);
        run.entries[*index].agg = agg;
        run.entries[*index].final_score = Some(score);
        let ahead = winner.is_none_or(|best| {
            let best = run.entries[best]
                .final_score
                .expect("the current winner was scored");
            better(&score, &best)
        });
        if ahead {
            winner = Some(*index);
        }
        run.final_evaluations += 1;
        if run.report(report, "Final comparison").is_err() {
            return Outcome::Cancelled;
        }
    }
    let Some(winner) = winner else {
        return Outcome::Refused("The search found no plan");
    };
    let _ = run.report(report, "Done");
    let finalist = |index: usize| Finalist {
        label: run.entries[index].label.clone(),
        score: run.entries[index]
            .final_score
            .unwrap_or(run.entries[index].score),
    };
    let finalists: Vec<Finalist> = run
        .entries
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.final_score.is_some())
        .map(|(index, _)| finalist(index))
        .collect();
    let baselines: Vec<Finalist> = run.baselines.iter().map(|index| finalist(*index)).collect();
    let candidate = &run.entries[winner];
    let detail = candidate
        .agg
        .runs
        .first()
        .cloned()
        .unwrap_or_else(|| simulate(inputs, &candidate.plan, options, run.final_seeds[0]));
    Outcome::Found(Box::new(Search {
        trial: options.trial,
        kind: candidate.kind,
        label: candidate.label.clone(),
        plan: candidate.plan.clone(),
        score: candidate.final_score.expect("the winner was scored"),
        detail,
        mean_best_wave: candidate.agg.mean_best_wave,
        min_best_wave: candidate.agg.min_best_wave,
        max_best_wave: candidate.agg.max_best_wave,
        saved_best,
        evaluations: run.evaluations + run.final_evaluations,
        seeds: run.final_seeds.clone(),
        finalists,
        baselines,
        duplicates_skipped: run.duplicates,
        families: run.families.len(),
    }))
}
