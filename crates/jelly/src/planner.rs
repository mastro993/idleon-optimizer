//! The board section planner: which plot tokens to buy, in what order, and how
//! the board fills around them.
//!
//! Ports `planJellySections` and its helpers from
//! `js/stats/systems/w7/jelly-operator.js`: `jellySectionGeometry`,
//! `jellyCompleteLayout`, `jellyQualityOptions`, `jellyStrategyOptions`,
//! `proxyObjectiveScore`, `simulateSectionCandidates`, `compareSectionRows`,
//! `pairedOperationConfidence` and `upgradeMarginalScore`. A plan screens every
//! unpurchased section by geometry, keeps the strongest with a bounded beam,
//! and compares the finalists with shared-seed trials so one section's luck
//! cannot outrank a better board.
//!
//! The planner is deterministic: the trials are seeded, every sort breaks its
//! ties on plot ids, and no map iteration order reaches the result.

use super::board::{self, Layout};
use super::data::Plot;
use super::estimate::{Estimate, mean, standard_error, summarize};
use super::operation::{OperationOptions, simulate};
use super::surrogate::Objective as SurrogateObjective;
use super::{Jelly, PROXIMITY_ANCHORS, data};
use idleon_common::Progress;
use std::cmp::Ordering;

/// What a plan maximizes (`objectiveOptions`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Objective {
    /// Highest clear chance, then the fastest clear.
    #[default]
    Clear,
    /// Most Bloodcells per attempt.
    Bloodcells,
    /// Most Cell EXP per attempt.
    Exp,
    /// Highest operation damage.
    Dps,
    /// Longest Critical survival.
    Survival,
}

impl Objective {
    pub fn id(self) -> &'static str {
        match self {
            Objective::Clear => "clear",
            Objective::Bloodcells => "bloodcells",
            Objective::Exp => "exp",
            Objective::Dps => "dps",
            Objective::Survival => "survival",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Objective::Clear => "Highest clear chance, then fastest",
            Objective::Bloodcells => "Most Bloodcells per attempt",
            Objective::Exp => "Most Cell EXP per attempt",
            Objective::Dps => "Highest operation damage",
            Objective::Survival => "Longest Critical survival",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [
            Objective::Clear,
            Objective::Bloodcells,
            Objective::Exp,
            Objective::Dps,
            Objective::Survival,
        ]
        .into_iter()
        .find(|objective| objective.id() == id)
    }

    /// The metric the page's chooseable objectives require: Bloodcells need
    /// the Currency upgrade, EXP the Cell EXP upgrade and Survival Critical.
    pub fn visible(self, jelly: &Jelly) -> bool {
        match self {
            Objective::Clear | Objective::Dps => true,
            Objective::Bloodcells => jelly.upgrade_level(23) > 0.0,
            Objective::Exp => jelly.upgrade_level(10) > 0.0,
            Objective::Survival => jelly.upgrade_level(36) > 0.0,
        }
    }

    pub(crate) fn surrogate(self) -> SurrogateObjective {
        match self {
            Objective::Clear => SurrogateObjective::Clear,
            Objective::Bloodcells => SurrogateObjective::Bloodcells,
            Objective::Exp => SurrogateObjective::Exp,
            Objective::Dps => SurrogateObjective::Dps,
            Objective::Survival => SurrogateObjective::Survival,
        }
    }
}

/// The ranking preset (`JELLY_STRATEGY_PROFILES`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Strategy {
    Conservative,
    #[default]
    Balanced,
    Aggressive,
    /// The page's default: progress now.
    Progression,
    Bloodcells,
    Exp,
    Critical,
    LongTerm,
    LowRisk,
}

