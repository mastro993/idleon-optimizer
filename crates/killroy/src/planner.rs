//! The selection, the upgrade list and the plan the trials run.
//!
//! Ports `killroyUpgradeChoices` and `buildKillroyPlan` from
//! `js/stats/systems/common/killroy-planner.js`. A plan turns the checked
//! encounters into one baseline row each plus one `+1` row per comparable
//! upgrade, so every option is compared against the same baseline with the same
//! seeds.

use super::buffs::{self, CharacterBuffs};
use super::forecast::{Encounter, encounter, rotation};
use super::inventory::{self, Bag};
use super::trial::UPGRADES;
use super::{MAX_FORECAST_WEEKS, WEEK_SECONDS, data, number, options};
use idleon_save::Save;
use idleon_save::at;
use serde::{Deserialize, Serialize};

/// The save's upgrade levels, the run requirement each one carries and whether
/// the save has unlocked it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpgradeChoice {
    pub id: usize,
    pub name: String,
    pub effect: String,
    pub level: Option<f64>,
    /// Killroy runs needed to unlock the level, matching the upstream table.
    pub runs_required: f64,
    /// `None` when the save does not carry the run count or the Dream level.
    pub unlocked: Option<bool>,
}

/// One upgrade a row compares: its saved level and the `+1` it tests.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Upgrade {
    pub id: usize,
    pub name: String,
    pub effect: String,
    pub level: f64,
    pub next: f64,
}

/// Why a plan could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KillroyError(String);

impl std::fmt::Display for KillroyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for KillroyError {}

impl KillroyError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// One encounter a plan runs: a character's slot in a week ahead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub week_offset: usize,
    pub ci: usize,
    pub slot: usize,
}

/// Which upgrades the plan compares against each baseline.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Comparisons {
    /// Baseline rows only.
    #[default]
    Off,
    /// Every unlocked upgrade the save carries.
    All,
    /// The chosen upgrade ids.
    Only(Vec<usize>),
}

/// What to plan: the checked encounters, the trials, the upgrade filter and the
/// two calculator options.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanInput {
    pub selections: Vec<Selection>,
    pub seeds: Vec<u64>,
    pub comparisons: Comparisons,
    /// The stand-in for the combat simulator's kill rate.
    pub kills_per_second: f64,
    /// Keep the save's `BuffsActive` timers instead of starting from a fresh map
    /// entry (`useSavedBuffs` upstream).
    pub keep_buffs: bool,
    /// Lift the bag-space limit on the run's pickups (`unlimitedInventory`).
    pub unlimited_inventory: bool,
}

/// One configuration the plan runs: a baseline or one `+1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The row's identity: the baseline key, with `:<upgrade id>` when comparing.
    pub key: String,
    /// The baseline row this one is compared against.
    pub baseline: String,
    pub upgrade: Option<Upgrade>,
    pub ci: usize,
    pub slot: usize,
    pub week_offset: usize,
    pub character: String,
    pub encounter: Encounter,
    /// The run limit the upstream plan computes: `103 + Time`, plus the `+1`.
    pub seconds: f64,
    /// The trial the row's seed runs with.
    pub trial: super::TrialInputs,
}

/// One week the plan covers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Week {
    pub week_offset: usize,
    pub week: i64,
    pub starts_at: f64,
    pub ends_at: f64,
}

/// One attempt: a row run with one seed.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub key: String,
    pub seed: u64,
}

/// The rows, the attempts and the weeks the section shows in its preview.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub seeds: Vec<u64>,
    pub rows: Vec<Row>,
    pub jobs: Vec<Job>,
    pub weeks: Vec<Week>,
    pub warnings: Vec<String>,
    /// Whether the run keeps the save's buff timers.
    pub keep_buffs: bool,
    /// Whether the run's pickups ignore the bag limit.
    pub unlimited_inventory: bool,
    /// The buffs each selected character's run starts with.
    pub characters: Vec<CharacterBuffs>,
}

impl Plan {
    /// Whether any row compares an upgrade.
    pub fn comparing(&self) -> bool {
        self.rows.iter().any(|row| row.upgrade.is_some())
    }

