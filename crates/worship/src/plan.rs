//! Worship TD build plans: the coverage heuristic, the community guide preset,
//! staged layouts and the checks the Plan pane lists.
//!
//! Ported from `heuristicPlan`, `guidePresetPlan`, `planPlacementLayouts` and
//! `validatePlanningPlacement` in `js/stats/systems/w3/worship-td.js`. A plan's
//! towers carry a slot, a weight, a trait string and an optional level cap; the
//! simulator places them in order and upgrades by weight per point.

use super::{
    Inputs, Placement, Spot, TOWER_COUNT, Trial, distance, placement_bands, tower_centers,
    validate_placement,
};
use serde::{Deserialize, Serialize};

/// How a plan presses the Next Wave button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkipPolicy {
    /// Wait out every countdown.
    #[default]
    Never,
    /// Skip once the field and the pending spawns are empty.
    WhenClear,
    /// Press Next Wave every second outside a boss hold, as the guide play does.
    Spam,
}

impl SkipPolicy {
    /// The stable id and the label the pane shows.
    pub fn id(self) -> &'static str {
        match self {
            SkipPolicy::Never => "never",
            SkipPolicy::WhenClear => "when-clear",
            SkipPolicy::Spam => "spam",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SkipPolicy::Never => "Never",
            SkipPolicy::WhenClear => "When clear",
            SkipPolicy::Spam => "Spam",
        }
    }
}

/// Whether a plan asks for the Archer root and when. The simulator does not
/// model roots this build; the pane names the gap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RootPolicy {
    #[default]
    Off,
    Cooldown,
    Danger,
}

impl RootPolicy {
    pub fn id(self) -> &'static str {
        match self {
            RootPolicy::Off => "off",
            RootPolicy::Cooldown => "cooldown",
            RootPolicy::Danger => "danger",
        }
    }
}

/// One planned tower: where it goes, what it is worth and how it grows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanTower {
    /// The tower type (`Tower[9 + type]` in the save).
    #[serde(rename = "type")]
    pub type_index: usize,
    pub slot: Spot,
    /// Upgrade priority relative to its cost; placement scores double.
    pub weight: f64,
    /// One character per trait earned, `A` or `B`.
    pub traits: String,
    /// The level cap, or `None` for unlimited upgrades.
    pub max_level: Option<u32>,
}

impl Default for PlanTower {
    fn default() -> Self {
        Self {
            type_index: 0,
            slot: Spot { node: 0, x: 0.0 },
            weight: 1.0,
            traits: String::new(),
            max_level: None,
        }
    }
}

impl PlanTower {
    fn new(
        type_index: usize,
        node: usize,
        x: f64,
        traits: &str,
        max_level: Option<u32>,
        weight: f64,
    ) -> Self {
        Self {
            type_index,
            slot: Spot { node, x },
            weight,
            traits: traits.into(),
            max_level,
        }
    }

    /// The cap as a `u32`, with `None` meaning "keep upgrading".
    pub(crate) fn level_cap(&self) -> u32 {
        self.max_level.unwrap_or(u32::MAX)
    }
}

/// A plan's sell trigger and target list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhaseSell {
    /// Sell every tower except `keep_types`.
    pub all: bool,
    pub types: Vec<usize>,
    pub keep_types: Vec<usize>,
    /// Global plan indices, across the initial list and every phase before it.
    pub plan_idx: Vec<usize>,
}

/// The conditions a phase waits for; every present trigger must hold.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PhaseWhen {
    pub min_wave: Option<u32>,
    pub boss_hold: Option<bool>,
    pub alive_max: Option<u32>,
    pub min_time_sec: Option<f64>,
    pub boss_hp_frac_max: Option<f64>,
}

/// One sell-and-rebuild stage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanPhase {
    pub label: String,
    pub when: PhaseWhen,
    pub sell: PhaseSell,
    pub towers: Vec<PlanTower>,
    /// Start the new queue at this phase's first tower once it fires.
    pub skip_remaining: bool,
}

impl Default for PlanPhase {
    fn default() -> Self {
        Self {
            label: String::new(),
            when: PhaseWhen::default(),
            sell: PhaseSell::default(),
            towers: Vec::new(),
            skip_remaining: true,
        }
    }
}

/// A complete build plan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Plan {
    pub towers: Vec<PlanTower>,
    pub phases: Vec<PlanPhase>,
    pub skip_policy: SkipPolicy,
    pub roots: RootPolicy,
}