impl Strategy {
    pub fn id(self) -> &'static str {
        match self {
            Strategy::Conservative => "conservative",
            Strategy::Balanced => "balanced",
            Strategy::Aggressive => "aggressive",
            Strategy::Progression => "progression",
            Strategy::Bloodcells => "bloodcells",
            Strategy::Exp => "exp",
            Strategy::Critical => "critical",
            Strategy::LongTerm => "long-term",
            Strategy::LowRisk => "low-risk",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Strategy::Conservative => "Conservative",
            Strategy::Balanced => "Balanced",
            Strategy::Aggressive => "Aggressive",
            Strategy::Progression => "Progress now",
            Strategy::Bloodcells => "Farm Bloodcells",
            Strategy::Exp => "Level cells",
            Strategy::Critical => "Critical push",
            Strategy::LongTerm => "Long-term progression",
            Strategy::LowRisk => "Low-risk gains",
        }
    }

    /// The six presets the page offers, in its order.
    pub const PAGE: [Strategy; 6] = [
        Strategy::Progression,
        Strategy::Bloodcells,
        Strategy::Exp,
        Strategy::Critical,
        Strategy::LongTerm,
        Strategy::LowRisk,
    ];

    pub fn from_id(id: &str) -> Option<Self> {
        [
            Strategy::Conservative,
            Strategy::Balanced,
            Strategy::Aggressive,
            Strategy::Progression,
            Strategy::Bloodcells,
            Strategy::Exp,
            Strategy::Critical,
            Strategy::LongTerm,
            Strategy::LowRisk,
        ]
        .into_iter()
        .find(|strategy| strategy.id() == id)
    }

    /// `strategyObjective`: the strategy a pick names, or `None` to keep the
    /// current objective.
    pub fn objective(self) -> Option<Objective> {
        match self {
            Strategy::Progression => Some(Objective::Clear),
            Strategy::Bloodcells | Strategy::LongTerm => Some(Objective::Bloodcells),
            Strategy::Exp => Some(Objective::Exp),
            Strategy::Critical => Some(Objective::Survival),
            Strategy::Conservative
            | Strategy::Balanced
            | Strategy::Aggressive
            | Strategy::LowRisk => None,
        }
    }

    fn profile(self) -> (usize, usize, i8) {
        // candidateCap, beamWidth, confidenceWeight.
        match self {
            Strategy::Conservative => (8, 6, -1),
            Strategy::Balanced => (12, 8, 0),
            Strategy::Aggressive => (16, 10, 1),
            Strategy::Progression | Strategy::Bloodcells | Strategy::Exp | Strategy::Critical => {
                (12, 8, 0)
            }
            Strategy::LongTerm => (16, 10, 1),
            Strategy::LowRisk => (8, 6, -1),
        }
    }
}

/// The screening stages a plan runs (`JELLY_QUALITY_PROFILES`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Quality {
    Fast,
    #[default]
    Balanced,
    Thorough,
}

impl Quality {
    pub fn id(self) -> &'static str {
        match self {
            Quality::Fast => "fast",
            Quality::Balanced => "balanced",
            Quality::Thorough => "thorough",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Quality::Fast => "Fast (8 → 32 → 128)",
            Quality::Balanced => "Balanced (16 → 64 → 256)",
            Quality::Thorough => "Thorough (32 → 128 → 512)",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        [Quality::Fast, Quality::Balanced, Quality::Thorough]
            .into_iter()
            .find(|quality| quality.id() == id)
    }

    fn stages(self) -> [u32; 3] {
        match self {
            Quality::Fast => [8, 32, 128],
            Quality::Balanced => [16, 64, 256],
            Quality::Thorough => [32, 128, 512],
        }
    }
}

/// The knobs one plan run reads.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanOptions {
    pub strategy: Strategy,
    pub objective: Objective,
    /// The EXP cell type objective EXP maximizes; `None` is total EXP.
    pub exp_cell_type: Option<usize>,
    /// How many sections to unlock; `None` uses the save's unused tokens.
    pub tokens: Option<usize>,
    /// Whether to plan sections the save cannot afford yet.
    pub allow_hypothetical_tokens: bool,
    pub quality: Quality,
    /// The behaviour combination's beam width; `0` keeps the strategy preset.
    pub beam_width: usize,
    pub seed: u32,
    /// The board to plan from; `None` uses the save's layout.
    pub layout: Option<Layout>,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            strategy: Strategy::default(),
            objective: Objective::default(),
            exp_cell_type: None,
            tokens: None,
            allow_hypothetical_tokens: false,
            quality: Quality::default(),
            beam_width: 0,
            seed: 1,
            layout: None,
        }
    }
}

/// The quality profile the plan ran (`jellyQualityOptions`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QualityProfile {
    pub screening_trials: u32,
    pub racing_trials: u32,
    pub final_trials: u32,
    pub stages: Vec<u32>,
}

/// How many new large-cell placements a section opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FootprintPlacement {
    pub cell_type: usize,
    pub name: String,
    pub count: usize,
}

/// The paired confidence one candidate has against the baseline
/// (`pairedOperationConfidence`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Confidence {
    pub trials: usize,
    pub chance_beats_baseline: f64,
    pub mean_difference: f64,
    pub low: f64,
    pub high: f64,
}

/// One ranked section or section sequence.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionRow {
    /// The single plot, for the individual-section table.
    pub plot_id: Option<usize>,
    pub start: Option<usize>,
    pub width: Option<usize>,
    pub height: Option<usize>,
    /// The plot ids the row buys, in ascending order.
    pub plot_ids: Vec<usize>,
    pub new_usable_slots: usize,
    pub new_slots: Vec<usize>,
    pub proximity_slots: usize,
    pub newly_legal_footprint_placements: Vec<FootprintPlacement>,
    /// The board the row fills once its sections are bought.
    pub layout: Layout,
    pub proxy_score: f64,
    pub proxy_gain: f64,
    /// The finalists' shared-seed aggregate; a pruned row keeps the stage it
    /// last ran.
    pub operation: Option<Estimate>,
    pub objective_value: f64,
    pub objective_gain: f64,
    pub gain_per_slot: f64,
    pub confidence: Option<Confidence>,
    pub chance_beats_baseline: Option<f64>,
    pub simulation_trials: Option<u32>,
    pub finalist: bool,
    /// The save with the row's plots bought, which its trials run against.
    pub(crate) expanded: Jelly,
}

