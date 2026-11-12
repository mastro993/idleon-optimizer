//! The Trench depth projection: the saved depth's fish requirement, the
//! efficiency requirements each projected depth raises, the hours every depth
//! takes with the selected and the fastest lure + line, and the other
//! characters fishing alongside.
//!
//! Ported from the upstream `trenchPlan` in `js/stats/systems/w2/fishing.js` and
//! the cavern helpers it calls in `js/stats/systems/w5/motherlode.js`
//! (`motherlodeEffBase`, `trenchFishingEffReqs`, `cavernLayerRequirement`). The
//! save rows are the upstream `holesData[11]` (progress and cleared layers),
//! `holesData[24][5]` (the jar collectible) and `holesData[26]` (Bolaia), plus
//! `spelunkData[18][29]` (Legend 29).
//!
//! The upstream recomputes each toolkit's efficiency and EXP multiplier through
//! the skill systems; this port holds them in [`AccountInputs`] and therefore
//! searches the fastest lure + line with the account values fixed, exactly as
//! the FI-02 ranking does.

use super::data::{Pool, data};
use super::save::{Character, FishingSave};
use super::{AccountInputs, PoolOutcome, TRENCH_POOL, resolve};
use crate::save::{Save, at, number};

/// The `holesData` rows the Trench requirement reads.
mod row {
    /// `Holes[11]`: the per-cavern cleared layers and collected amounts.
    pub const STATE: usize = 11;
    /// `Holes[24]`: the jar collectibles.
    pub const COLLECTIBLES: usize = 24;
    /// `Holes[26]`: the Bolaia study levels.
    pub const BOLAIA: usize = 26;
}

/// The jar collectible whose bonus reduces the skilling cavern requirements.
const COLLECTIBLE: usize = 5;

/// The Legend talent that raises every jar collectible.
const LEGEND: usize = 29;

/// The Trench's index in `SKILLING_CAVERNS`.
const TRENCH: usize = 3;

/// The Trench location's depth bonus: the pool is fished outside the maps.
const TRENCH_SPOT_BONUS: [f64; 4] = [0.0; 4];

/// One Bolaia study: its cavern, its per-level bonus and its saved level.
#[derive(Clone, Debug, PartialEq)]
pub struct BolaiaStudy {
    pub cavern: String,
    pub study: usize,
    pub per_level: f64,
    pub level: f64,
}

/// A Bolaia term at one projected depth.
#[derive(Clone, Debug, PartialEq)]
pub struct BolaiaLayer {
    pub cavern: String,
    pub study: usize,
    pub per_layer: f64,
    pub layers: f64,
    pub value: f64,
}

/// The Trench's saved depth and the reductions every projected depth shares.
#[derive(Clone, Debug, PartialEq)]
pub struct Trench {
    /// `Holes[11][1 + 2t]` for each of the four skilling caverns.
    pub cavern_layers: [f64; 4],
    /// `Holes[11][6]`: the Trench's collected amount at the current depth.
    pub progress: f64,
    /// `legendPTSbonus(29)`.
    pub legend: f64,
    /// `collectibleBonus(save, 5, legend)`.
    pub jar: f64,
    /// `Holes[26]` levels for the four Bolaia layer studies.
    pub bolaia: Vec<BolaiaStudy>,
}

impl Trench {
    /// Read the Trench out of a decoded save, or `None` when it has no `Holes`
    /// rows — the upstream `base.trench`.
    pub fn from_save(save: &Save) -> Option<Self> {
        let holes = save.get("Holes")?;
        let holes = holes.as_array()?;
        if holes.is_empty() {
            return None;
        }
        let cell = |row: usize, column: usize| {
            holes
                .get(row)
                .and_then(|row| at(row, column))
                .and_then(number)
                .unwrap_or(0.0)
        };
        let data = &data().trench;
        let legend = (spelunk(save, 18, LEGEND) * data.legend_per_point).round();
        let bolaia = data
            .studies
            .iter()
            .enumerate()
            .map(|(cavern, study)| BolaiaStudy {
                cavern: data.caverns.get(cavern).cloned().unwrap_or_default(),
                study: *study,
                per_level: data.bolaia_per_level.get(cavern).copied().unwrap_or(0.0),
                level: cell(row::BOLAIA, *study),
            })
            .collect();
        Some(Self {
            cavern_layers: [
                cell(row::STATE, 1),
                cell(row::STATE, 3),
                cell(row::STATE, 5),
                cell(row::STATE, 7),
            ],
            progress: cell(row::STATE, 6),
            legend,
            jar: cell(row::COLLECTIBLES, COLLECTIBLE)
                * data.collectible5_per_level
                * (1.0 + legend / 100.0),
            bolaia,
        })
    }