impl Plan {
    /// Drop entries the simulator cannot place and clamp the values it reads.
    pub fn normalized(&self) -> Plan {
        let mut plan = self.clone();
        plan.towers.retain(|tower| tower.type_index < TOWER_COUNT);
        for phase in &mut plan.phases {
            phase.towers.retain(|tower| tower.type_index < TOWER_COUNT);
        }
        for tower in plan.towers.iter_mut().chain(
            plan.phases
                .iter_mut()
                .flat_map(|phase| phase.towers.iter_mut()),
        ) {
            tower.weight = tower.weight.max(0.01);
            tower.max_level = tower.max_level.map(|level| level.max(1));
        }
        plan
    }
}

/// One planned tower as a stage lays it out.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanPlacement {
    pub plan_index: usize,
    pub type_index: usize,
    pub node: usize,
    /// The x the game snaps to, which may differ from `requested_x`.
    pub x: f64,
    pub requested_x: f64,
    pub traits: String,
    pub max_level: Option<u32>,
}

/// A planned tower the stage could not place, with the game's reason.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanReject {
    pub plan_index: usize,
    pub type_index: usize,
    pub reason: String,
}

/// The towers standing at one stage: the initial build, then each phase.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanStage {
    pub label: String,
    pub towers: Vec<PlanPlacement>,
    pub rejected: Vec<PlanReject>,
}

/// The tower position and its scene distance to a sampled path point.
fn path_samples(trial: &Trial, spacing: f64) -> Vec<(f64, f64)> {
    let Some(geometry) = super::geometry(trial) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for node_index in &trial.path {
        let Some(node) = geometry.nodes.get(*node_index) else {
            continue;
        };
        let top = node.y - trial.mob_def.height + (trial.mob_def.height - 50.0) / 2.0;
        let mut x = node.x;
        while x <= node.length {
            out.push((x, top + trial.mob_def.frame_h / 2.0));
            x += spacing;
        }
    }
    out
}

/// How many path samples a tower's range covers.
pub(super) fn slot_coverage(trial: &Trial, spot: Spot, range: f64) -> usize {
    let centers = tower_centers(trial, spot.node, spot.x);
    path_samples(trial, 20.0)
        .into_iter()
        .filter(|(x, y)| distance(x - centers.wx, y - centers.wy) < range)
        .count()
}

/// Whether a click zone can reach a tower for the upgrade menu.
pub(crate) fn tower_upgrade_access(trial: &Trial, node: usize, x: f64) -> bool {
    let centers = tower_centers(trial, node, x);
    let (left, right) = (centers.cx - 35.0, centers.cx + 29.0);
    let (top, bottom) = (centers.cy - 105.0, centers.cy - 37.0);
    let mut xs = vec![left, right];
    let mut ys = vec![top, bottom];
    for zone in &trial.npcs {
        if zone.left > left && zone.left < right {
            xs.push(zone.left);
        }
        if zone.right > left && zone.right < right {
            xs.push(zone.right);
        }
        if zone.top > top && zone.top < bottom {
            ys.push(zone.top);
        }
        if zone.bottom > top && zone.bottom < bottom {
            ys.push(zone.bottom);
        }
    }
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    for i in 1..xs.len() {
        for j in 1..ys.len() {
            if xs[i] <= xs[i - 1] || ys[j] <= ys[j - 1] {
                continue;
            }
            let px = (xs[i] + xs[i - 1]) / 2.0;
            let py = (ys[j] + ys[j - 1]) / 2.0;
            if !trial
                .npcs
                .iter()
                .any(|zone| px > zone.left && px < zone.right && py > zone.top && py < zone.bottom)
            {
                return true;
            }
        }
    }
    false
}

/// The game's placement, plus the planning checks: camera-independent spawn and
/// exit slots, and a reachable upgrade click.
pub(super) fn validate_planning_placement(
    trial: &Trial,
    existing: &[Spot],
    node: usize,
    x: f64,
) -> Placement {
    let placement = validate_placement(trial, existing, node, x, 0.0);
    if !placement.ok {
        return placement;
    }
    let scene = &trial.scene;
    let camera = (scene.w - 960.0).max(0.0).min(placement.x - 480.0);
    let centred = validate_placement(trial, existing, node, x, camera);
    if !centred.ok {
        return Placement {
            ok: false,
            x: placement.x,
            reason: "Camera-sensitive placement",
        };
    }
    if !tower_upgrade_access(trial, node, placement.x) {
        return Placement {
            ok: false,
            x: placement.x,
            reason: "NPC blocks the tower upgrade click zone",
        };
    }
    placement
}