/// One board section plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub objective: Objective,
    /// `None` is total EXP, as the page's `-1` target is.
    pub exp_cell_type: Option<usize>,
    pub quality: Quality,
    pub quality_profile: QualityProfile,
    pub strategy: Strategy,
    /// The effective beam width after the strategy preset's clamp.
    pub beam_width: usize,
    pub tokens: usize,
    pub requested_tokens: usize,
    pub available_tokens: usize,
    pub baseline_layout: Layout,
    pub baseline_unlocked_slots: Vec<usize>,
    pub baseline_obstruction: usize,
    pub baseline_operation: Option<Estimate>,
    /// Every unpurchased section, ranked.
    pub plots: Vec<SectionRow>,
    /// The best section sequences, ranked.
    pub sequences: Vec<SectionRow>,
    pub recommended: Option<SectionRow>,
    pub final_layout: Layout,
    pub final_operation: Option<Estimate>,
    /// The page's one-line explanation of the method.
    pub note: String,
}

const NOTE: &str = "Every unpurchased section is geometry-screened. Multi-token candidates use bounded non-greedy beam search, shared seeds, adaptive simulation, and a fully occupied legal final layout. Statistically overlapping clear results prefer faster clears, then higher damage.";

impl Plan {
    /// Rank the sections once, without progress reports.
    pub fn run(jelly: &Jelly, options: PlanOptions) -> Plan {
        Self::run_with(jelly, options, |_| true).expect("a plan that never cancels finishes")
    }

    /// The same run, reporting after each screened section and each simulated
    /// candidate. The callback returns whether to keep going; a stopped run
    /// reports `None`.
    pub fn run_with(
        jelly: &Jelly,
        options: PlanOptions,
        mut report: impl FnMut(Progress) -> bool,
    ) -> Option<Plan> {
        let objective = options.objective;
        let exp_cell_type = normalize_exp_cell_type(jelly, options.exp_cell_type);
        let quality = quality_options(options.quality);
        let profile = strategy_options(options.strategy, options.beam_width);
        let baseline_layout = options
            .layout
            .clone()
            .unwrap_or_else(|| jelly.layout.clone());
        let purchased: Vec<usize> = jelly.plots.clone();
        let available_plots: Vec<&Plot> = data()
            .plots
            .iter()
            .filter(|plot| !purchased.contains(&plot.plot_id))
            .collect();
        let available_tokens = board::slot_purchases_left(jelly).max(0.0) as usize;
        let requested_tokens = options.tokens.unwrap_or(available_tokens.min(1)).min(40);
        let budget = if options.allow_hypothetical_tokens {
            requested_tokens
        } else {
            requested_tokens.min(available_tokens)
        };
        let tokens = budget.min(available_plots.len());
        let options = PlanOptions {
            exp_cell_type,
            ..options
        };
        let total = (available_plots.len()
            + profile.candidate_cap * tokens.max(1) * quality.stages.len())
        .max(1) as f64;
        let mut reporter = Reporter {
            report: &mut report,
            completed: 0.0,
            total,
        };

        let baseline_proxy = proxy_score(jelly, &baseline_layout, &options);
        let mut plot_rows: Vec<SectionRow> = Vec::with_capacity(available_plots.len());
        for plot in &available_plots {
            let geometry = section_geometry(jelly, &[plot.plot_id]);
            let layout = complete_layout_with(
                &geometry.expanded,
                &baseline_layout,
                &options,
                Some(&geometry),
            );
            let proxy = proxy_score(&geometry.expanded, &layout, &options);
            plot_rows.push(SectionRow {
                plot_id: Some(plot.plot_id),
                start: Some(plot.start),
                width: Some(plot.width),
                height: Some(plot.height),
                plot_ids: geometry.plot_ids.clone(),
                new_usable_slots: geometry.new_slots.len(),
                new_slots: geometry.new_slots.clone(),
                proximity_slots: geometry.proximity_slots,
                newly_legal_footprint_placements: geometry.newly_legal.clone(),
                layout,
                proxy_score: proxy,
                proxy_gain: proxy - baseline_proxy,
                expanded: geometry.expanded,
                ..SectionRow::default()
            });
            if !reporter.step("Screening section geometry", 1.0) {
                return None;
            }
        }

        let mut baseline_operation = None;
        if tokens > 0 {
            baseline_operation = simulate_candidates(
                &mut plot_rows,
                &baseline_layout,
                jelly,
                &options,
                &profile,
                &quality,
                &mut reporter,
            )?;
        }

        let mut sequence_rows: Vec<SectionRow> = Vec::new();
        if tokens == 1 {
            sequence_rows = plot_rows.clone();
        } else if tokens > 1 {
            let mut ranked: Vec<&SectionRow> = plot_rows.iter().collect();
            ranked.sort_by(|a, b| {
                difference(a.proxy_gain, b.proxy_gain)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| plot_key(&a.plot_ids).cmp(&plot_key(&b.plot_ids)))
            });
            let mut frontier: Vec<Vec<usize>> = ranked
                .into_iter()
                .take(profile.beam_width)
                .map(|row| row.plot_ids.clone())
                .collect();
            for depth in 2..=tokens {
                let mut sequences: Vec<Vec<usize>> = Vec::new();
                if depth == 2 {
                    for first in 0..available_plots.len() {
                        for second in first + 1..available_plots.len() {
                            sequences.push(vec![
                                available_plots[first].plot_id,
                                available_plots[second].plot_id,
                            ]);
                        }
                    }
                } else {
                    for sequence in &frontier {
                        for plot in &available_plots {
                            if sequence.contains(&plot.plot_id) {
                                continue;
                            }
                            let mut next = sequence.clone();
                            next.push(plot.plot_id);
                            next.sort_unstable();
                            sequences.push(next);
                        }
                    }
                }
                let mut seen = std::collections::BTreeSet::new();
                let mut screened: Vec<SectionRow> = Vec::new();
                for plot_ids in sequences {
                    if !seen.insert(plot_key(&plot_ids)) {
                        continue;
                    }
                    let geometry = section_geometry(jelly, &plot_ids);
                    let layout = complete_layout_with(
                        &geometry.expanded,
                        &baseline_layout,
                        &options,
                        Some(&geometry),
                    );
                    let proxy = proxy_score(&geometry.expanded, &layout, &options);
                    screened.push(SectionRow {
                        plot_ids: geometry.plot_ids.clone(),
                        new_usable_slots: geometry.new_slots.len(),
                        new_slots: geometry.new_slots.clone(),
                        proximity_slots: geometry.proximity_slots,
                        newly_legal_footprint_placements: geometry.newly_legal.clone(),
                        layout,
                        proxy_score: proxy,
                        proxy_gain: proxy - baseline_proxy,
                        expanded: geometry.expanded,
                        ..SectionRow::default()
                    });
                    if !reporter.step("Screening section sequences", 0.0) {
                        return None;
                    }
                }
                screened.sort_by(|a, b| {
                    difference(a.proxy_gain, b.proxy_gain)
                        .unwrap_or(Ordering::Equal)
                        .then_with(|| plot_key(&a.plot_ids).cmp(&plot_key(&b.plot_ids)))
                });
                frontier = screened
                    .iter()
                    .take(profile.beam_width)
                    .map(|row| row.plot_ids.clone())
                    .collect();
                if depth == tokens {
                    sequence_rows = screened.into_iter().take(profile.candidate_cap).collect();
                    baseline_operation = simulate_candidates(
                        &mut sequence_rows,
                        &baseline_layout,
                        jelly,
                        &options,
                        &profile,
                        &quality,
                        &mut reporter,
                    )?;
                }
            }
        }