    /// `cavernLayersCleared(save, 3)`: the Trench depth already cleared.
    pub fn layers(&self) -> f64 {
        self.cavern_layers[TRENCH]
    }

    /// `trenchFishingEffReqs(save, layers)`: the three efficiency requirements
    /// `fishingPoolOutcome` applies to the Trench pool in place of FishPools'.
    pub fn eff_reqs(&self, layers: f64) -> [f64; 3] {
        // `motherlodeEffBase(save, 3, layers) / 4`, with `1 + 99 × ⌊3 / 3⌋`
        // folded in: 2e4 × 100 / 4 = 5e5.
        let quarter = 5e5 * 1.8_f64.powf(1.0 + layers);
        [quarter, quarter + 100.0, quarter + 200.0]
    }

    /// `cavernLayerRequirement(save, 3, layers)`. `layers` projects the Trench
    /// at a different depth, which zeroes the saved progress, as the upstream
    /// override does.
    pub fn requirement(&self, layers: Option<f64>) -> TrenchRequirement {
        let projected = layers.is_some();
        let layers = layers.unwrap_or_else(|| self.layers());
        // `(1 + 99 × ⌊3 / 3⌋) × 200 × 2.2^(1 + layers)`.
        let base = 2e4 * 2.2_f64.powf(1.0 + layers);
        let bolaia: Vec<BolaiaLayer> = self
            .bolaia
            .iter()
            .enumerate()
            .map(|(cavern, study)| {
                let layers = if cavern == TRENCH {
                    layers
                } else {
                    self.cavern_layers[cavern]
                };
                let per_layer = study.level * study.per_level;
                BolaiaLayer {
                    cavern: study.cavern.clone(),
                    study: study.study,
                    per_layer,
                    layers,
                    value: per_layer * layers,
                }
            })
            .collect();
        let reduction_pct = self.jar + bolaia.iter().map(|layer| layer.value).sum::<f64>();
        let value = base / (1.0 + reduction_pct / 100.0);
        let progress = if projected { 0.0 } else { self.progress };
        TrenchRequirement {
            cavern: data().trench.caverns[TRENCH].clone(),
            layers,
            base,
            jar: self.jar,
            legend: self.legend,
            bolaia,
            reduction_pct,
            value,
            progress,
            remaining: (value - progress).max(0.0),
        }
    }
}

/// `cavernLayerRequirement(save, 3, layers)`: one depth's fish requirement.
#[derive(Clone, Debug, PartialEq)]
pub struct TrenchRequirement {
    pub cavern: String,
    pub layers: f64,
    /// `20000 × 2.2^(1 + layers)`.
    pub base: f64,
    /// The jar collectible's reduction, already raised by the legend points.
    pub jar: f64,
    pub legend: f64,
    pub bolaia: Vec<BolaiaLayer>,
    pub reduction_pct: f64,
    /// The requirement after every reduction.
    pub value: f64,
    pub progress: f64,
    pub remaining: f64,
}

/// How a helper character's lure and line are chosen, the upstream
/// `state.trenchHelperMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HelperMode {
    /// Their saved lure + line.
    #[default]
    Saved,
    /// Their fish-maximizing lure + line at each projected depth.
    Best,
}

impl HelperMode {
    /// The control label the upstream select shows.
    pub fn label(self) -> &'static str {
        match self {
            HelperMode::Saved => "Their saved lure + line",
            HelperMode::Best => "Their fastest lure + line",
        }
    }
}

/// One lure and line's place on a projected depth.
#[derive(Clone, Debug)]
pub struct TrenchSelection {
    pub lure: usize,
    pub line: usize,
    pub outcome: PoolOutcome,
    /// Hours to clear this depth's requirement with this toolkit alone.
    pub hours: f64,
    /// Hours from the current depth through the end of this row.
    pub cumulative_hours: f64,
}

/// One projected depth.
#[derive(Clone, Debug)]
pub struct TrenchRow {
    /// The depth this row reaches, one past its cleared layers.
    pub depth: f64,
    pub layers: f64,
    pub eff_reqs: [f64; 3],
    pub requirement: TrenchRequirement,
    pub selected: TrenchSelection,
    pub best: TrenchSelection,
}