/// Every planning-legal slot, walking each band in `step` increments.
pub(super) fn legal_plan_slots(trial: &Trial, step: f64) -> Vec<Spot> {
    let mut out = Vec::new();
    for (node, spans) in placement_bands(trial).iter().enumerate() {
        for (a, b) in spans {
            let mut x = *a;
            while x <= *b {
                let check = validate_planning_placement(trial, &[], node, x);
                if check.ok && check.x == x {
                    out.push(Spot { node, x });
                }
                x += step;
            }
        }
    }
    out
}

/// The stage-by-stage layout, snapping each request against the towers standing
/// at that moment. Rejected requests are listed instead of drawn.
pub fn plan_stages(trial: &Trial, plan: &Plan) -> Vec<PlanStage> {
    /// A tower standing after a stage's placements.
    struct Standing {
        spot: Spot,
        type_index: usize,
        plan_index: usize,
    }

    let mut standing: Vec<Standing> = Vec::new();
    let mut index = 0;
    let mut stages = Vec::new();

    fn add(
        trial: &Trial,
        standing: &mut Vec<Standing>,
        index: &mut usize,
        tower: &PlanTower,
        out: &mut Vec<PlanPlacement>,
        rejected: &mut Vec<PlanReject>,
    ) {
        let plan_index = *index;
        *index += 1;
        if tower.type_index >= TOWER_COUNT {
            rejected.push(PlanReject {
                plan_index,
                type_index: tower.type_index,
                reason: "Unknown tower type".into(),
            });
            return;
        }
        let spots: Vec<Spot> = standing.iter().map(|tower| tower.spot).collect();
        let check = validate_placement(trial, &spots, tower.slot.node, tower.slot.x, 0.0);
        if !check.ok {
            rejected.push(PlanReject {
                plan_index,
                type_index: tower.type_index,
                reason: check.reason.into(),
            });
            return;
        }
        standing.push(Standing {
            spot: Spot {
                node: tower.slot.node,
                x: check.x,
            },
            type_index: tower.type_index,
            plan_index,
        });
        out.push(PlanPlacement {
            plan_index,
            type_index: tower.type_index,
            node: tower.slot.node,
            x: check.x,
            requested_x: tower.slot.x,
            traits: tower.traits.clone(),
            max_level: tower.max_level,
        });
    }

    let mut placed = Vec::new();
    let mut rejected = Vec::new();
    for tower in &plan.towers {
        add(
            trial,
            &mut standing,
            &mut index,
            tower,
            &mut placed,
            &mut rejected,
        );
    }
    stages.push(PlanStage {
        label: "Initial build".into(),
        towers: placed,
        rejected,
    });

    for phase in &plan.phases {
        standing.retain(|tower| {
            if !tower_upgrade_access(trial, tower.spot.node, tower.spot.x) {
                return true;
            }
            if phase.sell.all {
                return phase.sell.keep_types.contains(&tower.type_index);
            }
            !(phase.sell.types.contains(&tower.type_index)
                || phase.sell.plan_idx.contains(&tower.plan_index))
        });
        let mut placed = Vec::new();
        let mut rejected = Vec::new();
        for tower in &phase.towers {
            add(
                trial,
                &mut standing,
                &mut index,
                tower,
                &mut placed,
                &mut rejected,
            );
        }
        stages.push(PlanStage {
            label: if phase.label.is_empty() {
                "Phase".into()
            } else {
                phase.label.clone()
            },
            towers: placed,
            rejected,
        });
    }
    stages
}

/// A warning the Plan pane lists: a missing input or a plan action the
/// simulator would refuse.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanIssue {
    pub message: String,
}