        sequence_rows.sort_by(|a, b| compare_rows(a, b, &options, &profile));
        plot_rows.sort_by(|a, b| {
            compare_rows(a, b, &options, &profile).then_with(|| {
                a.plot_id
                    .unwrap_or(usize::MAX)
                    .cmp(&b.plot_id.unwrap_or(usize::MAX))
            })
        });
        let recommended = sequence_rows.first().cloned();
        let final_layout = recommended
            .as_ref()
            .map(|row| row.layout.clone())
            .unwrap_or_else(|| baseline_layout.clone());
        let final_operation = recommended
            .as_ref()
            .and_then(|row| row.operation.clone())
            .or_else(|| baseline_operation.clone());
        if !reporter.step("Section plan complete", 0.0) {
            return None;
        }
        let unlocked = board::unlocked_slots(jelly);
        Some(Plan {
            objective,
            exp_cell_type,
            quality: options.quality,
            quality_profile: quality,
            strategy: options.strategy,
            beam_width: profile.beam_width,
            tokens,
            requested_tokens,
            available_tokens,
            baseline_layout,
            baseline_unlocked_slots: (0..unlocked.len()).filter(|slot| unlocked[*slot]).collect(),
            baseline_obstruction: jelly.progress.obstruction,
            baseline_operation,
            plots: plot_rows,
            sequences: sequence_rows,
            recommended,
            final_layout,
            final_operation,
            note: NOTE.to_string(),
        })
    }
}

/// One strategy preset, resolved.
struct StrategyProfile {
    candidate_cap: usize,
    beam_width: usize,
    confidence_weight: i8,
}

fn strategy_options(strategy: Strategy, beam_width: usize) -> StrategyProfile {
    let (candidate_cap, preset_beam, confidence_weight) = strategy.profile();
    let beam = if beam_width == 0 {
        preset_beam
    } else {
        beam_width
    };
    StrategyProfile {
        candidate_cap: candidate_cap.clamp(2, 24),
        beam_width: beam.clamp(2, 20),
        confidence_weight,
    }
}