    /// The attempts a run covers.
    pub fn attempts(&self) -> usize {
        self.jobs.len()
    }
}

/// Every upgrade the save's Killroy screen lists, in the game's order.
pub fn upgrade_choices(save: &Save) -> Vec<UpgradeChoice> {
    let values = options(save);
    let completed = values
        .as_ref()
        .and_then(|values| at(values, 112))
        .and_then(number);
    let dream = save
        .get("Dream")
        .as_ref()
        .and_then(|dream| at(dream, 4))
        .and_then(number);
    data()
        .upgrades
        .iter()
        .enumerate()
        .map(|(id, upgrade)| {
            let runs_required = if id == 0 {
                0.0
            } else {
                (2.0 * id as f64).powf(id.min(2) as f64)
            };
            UpgradeChoice {
                id,
                name: upgrade.name.clone(),
                effect: upgrade.effect.clone(),
                level: values
                    .as_ref()
                    .and_then(|values| at(values, 106 + id))
                    .and_then(number),
                runs_required,
                unlocked: match (completed, dream) {
                    (Some(completed), Some(dream)) => {
                        Some(id < (3.0 + dream).round() as usize && completed >= runs_required)
                    }
                    _ => None,
                },
            }
        })
        .collect()
}

/// Build the baseline and `+1` rows for the checked encounters.
pub fn plan(save: &Save, input: &PlanInput) -> Result<Plan, KillroyError> {
    if input.selections.is_empty() {
        return Err(KillroyError::new(
            "Select at least one character and encounter",
        ));
    }
    if input.seeds.is_empty() || input.seeds.len() > 20 {
        return Err(KillroyError::new("Choose 1-20 trials"));
    }
    if input
        .seeds
        .iter()
        .enumerate()
        .any(|(index, seed)| input.seeds[..index].contains(seed))
    {
        return Err(KillroyError::new("Trial seeds must be distinct"));
    }
    let choices = upgrade_choices(save);
    let ids = match &input.comparisons {
        Comparisons::Off => None,
        Comparisons::All => Some(choices.iter().map(|choice| choice.id).collect()),
        Comparisons::Only(ids) => Some(ids.clone()),
    };
    if let Some(ids) = &ids
        && ids.iter().any(|id| *id >= UPGRADES)
    {
        return Err(KillroyError::new("Invalid Killroy upgrade choice"));
    }
    let compare = ids.as_ref().is_some_and(|ids| !ids.is_empty());
    let mut rows: Vec<Row> = Vec::new();
    let mut seen: Vec<Selection> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut cis: Vec<usize> = input
        .selections
        .iter()
        .map(|selection| selection.ci)
        .collect();
    cis.sort_unstable();
    cis.dedup();
    for ci in &cis {
        if !input.unlimited_inventory && !inventory::bag(save, *ci).room_for(inventory::SKULLS) {
            warnings.push(format!(
                "{}: the bag has no room for a skull stack, so collected skulls stay at zero. \
                 Free a slot or turn on unlimited inventory.",
                save.character_name(*ci)
            ));
        }
    }

    for selection in &input.selections {
        if selection.week_offset > MAX_FORECAST_WEEKS {
            return Err(KillroyError::new("Choose a week from 0 to 52 weeks ahead"));
        }
        if seen.contains(selection) {
            return Err(KillroyError::new("Duplicate character/encounter selection"));
        }
        seen.push(*selection);
        let found = encounter(save, selection.ci, selection.slot, selection.week_offset)?;
        if !found.eligible {
            return Err(KillroyError::new(if found.reason.is_empty() {
                "Invalid encounter"
            } else {
                found.reason.as_str()
            }));
        }
        let key = if selection.week_offset == 0 {
            format!("{}:{}", selection.ci, selection.slot)
        } else {
            format!(
                "w{}:{}:{}",
                selection.week_offset, selection.ci, selection.slot
            )
        };
        let character = save.character_name(selection.ci);
        let time = killroy_levels(save).levels[0];
        let bag = if input.unlimited_inventory {
            Bag::unlimited()
        } else {
            inventory::bag(save, selection.ci)
        };
        let base = super::TrialInputs {
            kills_per_second: input.kills_per_second.max(0.0),
            bag,
            ..killroy_levels(save)
        };
        rows.push(Row {
            key: key.clone(),
            baseline: key.clone(),
            upgrade: None,
            ci: selection.ci,
            slot: selection.slot,
            week_offset: selection.week_offset,
            character: character.clone(),
            encounter: found.clone(),
            seconds: (103.0 + time).ceil(),
            trial: base,
        });
        if !compare {
            continue;
        }
        for choice in choices.iter().filter(|choice| {
            ids.as_ref()
                .is_none_or(|ids| !ids.is_empty() && ids.contains(&choice.id))
                && choice.unlocked == Some(true)
                && choice.level.is_some()
        }) {
            let level = choice.level.expect("the filter keeps known levels");
            let mut trial = base;
            trial.levels[choice.id] = level + 1.0;
            rows.push(Row {
                key: format!("{key}:{}", choice.id),
                baseline: key.clone(),
                upgrade: Some(Upgrade {
                    id: choice.id,
                    name: choice.name.clone(),
                    effect: choice.effect.clone(),
                    level,
                    next: level + 1.0,
                }),
                ci: selection.ci,
                slot: selection.slot,
                week_offset: selection.week_offset,
                character: character.clone(),
                encounter: found.clone(),
                seconds: (103.0 + time + if choice.id == 0 { 1.0 } else { 0.0 }).ceil(),
                trial,
            });
        }
        if choices
            .iter()
            .any(|choice| choice.unlocked.is_none() || choice.level.is_none())
        {
            warnings.push("Some selected upgrade choices are unavailable in this save.".into());
        }
        for id in ids.iter().flatten() {
            let choice = &choices[*id];
            if choice.unlocked != Some(true) || choice.level.is_none() {
                warnings.push(format!(
                    "{character}: +1 {} is locked or unavailable in the selected loadout.",
                    choice.name
                ));
            }
        }
    }
    if compare && !rows.iter().any(|row| row.upgrade.is_some()) {
        return Err(KillroyError::new(
            "No selected upgrades are available for these characters",
        ));
    }
    if rows.iter().any(|row| row.seconds > 86_400.0) {
        return Err(KillroyError::new(
            "Killroy duration exceeds the 24-hour simulation limit",
        ));
    }
    let jobs = input
        .seeds
        .iter()
        .flat_map(|seed| {
            rows.iter().map(|row| Job {
                key: row.key.clone(),
                seed: *seed,
            })
        })
        .collect();
    let rotation = rotation(save)?;
    let mut offsets: Vec<usize> = rows.iter().map(|row| row.week_offset).collect();
    offsets.sort_unstable();
    offsets.dedup();
    let weeks: Vec<Week> = offsets
        .into_iter()
        .map(|week_offset| Week {
            week_offset,
            week: rotation.week + week_offset as i64,
            starts_at: rotation.reset_at + (week_offset as f64 - 1.0) * WEEK_SECONDS,
            ends_at: rotation.reset_at + week_offset as f64 * WEEK_SECONDS,
        })
        .collect();
    if weeks.iter().any(|week| week.week_offset > 0) {
        warnings.push(
            "Forecast assumes unchanged server rotation, unlocks, loadouts, upgrades and records. \
             Each attempt starts independently from the captured save; no weekly progression is \
             applied."
                .into(),
        );
    }
    let mut unique: Vec<String> = Vec::new();
    for warning in warnings {
        if !unique.contains(&warning) {
            unique.push(warning);
        }
    }
    let warnings = unique;

    Ok(Plan {
        seeds: input.seeds.clone(),
        rows,
        jobs,
        weeks,
        warnings,
        keep_buffs: input.keep_buffs,
        unlimited_inventory: input.unlimited_inventory,
        characters: buffs::character_buffs(save, &cis, input.keep_buffs),
    })
}