/// Every missing input and refused action a plan carries.
pub fn plan_issues(inputs: &Inputs, trial: &Trial, plan: &Plan) -> Vec<PlanIssue> {
    let mut out = Vec::new();
    let no_levels = inputs.tower_levels.iter().all(|level| *level == 0);
    if no_levels {
        out.push(PlanIssue {
            message: "No tower trial levels: import a save before planning.".into(),
        });
    } else if (0..TOWER_COUNT).all(|index| !inputs.unlocked(index)) {
        out.push(PlanIssue {
            message: "No tower type is unlocked at trial level 1.".into(),
        });
    }

    let all = plan
        .towers
        .iter()
        .chain(plan.phases.iter().flat_map(|phase| phase.towers.iter()));
    let mut locked_types = Vec::new();
    for (index, tower) in all.enumerate() {
        if tower.type_index >= TOWER_COUNT {
            out.push(PlanIssue {
                message: format!("#{index} unknown tower type {}", tower.type_index),
            });
            continue;
        }
        if !no_levels
            && !inputs.unlocked(tower.type_index)
            && !locked_types.contains(&tower.type_index)
        {
            locked_types.push(tower.type_index);
            out.push(PlanIssue {
                message: format!(
                    "#{index} {}: locked at trial level 0",
                    super::data().towers[tower.type_index].name
                ),
            });
        }
    }

    if plan.towers.is_empty() && plan.phases.iter().all(|phase| phase.towers.is_empty()) {
        out.push(PlanIssue {
            message: "The plan has no towers.".into(),
        });
    }
    if plan.roots != RootPolicy::Off {
        out.push(PlanIssue {
            message: "Root casts are not modelled; the plan runs without roots.".into(),
        });
    }

    for stage in plan_stages(trial, plan) {
        for rejected in stage.rejected {
            if rejected.type_index >= TOWER_COUNT {
                continue;
            }
            out.push(PlanIssue {
                message: format!(
                    "#{} {}: {}",
                    rejected.plan_index,
                    super::data().towers[rejected.type_index].name,
                    rejected.reason
                ),
            });
        }
        for tower in stage.towers {
            let check = validate_planning_placement(trial, &[], tower.node, tower.x);
            if !check.ok {
                out.push(PlanIssue {
                    message: format!(
                        "#{} {}: {}",
                        tower.plan_index,
                        super::data().towers[tower.type_index].name,
                        check.reason
                    ),
                });
            }
        }
    }
    out
}

/// The coverage plan: the highest-coverage legal slots, cycling the preferred
/// tower types in the order the upstream heuristic uses.
pub fn heuristic_plan(inputs: &Inputs, trial: &Trial) -> Plan {
    let types: Vec<usize> = (0..TOWER_COUNT)
        .filter(|index| *index != 5 && inputs.unlocked(*index))
        .collect();
    if types.is_empty() {
        return Plan::default();
    }
    let mut slots: Vec<(Spot, usize)> = legal_plan_slots(trial, 20.0)
        .into_iter()
        .map(|spot| (spot, slot_coverage(trial, spot, 160.0)))
        .collect();
    slots.sort_by_key(|slot| std::cmp::Reverse(slot.1));
    let mut chosen = Vec::new();
    for (spot, _) in slots {
        if chosen
            .iter()
            .any(|picked: &Spot| picked.node == spot.node && (picked.x - spot.x).abs() < 62.0)
        {
            continue;
        }
        chosen.push(spot);
        if chosen.len() >= 24 {
            break;
        }
    }
    let preferred: Vec<usize> = [0, 4, 1, 2, 3, 6, 7, 8]
        .into_iter()
        .filter(|index| types.contains(index))
        .collect();
    Plan {
        towers: chosen
            .into_iter()
            .enumerate()
            .map(|(index, slot)| {
                PlanTower::new(
                    preferred[index % preferred.len()],
                    slot.node,
                    slot.x,
                    "AAAAAAAAAAAA",
                    None,
                    1.0,
                )
            })
            .collect(),
        phases: Vec::new(),
        skip_policy: SkipPolicy::Never,
        roots: RootPolicy::Off,
    }
}

/// A community guide plan shipped as a preset.
pub struct GuidePreset {
    pub id: &'static str,
    pub trial: usize,
    pub label: &'static str,
    pub target_wave: u32,
    pub source: &'static str,
    build: fn() -> Plan,
}

impl GuidePreset {
    /// Build this preset's plan.
    pub fn plan(&self) -> Plan {
        (self.build)()
    }
}

impl std::fmt::Debug for GuidePreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuidePreset")
            .field("id", &self.id)
            .field("trial", &self.trial)
            .field("label", &self.label)
            .finish()
    }
}

/// Every shipped guide preset.
fn guide_presets() -> &'static [GuidePreset] {
    &[GuidePreset {
        id: "gublin-230",
        trial: 0,
        label: "Glublin 230 hold + boss DPS",
        target_wave: 230,
        source: "Community guide (Glublin W1 230)",
        build: gublin_230,
    }]
}

/// The guide presets for one trial.
pub fn guide_presets_for_trial(trial: usize) -> impl Iterator<Item = &'static GuidePreset> {
    guide_presets()
        .iter()
        .filter(move |preset| preset.trial == trial)
}

/// A preset by id.
pub fn guide_preset(id: &str) -> Option<&'static GuidePreset> {
    guide_presets().iter().find(|preset| preset.id == id)
}