fn quality_options(quality: Quality) -> QualityProfile {
    let [screening_trials, racing_trials, final_trials] = quality.stages();
    let mut stages = vec![screening_trials, racing_trials, final_trials];
    stages.dedup();
    QualityProfile {
        screening_trials,
        racing_trials,
        final_trials,
        stages,
    }
}

/// `normalizeExpCellType`: the save's own `-1` target, else a owned cell type.
fn normalize_exp_cell_type(jelly: &Jelly, value: Option<usize>) -> Option<usize> {
    value.map(|cell_type| cell_type.min(jelly.units_owned().saturating_sub(1)))
}

/// The save with the plots bought (`saveWithJellyPlots`).
pub fn save_with_plots(jelly: &Jelly, plot_ids: &[usize]) -> Jelly {
    let mut expanded = jelly.clone();
    for plot_id in plot_ids {
        if !expanded.plots.contains(plot_id) {
            expanded.plots.push(*plot_id);
        }
    }
    expanded
}

/// `jellyLegalPlacementAnchors`: every anchor whose footprint is unlocked.
fn legal_placement_anchors(jelly: &Jelly, cell_type: usize) -> Vec<usize> {
    let unlocked = board::unlocked_slots(jelly);
    (0..data().size())
        .filter(|anchor| {
            board::footprint(*anchor, cell_type).is_some_and(|slots| {
                slots
                    .iter()
                    .all(|slot| unlocked.get(*slot).copied() == Some(true))
            })
        })
        .collect()
}

/// One section's geometry (`jellySectionGeometry`).
struct Geometry {
    plot_ids: Vec<usize>,
    new_slots: Vec<usize>,
    proximity_slots: usize,
    newly_legal: Vec<FootprintPlacement>,
    placement_anchors_by_type: Vec<Vec<usize>>,
    expanded: Jelly,
}

fn section_geometry(jelly: &Jelly, plot_ids: &[usize]) -> Geometry {
    let base = board::unlocked_slots(jelly);
    let expanded = save_with_plots(jelly, plot_ids);
    let open = board::unlocked_slots(&expanded);
    let new_slots: Vec<usize> = (0..open.len())
        .filter(|slot| open[*slot] && !base.get(*slot).copied().unwrap_or(false))
        .collect();
    let mut newly_legal = Vec::new();
    let mut placement_anchors_by_type = Vec::new();
    for cell_type in 0..jelly.units_owned() {
        let before = legal_placement_anchors(jelly, cell_type);
        let anchors: Vec<usize> = legal_placement_anchors(&expanded, cell_type)
            .into_iter()
            .filter(|anchor| !before.contains(anchor))
            .collect();
        newly_legal.push(FootprintPlacement {
            cell_type,
            name: data().cell_name(cell_type).to_string(),
            count: anchors.len(),
        });
        placement_anchors_by_type.push(anchors);
    }
    Geometry {
        plot_ids: plot_ids.to_vec(),
        proximity_slots: new_slots
            .iter()
            .filter(|slot| PROXIMITY_ANCHORS.contains(slot))
            .count(),
        new_slots,
        newly_legal,
        placement_anchors_by_type,
        expanded,
    }
}

/// `proxyObjectiveScore`: the deterministic projection a layout ranks by.
fn proxy_score(jelly: &Jelly, layout: &Layout, options: &PlanOptions) -> f64 {
    super::surrogate::proxy_score(
        jelly,
        layout,
        options.objective.surrogate(),
        options.exp_cell_type,
        None,
    )
}

/// `jellyCompleteLayout`: fill every unlocked slot with the best one-slot cell,
/// then try the new large-cell anchors the section opens.
fn complete_layout_with(
    jelly: &Jelly,
    layout: &Layout,
    options: &PlanOptions,
    geometry: Option<&Geometry>,
) -> Layout {
    let unlocked = board::unlocked_slots(jelly);
    let types: Vec<usize> = (0..jelly.units_owned()).collect();
    let one_slot: Vec<usize> = types
        .iter()
        .copied()
        .filter(|cell_type| data().footprint(*cell_type).len() == 1)
        .collect();
    let mut completed = if board::validate(jelly, layout).is_ok() {
        layout.clone()
    } else {
        Layout::new()
    };
    let mut occupied = board::occupancy(&completed);
    for slot in 0..unlocked.len() {
        if !unlocked[slot] || occupied.contains_key(&slot) {
            continue;
        }
        let mut best: Option<(f64, Layout)> = None;
        for cell_type in &one_slot {
            let Ok(candidate) = board::place(jelly, &completed, slot, *cell_type) else {
                continue;
            };
            // A footprint that overlaps an existing cell would remove it.
            if candidate.len() != completed.len() + 1 {
                continue;
            }
            let score = proxy_score(jelly, &candidate, options);
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, candidate));
            }
        }
        if let Some((_, best)) = best {
            completed = best;
            occupied.insert(slot, (slot, completed[&slot]));
        }
    }

    let mut current_score = proxy_score(jelly, &completed, options);
    if let Some(geometry) = geometry {
        let mut placements = Vec::new();
        for (cell_type, anchors) in geometry.placement_anchors_by_type.iter().enumerate() {
            if data().footprint(cell_type).len() <= 1 {
                continue;
            }
            for anchor in anchors {
                placements.push((cell_type, *anchor));
            }
        }
        for (cell_type, anchor) in placements.into_iter().take(24) {
            let Ok(placed) = board::place(jelly, &completed, anchor, cell_type) else {
                continue;
            };
            let candidate = complete_layout_with(jelly, &placed, options, None);
            let score = proxy_score(jelly, &candidate, options);
            if score > current_score {
                completed = candidate;
                current_score = score;
            }
        }
    }
    completed
}

