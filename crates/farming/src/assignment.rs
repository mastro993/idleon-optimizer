//! Plot assignment: which crop each of the 36 plots should carry.
//!
//! Ported from `bestCropForObjective` and `plotAssignmentPlan` in
//! `js/stats/systems/w6/farming-planner.js`. Each candidate is scored as a
//! hypothetical plot at evolution zero, so the assignment reads the objective's
//! own per-plot formula, while the evolution objective aims every plot at the
//! crop before its target.

use super::{Farm, PLOTS, Plot, Snapshot, data};

/// The objectives the Farming calculator plans for, in the upstream order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Objective {
    #[default]
    RankXp,
    FarmingExp,
    Evolution,
    Overgrowth,
    CropValue,
    Beans,
    Growth,
}

impl Objective {
    /// Every objective the calculator offers.
    pub const ALL: [Objective; 7] = [
        Objective::RankXp,
        Objective::FarmingExp,
        Objective::Evolution,
        Objective::Overgrowth,
        Objective::CropValue,
        Objective::Beans,
        Objective::Growth,
    ];

    /// The upstream id, used by the project document.
    pub fn id(self) -> &'static str {
        match self {
            Objective::RankXp => "rankXp",
            Objective::FarmingExp => "farmingExp",
            Objective::Evolution => "evolution",
            Objective::Overgrowth => "overgrowth",
            Objective::CropValue => "cropValue",
            Objective::Beans => "beans",
            Objective::Growth => "growth",
        }
    }

    /// The upstream tab label.
    pub fn label(self) -> &'static str {
        match self {
            Objective::RankXp => "Land Rank XP",
            Objective::FarmingExp => "Farming EXP",
            Objective::Evolution => "Crop Evolution",
            Objective::Overgrowth => "Overgrowth",
            Objective::CropValue => "Crop Value",
            Objective::Beans => "Magic Beans",
            Objective::Growth => "Growth Speed",
        }
    }

    /// The objective behind an upstream id.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|objective| objective.id() == id)
    }
}

/// `_cropWeight`: later families and deeper crops weigh more for Magic Beans.
pub(crate) fn crop_weight(crop_id: i64) -> f64 {
    let Some(family) = data().family_of(crop_id) else {
        return 0.0;
    };
    2.5_f64.powi(family.id as i32) * 1.08_f64.powf((crop_id - family.start) as f64)
}

/// A candidate crop, and the plot it would be planted as.
fn hypothetical_plot(crop_id: i64) -> Option<Plot> {
    let family = data().family_of(crop_id)?;
    Some(Plot {
        seed_family: family.id as i64,
        evolution: (crop_id - family.start) as u32,
        // Upstream writes `evolutionBlocked = 1`; only the evolution planner
        // reads it, and it must not count these candidate plots.
        evolution_blocked: true,
        crops_on_vine: 1,
        ..Plot::default()
    })
}

/// One plot's current crop and the crop the objective wants there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropRow {
    pub plot: usize,
    pub current_crop: i64,
    pub current_family: i64,
    pub target_crop: i64,
    pub target_family: i64,
    pub target_evolution: u32,
}

impl CropRow {
    /// Whether the target differs from what the plot carries now.
    pub fn change_needed(&self) -> bool {
        self.current_crop != self.target_crop
    }
}

/// The whole field's assignment for one objective.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub objective: Objective,
    /// The crop the evolution objective evolves into; `None` for the others.
    pub target_crop: Option<u32>,
    pub rows: Vec<CropRow>,
}

impl Assignment {
    /// How many plots differ from their target.
    pub fn changes(&self) -> usize {
        self.rows.iter().filter(|row| row.change_needed()).count()
    }
}

impl Default for Assignment {
    /// An empty field: every plot asks for nothing.
    fn default() -> Self {
        Self {
            objective: Objective::default(),
            target_crop: None,
            rows: (0..PLOTS)
                .map(|plot| CropRow {
                    plot,
                    current_crop: -1,
                    current_family: -1,
                    target_crop: -1,
                    target_family: -1,
                    target_evolution: 0,
                })
                .collect(),
        }
    }
}

impl Farm {
    /// The per-plot value the assignment ranks, in score units per second.
    fn plot_objective_value(
        &self,
        index: usize,
        plot: &Plot,
        objective: Objective,
        snapshot: &Snapshot,
    ) -> f64 {
        let cycle = self.growth_cycle_seconds(plot, snapshot);
        if !cycle.is_finite() || cycle <= 0.0 {
            return 0.0;
        }
        match objective {
            Objective::FarmingExp => self.farming_exp_per_harvest(plot, snapshot) / cycle,
            Objective::RankXp => self.rank_xp_for(index, plot, snapshot) / cycle,
            Objective::Overgrowth => self.next_overgrowth_chance(plot, snapshot) / cycle,
            // Growth, evolution and the account rows have no per-crop formula
            // upstream, so they rank by crop value; Magic Beans weigh it by how
            // much the crop is worth in beans.
            _ => {
                let value = self.crop_value(index, plot, snapshot).expected_harvest / cycle;
                if objective == Objective::Beans {
                    value * crop_weight(self.crop_type(plot))
                } else {
                    value
                }
            }
        }
    }

    /// `bestCropForObjective`: the discovered crop with the highest score.
    ///
    /// Ties keep the earlier crop id, as the upstream strict comparison does.
    pub(crate) fn best_crop(
        &self,
        index: usize,
        objective: Objective,
        snapshot: &Snapshot,
    ) -> Option<(i64, i64, u32)> {
        let mut best: Option<((i64, i64, u32), f64)> = None;
        for crop in self.crop_quantities.keys() {
            let Some(plot) = hypothetical_plot(i64::from(*crop)) else {
                continue;
            };
            let score = self.plot_objective_value(index, &plot, objective, snapshot);
            if best.is_none_or(|(_, current)| score > current) {
                best = Some(((i64::from(*crop), plot.seed_family, plot.evolution), score));
            }
        }
        best.map(|(candidate, _)| candidate)
    }

    /// `plotAssignmentPlan`: every plot's target crop for one objective.
    ///
    /// The evolution objective plants the crop before its target on every plot,
    /// because that is the crop whose growth rolls the target.
    pub fn assignment(&self, objective: Objective, target_crop: Option<u32>) -> Assignment {
        let snapshot = self.snapshot();
        let target = (objective == Objective::Evolution)
            .then_some(target_crop)
            .flatten()
            .filter(|crop| *crop > 0)
            .and_then(|crop| hypothetical_plot(i64::from(crop) - 1))
            .map(|plot| (self.crop_type(&plot), plot.seed_family, plot.evolution));
        let rows = (0..PLOTS)
            .map(|index| {
                let plot = self.plots.get(index).copied().unwrap_or_default();
                let (target_crop, target_family, target_evolution) = target
                    .or_else(|| self.best_crop(index, objective, &snapshot))
                    .unwrap_or((-1, -1, 0));
                CropRow {
                    plot: index,
                    current_crop: self.crop_type(&plot),
                    current_family: plot.seed_family,
                    target_crop,
                    target_family,
                    target_evolution,
                }
            })
            .collect();
        Assignment {
            objective,
            target_crop: target.map(|(crop, _, _)| crop as u32),
            rows,
        }
    }
}