/// The trial levels a save carries: the six upgrade levels, the talent-drop
/// counter and the meritoc skull bonus.
fn killroy_levels(save: &Save) -> super::TrialInputs {
    let values = options(save);
    let level = |id: usize| {
        values
            .as_ref()
            .and_then(|values| at(values, 106 + id))
            .and_then(number)
            .unwrap_or(0.0)
    };
    let mut levels = [0.0; UPGRADES];
    for (id, slot) in levels.iter_mut().enumerate() {
        *slot = level(id);
    }
    super::TrialInputs {
        levels,
        talent_drops: values
            .as_ref()
            .and_then(|values| at(values, 114))
            .and_then(number)
            .unwrap_or(0.0),
        // `computeMeritocBonusz(18)` needs the voter, companion, legend, arcade,
        // event-shop and sushi trees, which are later sections; a save outside
        // them runs the base skull factor.
        meritoc: 0.0,
        kills_per_second: 0.0,
        bag: Bag::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture as save;
    use idleon_save::Save;
    use serde_json::json;

    fn input(selections: Vec<Selection>, comparisons: Comparisons) -> PlanInput {
        PlanInput {
            selections,
            seeds: vec![1, 2, 3],
            comparisons,
            kills_per_second: 3.0,
            keep_buffs: false,
            unlimited_inventory: false,
        }
    }

    #[test]
    fn every_upgrade_the_save_lists_is_comparable() {
        let save = save();
        let choices = upgrade_choices(&save);
        assert_eq!(choices.len(), UPGRADES);
        assert!(choices.iter().all(|choice| choice.unlocked == Some(true)));
        let plan = plan(
            &save,
            &input(
                vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                Comparisons::All,
            ),
        )
        .expect("the plan builds");
        let comparisons: Vec<usize> = plan
            .rows
            .iter()
            .filter_map(|row| row.upgrade.as_ref().map(|upgrade| upgrade.id))
            .collect();
        assert_eq!(comparisons, (0..UPGRADES).collect::<Vec<_>>());
        assert!(plan.comparing());
        assert_eq!(plan.attempts(), (UPGRADES + 1) * 3);
    }

    #[test]
    fn each_upgrade_row_patches_its_own_level() {
        let save = save();
        let plan = plan(
            &save,
            &input(
                vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 1,
                }],
                Comparisons::Only(vec![0, 2]),
            ),
        )
        .expect("the plan builds");
        let baseline = plan
            .rows
            .iter()
            .find(|row| row.upgrade.is_none())
            .expect("the plan has a baseline");
        for row in plan.rows.iter().filter(|row| row.upgrade.is_some()) {
            let upgrade = row.upgrade.as_ref().expect("an upgrade row");
            assert_eq!(row.trial.levels[upgrade.id], upgrade.next);
            if upgrade.id == 0 {
                assert_eq!(row.seconds, (103.0 + upgrade.next).ceil());
            }
            // The other levels stay at the baseline.
            for other in (0..UPGRADES).filter(|other| *other != upgrade.id) {
                assert_eq!(row.trial.levels[other], baseline.trial.levels[other]);
            }
        }
    }

    #[test]
    fn a_save_without_run_counts_compares_nothing_and_says_so() {
        let save = Save::new(json!({
                "Research": "[]",
                "Lv0_0": "[]",
                "CharacterClass_0": 10,
                "TimeAway": "{\"GlobalTime\":1794296810.952,\"ShopRestock\":62019.836}",
                "OptLacc": { "39": 5, "227": 1 },
                "KLA_0": { "100": [0], "150": [0], "200": [0], "250": [0] },
                "Summon": [[0, 0, 5]],
                "serverVars": { "KillroySwap": 7 },
        }));
        let choices = upgrade_choices(&save);
        assert!(choices.iter().all(|choice| choice.unlocked.is_none()));
        let error = plan(
            &save,
            &input(
                vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 0,
                }],
                Comparisons::All,
            ),
        )
        .expect_err("nothing is comparable");
        assert_eq!(
            error.to_string(),
            "No selected upgrades are available for these characters"
        );
    }

    #[test]
    fn duplicate_selections_are_refused() {
        let save = save();
        let selection = Selection {
            week_offset: 0,
            ci: 0,
            slot: 1,
        };
        let error = plan(&save, &input(vec![selection, selection], Comparisons::Off))
            .expect_err("duplicates are refused");
        assert_eq!(error.to_string(), "Duplicate character/encounter selection");
    }

    #[test]
    fn an_ineligible_encounter_names_its_reason() {
        let save = save();
        let error = plan(
            &save,
            &input(
                vec![Selection {
                    week_offset: 0,
                    ci: 0,
                    slot: 0,
                }],
                Comparisons::Off,
            ),
        )
        .expect_err("a Warrior encounter needs a Warrior");
        assert_eq!(error.to_string(), "Requires Warrior");
    }

    #[test]
    fn the_trial_range_validates() {
        let save = save();
        let selection = Selection {
            week_offset: 0,
            ci: 0,
            slot: 1,
        };
        let mut bad = input(vec![selection], Comparisons::Off);
        bad.seeds = Vec::new();
        assert_eq!(
            plan(&save, &bad).expect_err("no trials").to_string(),
            "Choose 1-20 trials"
        );
        bad.seeds = (1..=21).collect();
        assert_eq!(
            plan(&save, &bad).expect_err("too many trials").to_string(),
            "Choose 1-20 trials"
        );
        bad.seeds = vec![4, 4];
        assert_eq!(
            plan(&save, &bad).expect_err("duplicate seeds").to_string(),
            "Trial seeds must be distinct"
        );
    }

    #[test]
    fn the_plan_keeps_the_buff_option_and_lists_the_saved_rows() {
        let save = save();
        let selection = Selection {
            week_offset: 0,
            ci: 0,
            slot: 1,
        };
        let mut input = input(vec![selection], Comparisons::Off);
        let fresh = plan(&save, &input).expect("the plan builds");
        assert!(!fresh.keep_buffs);
        assert!(!fresh.unlimited_inventory);
        assert_eq!(fresh.characters.len(), 1);
        assert_eq!(fresh.characters[0].ci, 0);
        assert!(!fresh.characters[0].buffs.is_empty());
        assert!(
            fresh.characters[0]
                .buffs
                .iter()
                .all(|buff| buff.kept == buff.form),
            "a fresh run keeps only the form toggles"
        );

        input.keep_buffs = true;
        input.unlimited_inventory = true;
        let kept = plan(&save, &input).expect("the plan builds");
        assert!(kept.keep_buffs);
        assert!(kept.unlimited_inventory);
        assert!(kept.characters[0].buffs.iter().all(|buff| buff.kept));
        assert!(kept.rows.iter().all(|row| row.trial.bag.room_for("Pearl4")));
    }

    #[test]
    fn a_bag_without_room_for_skulls_warns_and_unlimited_lifts_it() {
        let mut wrapped = crate::test_support::wrapped_fixture();
        wrapped["data"]["InventoryOrder_0"] = json!(["Pearl4"]);
        let save = Save::new(wrapped);
        let selection = Selection {
            week_offset: 0,
            ci: 0,
            slot: 1,
        };
        let mut input = input(vec![selection], Comparisons::Off);
        let full = plan(&save, &input).expect("the plan builds");
        assert!(
            full.warnings
                .iter()
                .any(|warning| warning.contains("no room for a skull stack")),
            "{:?}",
            full.warnings
        );
        assert!(!full.rows[0].trial.bag.room_for("Quest69"));

        input.unlimited_inventory = true;
        let lifted = plan(&save, &input).expect("the plan builds");
        assert!(
            lifted
                .warnings
                .iter()
                .all(|warning| !warning.contains("no room for a skull stack")),
            "{:?}",
            lifted.warnings
        );
        assert!(lifted.rows[0].trial.bag.room_for("Quest69"));
    }

    #[test]
    fn the_upgrade_table_matches_upstream() {
        let save = save();
        let choices = upgrade_choices(&save);
        let runs: Vec<f64> = choices.iter().map(|choice| choice.runs_required).collect();
        assert_eq!(runs, [0.0, 2.0, 16.0, 36.0, 64.0, 100.0]);
        let levels: Vec<Option<f64>> = choices.iter().map(|choice| choice.level).collect();
        assert_eq!(
            levels,
            [
                Some(155.0),
                Some(11.0),
                Some(281.0),
                Some(28.0),
                Some(1.0),
                Some(2.0)
            ]
        );
    }
}