/// The operation's objective value for the ranking (`operationObjectiveValue`).
pub(crate) fn operation_value(
    operation: &Estimate,
    objective: Objective,
    exp_cell_type: Option<usize>,
) -> f64 {
    if !operation.valid {
        return 0.0;
    }
    match objective {
        Objective::Clear => operation.clear_probability,
        Objective::Bloodcells => operation.mean_bloodcells,
        Objective::Exp => exp_value(operation, exp_cell_type),
        Objective::Survival => operation.mean_critical_time,
        Objective::Dps => operation.mean_damage,
    }
}

pub(crate) fn exp_value(operation: &Estimate, exp_cell_type: Option<usize>) -> f64 {
    match exp_cell_type {
        Some(cell_type) => operation
            .mean_cell_exp_by_type
            .get(cell_type)
            .copied()
            .unwrap_or(0.0),
        None => operation.mean_cell_exp,
    }
}

/// `trialObjectiveValue`: one trial's contribution to the paired difference.
fn trial_value(
    trial: &super::operation::Trial,
    objective: Objective,
    exp_cell_type: Option<usize>,
) -> f64 {
    match objective {
        Objective::Clear => {
            if trial.success {
                1.0
            } else {
                0.0
            }
        }
        Objective::Bloodcells => trial.bloodcells,
        Objective::Exp => match exp_cell_type {
            Some(cell_type) => trial.exp_by_type.get(cell_type).copied().unwrap_or(0.0),
            None => trial.cell_exp,
        },
        Objective::Survival => trial.critical_time,
        Objective::Dps => trial.damage,
    }
}

/// One shared-seed trial set: its aggregate and each trial's objective value.
#[derive(Clone, Debug, PartialEq)]
pub struct TrialSet {
    pub estimate: Estimate,
    pub values: Vec<f64>,
}

/// The run's progress and its cancel point, shared by the screens and stages.
struct Reporter<'a> {
    report: &'a mut dyn FnMut(Progress) -> bool,
    completed: f64,
    total: f64,
}

impl Reporter<'_> {
    /// Add `steps` of the plan's total and report; `steps` may be zero when the
    /// caller only wants a cancel check.
    fn step(&mut self, message: impl Into<String>, steps: f64) -> bool {
        self.completed = (self.completed + steps / self.total.max(1.0)).min(1.0);
        (self.report)(Progress {
            fraction: self.completed as f32,
            message: message.into(),
        })
    }
}

pub(crate) fn trial_set(
    jelly: &Jelly,
    layout: &Layout,
    trials: u32,
    seed: u32,
    objective: Objective,
    exp_cell_type: Option<usize>,
) -> TrialSet {
    let trials = trials.clamp(1, 8192);
    let mut results = Vec::with_capacity(trials as usize);
    let mut values = Vec::with_capacity(trials as usize);
    for index in 0..trials {
        let trial = simulate(
            jelly,
            layout,
            OperationOptions {
                seed: seed.wrapping_add(index.wrapping_mul(2_654_435_761)),
                ..OperationOptions::default()
            },
        );
        values.push(trial_value(&trial, objective, exp_cell_type));
        results.push(trial);
    }
    TrialSet {
        estimate: summarize(&results, seed),
        values,
    }
}

/// `pairedOperationConfidence`.
pub(crate) fn paired_confidence(candidate: &[f64], baseline: &[f64]) -> Option<Confidence> {
    let count = candidate.len().min(baseline.len());
    if count == 0 {
        return None;
    }
    let mut differences = Vec::with_capacity(count);
    let mut wins = 0usize;
    let mut ties = 0usize;
    for index in 0..count {
        let difference = candidate[index] - baseline[index];
        differences.push(difference);
        if difference > 0.0 {
            wins += 1;
        } else if difference == 0.0 {
            ties += 1;
        }
    }
    let difference = mean(&differences);
    let margin = 1.96 * standard_error(&differences);
    Some(Confidence {
        trials: count,
        chance_beats_baseline: (wins as f64 + 0.5 * ties as f64) / count as f64,
        mean_difference: difference,
        low: difference - margin,
        high: difference + margin,
    })
}