/// One other character fishing the Trench.
#[derive(Clone, Debug)]
pub struct TrenchHelper {
    pub character: usize,
    pub name: String,
    pub level: u32,
    /// `AFKtarget_<n>`, the character's saved target.
    pub afk_target: String,
    /// The lure and line the table shows.
    pub lure: usize,
    pub line: usize,
    /// That toolkit's claim at the current depth.
    pub outcome: PoolOutcome,
    /// That toolkit's hours to the next depth alone.
    pub hours: f64,
    /// Fish per hour at every projected depth, for the team totals.
    pub rates: Vec<f64>,
    /// The character's saved target is the Trench, so they start ticked.
    pub at_trench: bool,
    /// Whether the table's checkbox is ticked.
    pub enabled: bool,
}

/// The team's fish per hour and hours at one projected depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrenchTeamRow {
    pub rate: f64,
    pub hours: f64,
    pub cumulative_hours: f64,
}

/// The selected character and the ticked helpers together.
#[derive(Clone, Debug, PartialEq)]
pub struct TrenchTeam {
    /// The current depth's fish per hour.
    pub rate: f64,
    /// The current depth's hours to the next one.
    pub hours: f64,
    /// The ticked helpers' share of [`Self::rate`].
    pub helpers_rate: f64,
    /// One entry per projected depth.
    pub rows: Vec<TrenchTeamRow>,
}

/// The whole projection: the saved depth, its rows and the helpers.
#[derive(Clone, Debug)]
pub struct TrenchPlan {
    pub trench: Trench,
    pub rows: Vec<TrenchRow>,
    pub helpers: Vec<TrenchHelper>,
    pub team: TrenchTeam,
}

/// What one projection runs with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrenchRequest<'a> {
    pub character: usize,
    pub lure: usize,
    pub line: usize,
    /// How many depths to project, clamped to the upstream `1..=25`.
    pub depths: usize,
    pub mode: HelperMode,
    /// One tick per character; the selected character's entry is ignored.
    pub helpers: &'a [bool],
}

/// Project the Trench's next depths, its fastest lure + line and the other
/// characters' contributions. `None` when the save carries no `Holes` rows.
pub fn trench_plan(
    save: &FishingSave,
    account: &AccountInputs,
    request: &TrenchRequest,
) -> Option<TrenchPlan> {
    let trench = save.trench.clone()?;
    let pool = data().pool(TRENCH_POOL).expect("the Trench pool ships");
    let character = save
        .character(request.character)
        .cloned()
        .unwrap_or_else(|| Character {
            index: request.character,
            name: format!("Character {}", request.character + 1),
            ..Character::default()
        });
    let rows = plan_rows(
        &trench,
        account,
        &character,
        pool,
        Selection {
            lure: request.lure,
            line: request.line,
        },
        request.depths,
        true,
    );

    let mut helpers = Vec::new();
    for other in &save.characters {
        if other.index == request.character {
            continue;
        }
        let best = request.mode == HelperMode::Best;
        let plan = plan_rows(
            &trench,
            account,
            other,
            pool,
            Selection {
                lure: other.saved_lure,
                line: other.saved_line,
            },
            request.depths,
            best,
        );
        let Some(first) = plan.first() else {
            continue;
        };
        helpers.push(TrenchHelper {
            character: other.index,
            name: other.name.clone(),
            level: other.level,
            afk_target: other.afk_target.clone(),
            lure: first.best.lure,
            line: first.best.line,
            outcome: first.best.outcome.clone(),
            hours: first.best.hours,
            rates: plan.iter().map(|row| row.best.outcome.fish_total).collect(),
            at_trench: other.afk_target == TRENCH_POOL,
            enabled: request.helpers.get(other.index).copied().unwrap_or(false),
        });
    }

    let team = team(&rows, &helpers, request.character);
    Some(TrenchPlan {
        trench,
        rows,
        helpers,
        team,
    })
}

/// One lure/line choice, before it is resolved.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Selection {
    lure: usize,
    line: usize,
}

/// A combination with its resolved claim.
#[derive(Clone, Debug)]
struct Claim {
    lure: usize,
    line: usize,
    outcome: PoolOutcome,
}