/// The Glublin 230 guide: a Kraken/Frozone wall, Void at the spawn, Parties
/// behind it, then a boss-DPS rebuild once wave 230 holds at 14 or fewer mobs.
fn gublin_230() -> Plan {
    let wall: Vec<PlanTower> = "KKFKKKFKK"
        .chars()
        .enumerate()
        .map(|(index, kind)| {
            let x = 354.0 + 62.0 * index as f64;
            if kind == 'K' {
                PlanTower::new(6, 5, x, "BAAAAA", Some(25), 1.0)
            } else {
                PlanTower::new(3, 5, x, "AAAAAA", None, 1.0)
            }
        })
        .collect();
    let party_traits = [
        "AAABBB", "AAABBB", "AAABBB", "AAABBB", "AAAABB", "BBBBBB", "BBBBBB", "BBBBBB", "BBBBBB",
        "BBBBBB", "BBBBBB",
    ];
    let parties: Vec<PlanTower> = party_traits
        .iter()
        .enumerate()
        .map(|(index, traits)| PlanTower::new(5, 6, 106.0 + 62.0 * index as f64, traits, None, 2.0))
        .collect();
    let mut towers = vec![PlanTower::new(8, 4, 306.0, "BBBB", None, 0.3)];
    towers.push(PlanTower::new(4, 3, 663.0, "BBBBBA", None, 2.0));
    for x in [725.0, 787.0, 849.0, 911.0] {
        towers.push(PlanTower::new(7, 3, x, "ABABBB", None, 3.0));
    }
    towers.extend(wall);
    towers.extend(parties);
    towers.push(PlanTower::new(7, 6, 794.0, "ABABBB", None, 3.0));

    let mut dps = vec![PlanTower::new(3, 5, 850.0, "AAAAAA", None, 2.0)];
    for x in [354.0, 416.0, 478.0, 540.0] {
        dps.push(PlanTower::new(5, 5, x, "BBBBBB", None, 3.0));
    }
    for x in [602.0, 664.0] {
        dps.push(PlanTower::new(7, 5, x, "ABABBB", None, 4.0));
    }

    Plan {
        towers,
        phases: vec![PlanPhase {
            label: "Boss DPS".into(),
            when: PhaseWhen {
                min_wave: Some(230),
                alive_max: Some(14),
                ..PhaseWhen::default()
            },
            sell: PhaseSell {
                types: vec![8],
                plan_idx: vec![6, 7, 8, 9, 10, 11, 14],
                ..PhaseSell::default()
            },
            towers: dps,
            skip_remaining: true,
        }],
        skip_policy: SkipPolicy::Spam,
        roots: RootPolicy::Danger,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unlocked() -> Inputs {
        Inputs {
            tower_levels: [50; 9],
            start_pts: 100_000.0,
            totem_hp: 50.0,
            ..Inputs::default()
        }
    }

    #[test]
    fn the_heuristic_plan_uses_every_chosen_slot() {
        let trial = crate::trial(0).expect("trial 0");
        let plan = heuristic_plan(&unlocked(), trial);
        assert!(!plan.towers.is_empty());
        assert!(plan.towers.len() <= 24);
        let stages = plan_stages(trial, &plan);
        assert!(stages[0].rejected.is_empty(), "{:?}", stages[0].rejected);
        assert_eq!(stages[0].towers.len(), plan.towers.len());
    }

    #[test]
    fn a_plan_without_tower_levels_is_empty_and_warned() {
        let trial = crate::trial(0).expect("trial 0");
        let plan = heuristic_plan(&Inputs::default(), trial);
        assert!(plan.towers.is_empty());
        let issues = plan_issues(&Inputs::default(), trial, &plan);
        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("trial levels"))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("no towers"))
        );
    }

    #[test]
    fn the_guide_preset_keeps_its_boss_phase() {
        let preset = guide_preset("gublin-230").expect("the guide preset ships");
        let plan = preset.plan();
        assert_eq!(plan.phases.len(), 1);
        assert_eq!(plan.phases[0].when.min_wave, Some(230));
        assert_eq!(plan.phases[0].when.alive_max, Some(14));
        assert!(plan.phases[0].sell.types.contains(&8));
        assert_eq!(plan.skip_policy, SkipPolicy::Spam);
        let trial = crate::trial(0).expect("trial 0");
        let stages = plan_stages(trial, &plan);
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].towers.len(), plan.towers.len());
        assert_eq!(stages[1].label, "Boss DPS");
    }

    #[test]
    fn locked_towers_are_named_in_the_issues() {
        let trial = crate::trial(0).expect("trial 0");
        let plan = Plan {
            towers: vec![PlanTower::new(0, 3, 561.0, "", None, 1.0)],
            ..Plan::default()
        };
        let mut inputs = Inputs::default();
        inputs.tower_levels[1] = 1;
        let issues = plan_issues(&inputs, trial, &plan);
        assert!(issues.iter().any(|issue| issue.message.contains("Pulse")));
    }
}