/// `upgradeMarginalScore`: what one candidate adds over the baseline.
fn marginal_score(
    baseline: &Estimate,
    candidate: &Estimate,
    objective: Objective,
    exp_cell_type: Option<usize>,
) -> f64 {
    match objective {
        Objective::Bloodcells => candidate.mean_bloodcells - baseline.mean_bloodcells,
        Objective::Exp => exp_value(candidate, exp_cell_type) - exp_value(baseline, exp_cell_type),
        Objective::Survival => candidate.mean_critical_time - baseline.mean_critical_time,
        Objective::Dps => candidate.mean_damage - baseline.mean_damage,
        Objective::Clear => {
            let probability_gain = candidate.clear_probability - baseline.clear_probability;
            if probability_gain.abs() > 1e-12 {
                return probability_gain;
            }
            if candidate.clear_probability > 0.0
                && baseline.mean_time != 0.0
                && candidate.mean_time != 0.0
            {
                let gain = baseline.mean_time - candidate.mean_time;
                gain / baseline.representative.standard_time.max(1.0) / 100.0
            } else {
                let hp = baseline.representative.hp.max(1.0);
                let before = baseline.mean_damage / hp;
                let after = candidate.mean_damage / hp;
                (after - before) / 1000.0
            }
        }
    }
}

/// `simulatedObjectiveScore`.
pub(crate) fn simulated_score(
    operation: &Estimate,
    objective: Objective,
    exp_cell_type: Option<usize>,
) -> f64 {
    if !operation.valid {
        return f64::NEG_INFINITY;
    }
    match objective {
        Objective::Clear => {
            let clear_time = if operation.mean_time != 0.0 {
                operation.mean_time
            } else {
                1e9
            };
            operation.clear_probability * 1e15 - clear_time * 1e6 + operation.mean_damage
        }
        Objective::Bloodcells => operation.mean_bloodcells,
        Objective::Exp => exp_value(operation, exp_cell_type),
        Objective::Survival => operation.mean_critical_time * 1e9 + operation.mean_damage,
        Objective::Dps => operation.mean_damage,
    }
}

/// `sectionSimulationTieBreak`.
fn simulation_tie_break(
    row: &SectionRow,
    objective: Objective,
    exp_cell_type: Option<usize>,
) -> f64 {
    let Some(operation) = &row.operation else {
        return f64::NEG_INFINITY;
    };
    if objective == Objective::Clear {
        -operation.mean_time * 1e9 + operation.mean_damage
    } else {
        simulated_score(operation, objective, exp_cell_type)
    }
}

/// `sectionStrategyScore`.
fn strategy_score(row: &SectionRow, profile: &StrategyProfile) -> f64 {
    let Some(confidence) = &row.confidence else {
        return row.objective_gain;
    };
    if profile.confidence_weight == 0 {
        return row.objective_gain;
    }
    if profile.confidence_weight < 0 {
        return confidence.low;
    }
    row.objective_gain + (confidence.high - row.objective_gain).max(0.0) * 0.25
}