/// The upstream `trenchPlan` row loop: one row per projected depth.
fn plan_rows(
    trench: &Trench,
    account: &AccountInputs,
    character: &Character,
    pool: &'static Pool,
    selection: Selection,
    depths: usize,
    search_best: bool,
) -> Vec<TrenchRow> {
    let count = depths.clamp(1, 25);
    let mut rows = Vec::with_capacity(count);
    let (mut cumulative_selected, mut cumulative_best) = (0.0, 0.0);
    for step in 0..count {
        let layers = trench.layers() + step as f64;
        let requirement = trench.requirement((step > 0).then_some(layers));
        let eff_reqs = trench.eff_reqs(layers);
        let selected = claim(account, character, pool, selection, eff_reqs);
        let best = if search_best {
            fastest(account, character, pool, eff_reqs, selection)
        } else {
            selected.clone()
        };
        let selected_hours = hours(&requirement, selected.outcome.fish_total);
        let best_hours = hours(&requirement, best.outcome.fish_total);
        cumulative_selected += selected_hours;
        cumulative_best += best_hours;
        rows.push(TrenchRow {
            depth: layers + 1.0,
            layers,
            eff_reqs,
            requirement,
            selected: TrenchSelection {
                lure: selected.lure,
                line: selected.line,
                outcome: selected.outcome,
                hours: selected_hours,
                cumulative_hours: cumulative_selected,
            },
            best: TrenchSelection {
                lure: best.lure,
                line: best.line,
                outcome: best.outcome,
                hours: best_hours,
                cumulative_hours: cumulative_best,
            },
        });
    }
    rows
}

/// The fish-maximizing lure + line at one projected depth, the upstream loop
/// inside `trenchPlan`. Ties keep the earlier combination and rank by EXP.
fn fastest(
    account: &AccountInputs,
    character: &Character,
    pool: &'static Pool,
    eff_reqs: [f64; 3],
    fallback: Selection,
) -> Claim {
    let data = data();
    let mut best = claim(account, character, pool, fallback, eff_reqs);
    for lure in &data.lures {
        for line in &data.lines {
            let candidate = claim(
                account,
                character,
                pool,
                Selection {
                    lure: lure.idx,
                    line: line.idx,
                },
                eff_reqs,
            );
            if candidate.outcome.fish_total > best.outcome.fish_total
                || (candidate.outcome.fish_total == best.outcome.fish_total
                    && candidate.outcome.exp > best.outcome.exp)
            {
                best = candidate;
            }
        }
    }
    best
}

/// Resolve one selection into the Trench claim with the depth's requirements.
fn claim(
    account: &AccountInputs,
    character: &Character,
    pool: &'static Pool,
    selection: Selection,
    eff_reqs: [f64; 3],
) -> Claim {
    let data = data();
    let lure = data
        .toolkit(0, selection.lure)
        .cloned()
        .expect("fishing lures");
    let line = data
        .toolkit(1, selection.line)
        .cloned()
        .expect("fishing lines");
    let resolved = resolve(
        account,
        character,
        &lure,
        &line,
        TRENCH_SPOT_BONUS,
        pool,
        Some(eff_reqs),
        1.0,
    );
    Claim {
        lure: lure.idx,
        line: line.idx,
        outcome: resolved.outcome,
    }
}

/// Hours to clear one row's requirement at a fish rate; endless at zero.
fn hours(requirement: &TrenchRequirement, fish_per_hour: f64) -> f64 {
    if fish_per_hour > 0.0 {
        requirement.remaining / fish_per_hour
    } else {
        f64::INFINITY
    }
}

/// The selected character plus the ticked helpers at every depth.
fn team(rows: &[TrenchRow], helpers: &[TrenchHelper], character: usize) -> TrenchTeam {
    let enabled: Vec<&TrenchHelper> = helpers
        .iter()
        .filter(|helper| helper.enabled && helper.character != character)
        .collect();
    let helpers_rate = enabled
        .iter()
        .map(|helper| helper.rates.first().copied().unwrap_or(0.0))
        .sum::<f64>();
    let mut cumulative = 0.0;
    let team_rows: Vec<TrenchTeamRow> = rows
        .iter()
        .enumerate()
        .map(|(depth, row)| {
            let rate = row.selected.outcome.fish_total
                + enabled
                    .iter()
                    .map(|helper| helper.rates.get(depth).copied().unwrap_or(0.0))
                    .sum::<f64>();
            let hours = hours(&row.requirement, rate);
            cumulative += hours;
            TrenchTeamRow {
                rate,
                hours,
                cumulative_hours: cumulative,
            }
        })
        .collect();
    TrenchTeam {
        rate: rows
            .first()
            .map_or(0.0, |row| row.selected.outcome.fish_total)
            + helpers_rate,
        hours: team_rows.first().map_or(f64::INFINITY, |row| row.hours),
        helpers_rate,
        rows: team_rows,
    }
}

/// `spelunkData[row][column]`.
fn spelunk(save: &Save, row: usize, column: usize) -> f64 {
    save.get("Spelunk")
        .and_then(|spelunk| spelunk.as_array().and_then(|rows| rows.get(row).cloned()))
        .as_ref()
        .and_then(|row| at(row, column))
        .and_then(number)
        .unwrap_or(0.0)
}