/// The plot ids, joined as the page's `plotIds.join(',')` does.
fn plot_key(ids: &[usize]) -> String {
    ids.iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// The JavaScript comparator's truthiness: a non-zero, non-NaN difference.
fn difference(before: f64, after: f64) -> Option<Ordering> {
    let difference = after - before;
    if difference == 0.0 || difference.is_nan() {
        return None;
    }
    Some(if difference > 0.0 {
        Ordering::Greater
    } else {
        Ordering::Less
    })
}

/// The `compareSectionRows` comparator: `Less` when `a` ranks before `b`.
fn compare_rows(
    a: &SectionRow,
    b: &SectionRow,
    options: &PlanOptions,
    profile: &StrategyProfile,
) -> Ordering {
    if a.finalist != b.finalist {
        return if a.finalist {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    if options.objective == Objective::Clear
        && let (Some(a_operation), Some(b_operation)) = (&a.operation, &b.operation)
    {
        if a_operation.clear_probability_low > b_operation.clear_probability_high {
            return Ordering::Less;
        }
        if b_operation.clear_probability_low > a_operation.clear_probability_high {
            return Ordering::Greater;
        }
        let a_break = simulation_tie_break(a, options.objective, options.exp_cell_type);
        let b_break = simulation_tie_break(b, options.objective, options.exp_cell_type);
        if let Some(order) = difference(a_break, b_break) {
            return order;
        }
    }
    if let Some(order) = difference(strategy_score(a, profile), strategy_score(b, profile)) {
        return order;
    }
    if let Some(order) = difference(
        simulation_tie_break(a, options.objective, options.exp_cell_type),
        simulation_tie_break(b, options.objective, options.exp_cell_type),
    ) {
        return order;
    }
    if let Some(order) = difference(a.proxy_gain, b.proxy_gain) {
        return order;
    }
    plot_key(&a.plot_ids).cmp(&plot_key(&b.plot_ids))
}

/// The staged comparison (`simulateSectionCandidates`). Returns the final
/// baseline aggregate, or `None` when the run was stopped.
fn simulate_candidates(
    rows: &mut [SectionRow],
    baseline_layout: &Layout,
    jelly: &Jelly,
    options: &PlanOptions,
    profile: &StrategyProfile,
    quality: &QualityProfile,
    reporter: &mut Reporter<'_>,
) -> Option<Option<Estimate>> {
    let seed = options.seed;
    if rows.is_empty() {
        let baseline = trial_set(
            jelly,
            baseline_layout,
            quality.final_trials,
            seed,
            options.objective,
            options.exp_cell_type,
        );
        return Some(Some(baseline.estimate));
    }
    for row in rows.iter_mut() {
        row.finalist = false;
    }
    let mut active: Vec<usize> = (0..rows.len()).collect();
    let mut final_baseline = None;
    let last_stage = quality.stages.len() - 1;
    for (stage_index, trials) in quality.stages.iter().copied().enumerate() {
        let baseline = trial_set(
            jelly,
            baseline_layout,
            trials,
            seed,
            options.objective,
            options.exp_cell_type,
        );
        for (position, index) in active.iter().copied().enumerate() {
            let row = &mut rows[index];
            let simulation = trial_set(
                &row.expanded,
                &row.layout,
                trials,
                seed,
                options.objective,
                options.exp_cell_type,
            );
            let confidence = paired_confidence(&simulation.values, &baseline.values);
            row.objective_value = operation_value(
                &simulation.estimate,
                options.objective,
                options.exp_cell_type,
            );
            row.objective_gain = marginal_score(
                &baseline.estimate,
                &simulation.estimate,
                options.objective,
                options.exp_cell_type,
            );
            row.gain_per_slot = if row.new_usable_slots > 0 {
                row.objective_gain / row.new_usable_slots as f64
            } else {
                0.0
            };
            row.chance_beats_baseline = confidence
                .as_ref()
                .map(|confidence| confidence.chance_beats_baseline);
            row.confidence = confidence;
            row.simulation_trials = Some(trials);
            row.finalist = stage_index == last_stage;
            row.operation = Some(simulation.estimate);
            if !reporter.step(
                format!(
                    "Simulating section candidates ({trials} trials), {} of {}",
                    position + 1,
                    active.len()
                ),
                0.0,
            ) {
                return None;
            }
        }
        final_baseline = Some(baseline.estimate);
        active.sort_by(|a, b| compare_rows(&rows[*a], &rows[*b], options, profile));
        if stage_index < last_stage {
            let next_limit = if stage_index == 0 {
                active
                    .len()
                    .div_ceil(3)
                    .max(4)
                    .min(profile.candidate_cap)
                    .min(active.len())
            } else {
                4.min(active.len())
            };
            active.truncate(next_limit);
        }
        if !reporter.step("Simulating section candidates", 1.0) {
            return None;
        }
    }
    Some(final_baseline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strategy_names_the_objective_it_plays_for() {
        assert_eq!(Strategy::Progression.objective(), Some(Objective::Clear));
        assert_eq!(
            Strategy::Bloodcells.objective(),
            Some(Objective::Bloodcells)
        );
        assert_eq!(Strategy::LongTerm.objective(), Some(Objective::Bloodcells));
        assert_eq!(Strategy::Exp.objective(), Some(Objective::Exp));
        assert_eq!(Strategy::Critical.objective(), Some(Objective::Survival));
        assert_eq!(Strategy::LowRisk.objective(), None);
        assert_eq!(Strategy::Conservative.objective(), None);
        for strategy in Strategy::PAGE {
            assert_eq!(Strategy::from_id(strategy.id()), Some(strategy));
        }
    }

    #[test]
    fn the_strategy_preset_sets_the_beam_and_the_request_clamps_it() {
        assert_eq!(strategy_options(Strategy::Progression, 0).beam_width, 8);
        assert_eq!(strategy_options(Strategy::Progression, 0).candidate_cap, 12);
        assert_eq!(strategy_options(Strategy::LowRisk, 0).beam_width, 6);
        assert_eq!(strategy_options(Strategy::LowRisk, 0).confidence_weight, -1);
        assert_eq!(strategy_options(Strategy::Progression, 2).beam_width, 2);
        assert_eq!(strategy_options(Strategy::Progression, 24).beam_width, 20);
    }

    #[test]
    fn every_quality_has_three_increasing_stages() {
        for quality in [Quality::Fast, Quality::Balanced, Quality::Thorough] {
            let profile = quality_options(quality);
            assert_eq!(profile.stages.len(), 3);
            assert!(profile.stages[0] < profile.stages[1]);
            assert!(profile.stages[1] < profile.stages[2]);
            assert_eq!(Quality::from_id(quality.id()), Some(quality));
        }
    }

    #[test]
    fn the_exp_target_is_clamped_to_an_owned_cell() {
        let jelly = Jelly {
            upgrades: vec![1; 8],
            ..Jelly::default()
        };
        assert_eq!(normalize_exp_cell_type(&jelly, None), None);
        assert_eq!(normalize_exp_cell_type(&jelly, Some(3)), Some(3));
        assert_eq!(normalize_exp_cell_type(&jelly, Some(40)), Some(7));
    }
}
