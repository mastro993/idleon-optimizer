use super::*;
use idleon_save::Save;
use serde::Deserialize;
use serde_json::json;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0)
}

#[derive(Deserialize)]
struct DepthCase {
    toolkit: ToolkitStats,
    equipment: [f64; 3],
    spot: [f64; 4],
    expected: ExpectedDepths,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ExpectedDepths {
    #[serde(rename = "D0")]
    d0: f64,
    #[serde(rename = "D1")]
    d1: f64,
    #[serde(rename = "D2")]
    d2: f64,
    #[serde(rename = "D3")]
    d3: f64,
}

#[derive(Deserialize)]
struct ShareCase {
    efficiency: f64,
    depths: ExpectedDepths,
    #[serde(rename = "effReqs")]
    eff_reqs: [f64; 3],
    expected: [f64; 4],
}

#[derive(Deserialize)]
struct OreCase {
    efficiency: f64,
    #[serde(rename = "effReq")]
    eff_req: f64,
    prowess: f64,
    expected: f64,
}

#[derive(Deserialize)]
struct MultiCase {
    inputs: MultiFishInputs,
    expected: MultiFish,
}

#[derive(Deserialize)]
struct SpeedCase {
    inputs: SpeedInputs,
    expected: f64,
}

#[derive(Debug, Deserialize)]
struct CatchCase {
    speed: f64,
    #[serde(rename = "attackFactor")]
    attack_factor: f64,
    expected: f64,
}

#[derive(Deserialize)]
struct Fixture {
    depths: Vec<DepthCase>,
    shares: Vec<ShareCase>,
    #[serde(rename = "oreValue")]
    ore_value: Vec<OreCase>,
    #[serde(rename = "multiFish")]
    multi_fish: Vec<MultiCase>,
    speed: Vec<SpeedCase>,
    catches: Vec<CatchCase>,
    outcomes: Vec<OutcomeCase>,
    rankings: Vec<RankCase>,
    trench: TrenchFixture,
}

#[derive(Deserialize)]
struct TrenchFixture {
    requirements: Vec<TrenchRequirementCase>,
    plans: Vec<TrenchPlanCase>,
}

#[derive(Deserialize)]
struct TrenchRequirementCase {
    /// `null` keeps the saved depth and its progress.
    layers: Option<f64>,
    #[serde(rename = "effReqs")]
    eff_reqs: [f64; 3],
    expected: FixtureRequirement,
}

#[derive(Deserialize)]
struct FixtureRequirement {
    layers: f64,
    base: f64,
    jar: f64,
    legend: f64,
    bolaia: Vec<FixtureBolaia>,
    #[serde(rename = "reductionPct")]
    reduction_pct: f64,
    value: f64,
    progress: f64,
    remaining: f64,
}

#[derive(Deserialize)]
struct FixtureBolaia {
    cavern: String,
    study: usize,
    #[serde(rename = "perLayer")]
    per_layer: f64,
    layers: f64,
    value: f64,
}

#[derive(Deserialize)]
struct TrenchPlanCase {
    character: RankCharacter,
    account: AccountInputs,
    depths: usize,
    selection: TrenchSelectionCase,
    rows: Vec<FixtureTrenchRow>,
}

#[derive(Deserialize)]
struct TrenchSelectionCase {
    lure: usize,
    line: usize,
}

#[derive(Deserialize)]
struct FixtureTrenchRow {
    depth: f64,
    layers: f64,
    #[serde(rename = "effReqs")]
    eff_reqs: [f64; 3],
    requirement: FixtureRowRequirement,
    selected: FixtureClaim,
    best: FixtureClaim,
}

#[derive(Deserialize)]
struct FixtureRowRequirement {
    base: f64,
    #[serde(rename = "reductionPct")]
    reduction_pct: f64,
    value: f64,
    progress: f64,
    remaining: f64,
}

/// One claim on a projected depth. The selected claim carries no indices in the
/// oracle, so `lure` and `line` are optional there.
#[derive(Deserialize)]
struct FixtureClaim {
    #[serde(default)]
    lure: Option<usize>,
    #[serde(default)]
    line: Option<usize>,
    #[serde(rename = "fishTotal")]
    fish_total: f64,
    exp: f64,
    #[serde(rename = "oreValue")]
    ore_value: f64,
    /// `null` when no fish clear the depth.
    hours: Option<f64>,
    #[serde(rename = "cumulativeHours")]
    cumulative_hours: Option<f64>,
}

#[derive(Deserialize)]
struct RankCase {
    location: String,
    character: RankCharacter,
    account: AccountInputs,
    objective: Objective,
    hours: f64,
    rows: Vec<RankRow>,
}

#[derive(Deserialize)]
struct RankCharacter {
    level: u32,
    #[serde(rename = "rodSpeed")]
    rod_speed: f64,
}

#[derive(Deserialize)]
struct RankRow {
    lure: usize,
    line: usize,
    exp: f64,
    #[serde(rename = "fishTotal")]
    fish_total: f64,
}

#[derive(Deserialize)]
struct OutcomeCase {
    pool: String,
    inputs: FixtureInputs,
    hours: f64,
    expected: FixtureOutcome,
}

#[derive(Deserialize)]
struct FixtureInputs {
    efficiency: f64,
    depths: ExpectedDepths,
    prowess: f64,
    #[serde(rename = "catchesPerHour")]
    catches_per_hour: f64,
    #[serde(rename = "afkRate")]
    afk_rate: f64,
    #[serde(rename = "multiFish")]
    multi_fish: f64,
    #[serde(rename = "expMulti")]
    exp_multi: f64,
}

#[derive(Deserialize)]
struct FixtureOutcome {
    hours: f64,
    shares: [f64; 4],
    #[serde(rename = "shareTotal")]
    share_total: f64,
    #[serde(rename = "oreValue")]
    ore_value: f64,
    catches: f64,
    #[serde(rename = "fishPerCatch")]
    fish_per_catch: f64,
    #[serde(rename = "fishTotal")]
    fish_total: f64,
    #[serde(rename = "rolledFishTotal")]
    rolled_fish_total: f64,
    #[serde(rename = "overwrittenFish")]
    overwritten_fish: f64,
    exp: f64,
    #[serde(rename = "expPerCatch")]
    exp_per_catch: f64,
    #[serde(rename = "panelCatchesPerHour")]
    panel_catches_per_hour: f64,
    #[serde(rename = "panelExpPerHour")]
    panel_exp_per_hour: f64,
    #[serde(rename = "panelOreValue")]
    panel_ore_value: f64,
    fish: Vec<FixtureFish>,
}

#[derive(Deserialize)]
struct FixtureFish {
    slot: usize,
    #[serde(rename = "itemKey")]
    item_key: String,
    share: f64,
    active: bool,
    unlocked: bool,
    #[serde(rename = "effReq")]
    eff_req: f64,
    count: f64,
    exp: f64,
    credited: bool,
    overwritten: bool,
    #[serde(rename = "creditedCount")]
    credited_count: f64,
}

fn fixture() -> Fixture {
    serde_json::from_str(&read("crates/fishing/tests/fishing-fixture.json"))
        .expect("the fishing oracle fixture parses")
}

#[test]
fn the_formula_cases_match_the_upstream_oracle() {
    let fixture = fixture();
    for case in &fixture.depths {
        let expected = &case.expected;
        let actual = depths(&case.toolkit, case.equipment, case.spot);
        assert!(
            close(actual.d0, expected.d0)
                && close(actual.d1, expected.d1)
                && close(actual.d2, expected.d2)
                && close(actual.d3, expected.d3),
            "depths {actual:?} != {expected:?}"
        );
    }
    for case in &fixture.shares {
        let d = &case.depths;
        let actual = catch_shares(
            case.efficiency,
            &Depths {
                d0: d.d0,
                d1: d.d1,
                d2: d.d2,
                d3: d.d3,
            },
            case.eff_reqs,
        );
        assert_eq!(actual, case.expected, "split at {}", case.efficiency);
    }
    for case in &fixture.ore_value {
        assert!(
            close(
                ore_value(case.efficiency, case.eff_req, case.prowess),
                case.expected
            ),
            "ore value at {}",
            case.efficiency
        );
    }
    for case in &fixture.multi_fish {
        let actual = multi_fish(&case.inputs);
        assert!(
            close(actual.value, case.expected.value)
                && close(actual.raw, case.expected.raw)
                && close(actual.cap, case.expected.cap),
            "multi-fish {actual:?} != {:?}",
            case.expected
        );
    }
    for case in &fixture.speed {
        assert!(
            close(speed(&case.inputs), case.expected),
            "speed at {:?}",
            case.inputs
        );
    }
    for case in &fixture.catches {
        assert!(
            close(
                catches_per_hour(case.speed, case.attack_factor),
                case.expected
            ),
            "catches at {:?}",
            case
        );
    }
}

#[test]
fn the_pool_outcomes_match_the_upstream_oracle() {
    let fixture = fixture();
    for case in &fixture.outcomes {
        let pool = data()
            .pool(&case.pool)
            .expect("the fixture names a pooled key");
        let d = &case.inputs.depths;
        let actual = pool_outcome(
            pool,
            &PoolInputs {
                efficiency: case.inputs.efficiency,
                depths: Depths {
                    d0: d.d0,
                    d1: d.d1,
                    d2: d.d2,
                    d3: d.d3,
                },
                prowess: case.inputs.prowess,
                catches_per_hour: case.inputs.catches_per_hour,
                afk_rate: case.inputs.afk_rate,
                multi_fish: case.inputs.multi_fish,
                exp_multi: case.inputs.exp_multi,
                eff_reqs: None,
            },
            case.hours,
        );
        let expected = &case.expected;
        assert!(close(actual.hours, expected.hours), "{} hours", case.pool);
        assert_eq!(actual.shares, expected.shares, "{} shares", case.pool);
        for (name, actual, expected) in [
            ("share total", actual.share_total, expected.share_total),
            ("ore value", actual.ore_value, expected.ore_value),
            ("catches", actual.catches, expected.catches),
            (
                "fish per catch",
                actual.fish_per_catch,
                expected.fish_per_catch,
            ),
            ("fish total", actual.fish_total, expected.fish_total),
            (
                "rolled fish total",
                actual.rolled_fish_total,
                expected.rolled_fish_total,
            ),
            (
                "overwritten fish",
                actual.overwritten_fish,
                expected.overwritten_fish,
            ),
            ("exp", actual.exp, expected.exp),
            (
                "exp per catch",
                actual.exp_per_catch,
                expected.exp_per_catch,
            ),
            (
                "panel catches",
                actual.panel_catches_per_hour,
                expected.panel_catches_per_hour,
            ),
            (
                "panel exp",
                actual.panel_exp_per_hour,
                expected.panel_exp_per_hour,
            ),
            (
                "panel ore",
                actual.panel_ore_value,
                expected.panel_ore_value,
            ),
        ] {
            assert!(
                close(actual, expected),
                "{} {name}: {actual} != {expected}",
                case.pool
            );
        }
        assert_eq!(actual.fish.len(), expected.fish.len(), "{} fish", case.pool);
        for (fish, expected) in actual.fish.iter().zip(&expected.fish) {
            assert_eq!(fish.slot, expected.slot);
            assert_eq!(fish.item_key, expected.item_key);
            assert!(close(fish.share, expected.share), "{} share", case.pool);
            assert_eq!(fish.active, expected.active, "{} active", case.pool);
            assert_eq!(fish.unlocked, expected.unlocked, "{} unlocked", case.pool);
            assert!(close(fish.eff_req, expected.eff_req));
            assert!(close(fish.count, expected.count), "{} count", case.pool);
            assert!(close(fish.exp, expected.exp), "{} fish exp", case.pool);
            assert_eq!(fish.credited, expected.credited, "{} credited", case.pool);
            assert_eq!(
                fish.overwritten, expected.overwritten,
                "{} overwritten",
                case.pool
            );
            assert!(close(fish.credited_count, expected.credited_count));
        }
    }
}

/// One level-100 character with a saved toolkit, parked on the medium pool.
fn ana() -> FishingSave {
    FishingSave::from_save(&Save::new(json!({
        "charNames": ["Ana"],
        "Lv0_0": [0, 0, 0, 0, 100],
        "EquipOrder_0": [[], { "2": "FishingRod13" }],
        "PVFishingToolkit_0": [8, 4],
        "PVFishingSpotIndex_0": 1,
        "CurrentMap_0": 61,
        "AFKtarget_0": "FishMed",
    })))
}

#[test]
fn the_save_decodes_levels_rods_toolkits_and_the_saved_spot() {
    let fishing = ana();
    let character = fishing.character(0).expect("one character");
    assert_eq!(character.name, "Ana");
    assert_eq!(character.level, 100);
    assert_eq!(character.rod_key, "FishingRod13");
    assert_eq!(character.rod_name, "Prehistoric Rod");
    assert_eq!(character.rod_speed, 8.0);
    assert_eq!((character.saved_lure, character.saved_line), (8, 4));
    assert!(character.toolkit_available);
    assert_eq!(character.saved_map, Some(61));
    assert_eq!(character.saved_spot, Some(1));
    assert!(character.spot_available);
    assert_eq!(character.afk_target, "FishMed");
    assert_eq!(
        character
            .saved_location()
            .map(|location| location.id.as_str()),
        Some("61:1")
    );
    assert_eq!(fishing.default_character(), 0);
}

#[test]
fn the_toolkit_rankings_match_the_upstream_oracle() {
    let fixture = fixture();
    assert!(!fixture.rankings.is_empty(), "the oracle carries rankings");
    for case in &fixture.rankings {
        let save = FishingSave {
            characters: vec![Character {
                level: case.character.level,
                rod_speed: case.character.rod_speed,
                ..Character::default()
            }],
            ..FishingSave::default()
        };
        let rows = rank(
            &save,
            &case.account,
            0,
            &case.location,
            case.objective,
            case.hours,
        );
        assert_eq!(
            rows.len(),
            225,
            "{} covers every combination",
            case.location
        );
        assert_eq!(rows.len(), case.rows.len(), "{} rows", case.location);
        for (index, (ranked, expected)) in rows.iter().zip(&case.rows).enumerate() {
            assert_eq!(
                (ranked.lure.idx, ranked.line.idx),
                (expected.lure, expected.line),
                "{} rank {}",
                case.location,
                index + 1
            );
            assert!(
                close(ranked.outcome.exp, expected.exp),
                "{} rank {} EXP: {} != {}",
                case.location,
                index + 1,
                ranked.outcome.exp,
                expected.exp
            );
            assert!(
                close(ranked.outcome.fish_total, expected.fish_total),
                "{} rank {} fish: {} != {}",
                case.location,
                index + 1,
                ranked.outcome.fish_total,
                expected.fish_total
            );
        }
    }
}

#[test]
fn a_ranking_covers_every_pair_once_and_sorts_by_the_objective() {
    let save = FishingSave {
        characters: vec![Character {
            level: 120,
            rod_speed: 8.0,
            ..Character::default()
        }],
        ..FishingSave::default()
    };
    let account = AccountInputs {
        efficiency: 5_000.0,
        exp_multi: 2.5,
        afk_rate: 0.42,
        multi_fish: 28.0,
        speed_bonus: 20.0,
        attack_factor: 1.4,
        prowess: 0.12,
        equipment_depth: [0.0; 3],
    };
    let pairs = data().lures.len() * data().lines.len();
    for (location, objective) in [
        ("54:0", Objective::Exp),
        ("54:0", Objective::Fish),
        ("x:MotherlodeFISH", Objective::Exp),
        ("72:FishBig", Objective::Fish),
    ] {
        let rows = rank(&save, &account, 0, location, objective, 2.0);
        assert_eq!(rows.len(), pairs, "{location}");
        let mut seen = vec![false; pairs];
        for row in &rows {
            let index = row.lure.idx * data().lines.len() + row.line.idx;
            assert!(!seen[index], "{location} repeats {index}");
            seen[index] = true;
        }
        assert!(seen.iter().all(|seen| *seen), "{location} misses a pair");
        let best = objective.metric(&rows[0].outcome);
        let tied = rows
            .iter()
            .take_while(|row| objective.metric(&row.outcome) == best)
            .count();
        assert_eq!(ties(&rows, objective), tied, "{location}");
        for pair in rows.windows(2) {
            let (first, second) = (&pair[0].outcome, &pair[1].outcome);
            let metric = objective.metric(first).total_cmp(&objective.metric(second));
            let tie_break = first
                .exp
                .total_cmp(&second.exp)
                .then(first.fish_total.total_cmp(&second.fish_total));
            assert!(
                metric.is_ge() && (metric.is_ne() || tie_break.is_ge()),
                "{location} sorts {first:?} before {second:?}"
            );
        }
    }
}

#[test]
fn floored_counts_tie_and_the_helper_counts_them() {
    let save = FishingSave {
        characters: vec![Character {
            level: 1,
            rod_speed: 0.0,
            ..Character::default()
        }],
        ..FishingSave::default()
    };
    let rows = rank(
        &save,
        &AccountInputs::default(),
        0,
        "72:FishBig",
        Objective::Fish,
        1.0,
    );
    assert!(ties(&rows, Objective::Fish) > 1, "the neutral account ties");
    assert_eq!(ties(&[], Objective::Fish), 0);
}

#[test]
fn a_save_without_a_toolkit_key_is_flagged_and_defaults_to_fly_and_twine() {
    let save = Save::new(json!({
        "Lv0_0": [0, 0, 0, 0, 40],
        "CurrentMap_0": 54,
        "AFKtarget_0": "FishSmall",
    }));
    let fishing = FishingSave::from_save(&save);
    let character = fishing.character(0).expect("one character");
    assert!(!character.toolkit_available);
    assert_eq!((character.saved_lure, character.saved_line), (0, 0));
    assert!(!character.spot_available);
    assert_eq!(character.rod_key, "Blank");
    assert_eq!(character.rod_name, "No rod");
    assert_eq!(
        character
            .saved_location()
            .map(|location| location.id.as_str()),
        Some("54:0")
    );
}

#[test]
fn the_synthetic_account_fixture_decodes_its_first_character() {
    let fishing = fixture_save();
    let character = fishing.character(0).expect("the fixture has characters");
    assert_eq!(character.level, 100);
    assert_eq!(character.rod_key, "FishingRod13");
    assert_eq!((character.saved_lure, character.saved_line), (0, 4));
    assert!(character.toolkit_available);
    // The fixture's characters are all on map 200, which has no fishing spot.
    assert!(character.saved_location().is_none());
    // The cavern rows give the Trench its depth and its reductions.
    assert!(fishing.trench.is_some());
}

/// The committed synthetic account, read through the fishing slice.
fn fixture_save() -> FishingSave {
    let save = serde_json::from_str(&read("fixtures/data.json")).expect("the fixture is JSON");
    FishingSave::from_save(&Save::new(save))
}

#[test]
fn switching_the_pool_changes_the_split_and_the_panel_ratio() {
    let fishing = ana();
    let account = AccountInputs {
        efficiency: 2_000_000.0,
        multi_fish: 75.0,
        exp_multi: 4.0,
        afk_rate: 0.5,
        ..AccountInputs::default()
    };
    let small = evaluate(&fishing, &account, 0, 8, 4, "54:0", 1.0);
    let medium = evaluate(&fishing, &account, 0, 8, 4, "61:1", 1.0);
    assert_eq!(small.location.pool_key, "FishSmall");
    assert_eq!(medium.location.pool_key, "FishMed");
    assert_ne!(small.outcome.shares, medium.outcome.shares);
    assert_ne!(small.outcome.exp, medium.outcome.exp);
    assert!(medium.outcome.panel_exp_per_hour > 0.0);
    assert!(small.depths.d3 != medium.depths.d3);
}

#[test]
fn the_breakdown_groups_add_up_to_the_outcome() {
    let save = Save::new(json!({
        "Lv0_0": [0, 0, 0, 0, 120],
        "EquipOrder_0": [[], { "2": "FishingRod10" }],
        "PVFishingToolkit_0": [3, 4],
        "CurrentMap_0": 55,
        "AFKtarget_0": "FishSmall",
    }));
    let fishing = FishingSave::from_save(&save);
    let account = AccountInputs {
        efficiency: 5_000.0,
        multi_fish: 28.0,
        exp_multi: 2.5,
        afk_rate: 0.42,
        speed_bonus: 20.0,
        ..AccountInputs::default()
    };
    let evaluation = evaluate(&fishing, &account, 0, 3, 4, "55:1", 2.0);
    let tree = breakdown(&evaluation);
    assert!(close(tree.value, evaluation.outcome.fish_total));
    let split = &tree.children[2];
    assert_eq!(split.label, "Fish split");
    assert!(close(split.sum_children(), evaluation.outcome.share_total));
    let fish = &tree.children[3];
    assert_eq!(fish.label, "Fish");
    assert!(close(fish.sum_children(), evaluation.outcome.fish_total));
    let exp = &tree.children[4];
    assert_eq!(exp.label, "Fishing EXP");
    assert!(
        (exp.sum_children() - evaluation.outcome.exp).abs() <= 1.0,
        "the per-fish terms add up before the EXP floor"
    );
    assert!(tree.match_count("Catch time") >= 1);
    assert!(tree.match_count("Fishing") >= 1);
    assert_eq!(tree.match_count("nothing here"), 0);
}

#[test]
fn an_out_of_range_toolkit_falls_back_to_the_first_option() {
    let fishing = FishingSave::from_save(&Save::new(json!({})));
    let evaluation = evaluate(
        &fishing,
        &AccountInputs::default(),
        3,
        99,
        99,
        "x:FishEquinox",
        1.0,
    );
    assert_eq!(evaluation.lure.name, "Fly");
    assert_eq!(evaluation.line.name, "Fishing Twine");
    assert_eq!(evaluation.character.name, "Character 4");
    assert_eq!(evaluation.location.pool_key, "FishEquinox");
}

#[test]
fn a_zero_hour_claim_uses_one_hour_like_the_upstream() {
    let pool = data().pool("FishSmall").expect("pool");
    let outcome = pool_outcome(
        pool,
        &PoolInputs {
            efficiency: 5000.0,
            depths: Depths::default(),
            prowess: 0.0,
            catches_per_hour: 220.0,
            afk_rate: 0.42,
            multi_fish: 0.0,
            exp_multi: 1.0,
            eff_reqs: None,
        },
        0.0,
    );
    assert_eq!(outcome.hours, 1.0);
}

#[test]
fn the_trench_requirement_matches_the_upstream_oracle() {
    let fixture = fixture();
    let save = fixture_save();
    let trench = save.trench.as_ref().expect("the fixture carries Holes");
    assert_eq!(trench.layers(), 9.0);
    for case in &fixture.trench.requirements {
        let actual = trench.requirement(case.layers);
        let expected = &case.expected;
        assert_eq!(actual.layers, expected.layers, "layers");
        assert_eq!(trench.eff_reqs(actual.layers), case.eff_reqs, "eff reqs");
        let depth = expected.layers;
        for (name, actual, expected) in [
            ("base", actual.base, expected.base),
            ("jar", actual.jar, expected.jar),
            ("legend", actual.legend, expected.legend),
            ("reduction", actual.reduction_pct, expected.reduction_pct),
            ("value", actual.value, expected.value),
            ("progress", actual.progress, expected.progress),
            ("remaining", actual.remaining, expected.remaining),
        ] {
            assert!(
                close(actual, expected),
                "depth {depth} {name}: {actual} != {expected}"
            );
        }
        assert_eq!(actual.bolaia.len(), expected.bolaia.len());
        for (actual, expected) in actual.bolaia.iter().zip(&expected.bolaia) {
            assert_eq!(actual.cavern, expected.cavern);
            assert_eq!(actual.study, expected.study);
            assert!(close(actual.per_layer, expected.per_layer));
            assert!(close(actual.layers, expected.layers));
            assert!(close(actual.value, expected.value));
        }
    }
}

#[test]
fn the_trench_projection_matches_the_upstream_oracle() {
    let fixture = fixture();
    let trench = fixture_save().trench.expect("the fixture carries Holes");
    assert!(!fixture.trench.plans.is_empty(), "the oracle carries plans");
    for case in &fixture.trench.plans {
        let save = FishingSave {
            characters: vec![Character {
                level: case.character.level,
                rod_speed: case.character.rod_speed,
                ..Character::default()
            }],
            trench: Some(trench.clone()),
        };
        let plan = trench_plan(
            &save,
            &case.account,
            &TrenchRequest {
                character: 0,
                lure: case.selection.lure,
                line: case.selection.line,
                depths: case.depths,
                mode: HelperMode::Saved,
                helpers: &[],
            },
        )
        .expect("the fixture has a Trench");
        assert_eq!(plan.rows.len(), case.rows.len());
        for (row, expected) in plan.rows.iter().zip(&case.rows) {
            assert_eq!(row.depth, expected.depth);
            assert!(close(row.layers, expected.layers));
            assert_eq!(row.eff_reqs, expected.eff_reqs, "depth {}", row.depth);
            assert!(
                close(row.requirement.base, expected.requirement.base),
                "depth {} base",
                row.depth
            );
            assert!(
                close(
                    row.requirement.reduction_pct,
                    expected.requirement.reduction_pct
                ),
                "depth {} reduction",
                row.depth
            );
            assert!(
                close(row.requirement.value, expected.requirement.value),
                "depth {} value",
                row.depth
            );
            assert!(
                close(row.requirement.progress, expected.requirement.progress),
                "depth {} progress",
                row.depth
            );
            assert!(
                close(row.requirement.remaining, expected.requirement.remaining),
                "depth {} remaining",
                row.depth
            );
            for (name, claim, expected) in [
                ("selected", &row.selected, &expected.selected),
                ("best", &row.best, &expected.best),
            ] {
                assert!(
                    close(claim.outcome.fish_total, expected.fish_total),
                    "depth {} {name} fish: {} != {}",
                    row.depth,
                    claim.outcome.fish_total,
                    expected.fish_total
                );
                assert!(
                    close(claim.outcome.exp, expected.exp),
                    "depth {} {name} EXP",
                    row.depth
                );
                assert!(
                    close(claim.outcome.ore_value, expected.ore_value),
                    "depth {} {name} ore value",
                    row.depth
                );
                assert!(
                    hours_match(claim.hours, expected.hours),
                    "depth {} {name} hours: {} != {:?}",
                    row.depth,
                    claim.hours,
                    expected.hours
                );
                assert!(
                    hours_match(claim.cumulative_hours, expected.cumulative_hours),
                    "depth {} {name} cumulative hours: {} != {:?}",
                    row.depth,
                    claim.cumulative_hours,
                    expected.cumulative_hours
                );
                if let Some(lure) = expected.lure {
                    assert_eq!(claim.lure, lure, "depth {} lure", row.depth);
                }
                if let Some(line) = expected.line {
                    assert_eq!(claim.line, line, "depth {} line", row.depth);
                }
            }
        }
    }
}

/// An endless projection reads as `null` in the JSON fixture.
fn hours_match(actual: f64, expected: Option<f64>) -> bool {
    expected.map_or(actual.is_infinite(), |expected| close(actual, expected))
}

#[test]
fn the_trench_location_uses_the_saved_depth_requirements() {
    let save = fixture_save();
    let account = AccountInputs {
        efficiency: 20_000_000.0,
        ..AccountInputs::default()
    };
    let trench = save.trench.as_ref().expect("the fixture carries Holes");
    let with_depth = evaluate(&save, &account, 0, 0, 4, "x:MotherlodeFISH", 1.0);
    assert_eq!(
        with_depth.outcome.eff_reqs,
        trench.eff_reqs(trench.layers()),
        "the Trench swaps the pool's own requirements"
    );
    let without_depth = evaluate(
        &FishingSave {
            trench: None,
            ..save.clone()
        },
        &account,
        0,
        0,
        4,
        "x:MotherlodeFISH",
        1.0,
    );
    assert_ne!(
        with_depth.outcome.eff_reqs, without_depth.outcome.eff_reqs,
        "a save without Holes keeps FishPools' requirements"
    );
    // The requirement is what the per-fish rows and the ore value read.
    assert_eq!(
        with_depth.outcome.fish[1].eff_req,
        with_depth.outcome.eff_reqs[0]
    );
}

/// A two-character roster on the Trench: the selected character plus one helper.
fn trench_roster() -> FishingSave {
    let trench = fixture_save().trench.expect("the fixture carries Holes");
    FishingSave {
        characters: vec![
            Character {
                index: 0,
                name: "Ana".into(),
                level: 250,
                rod_speed: 8.0,
                saved_lure: 0,
                saved_line: 4,
                toolkit_available: true,
                afk_target: "MotherlodeFISH".into(),
                ..Character::default()
            },
            Character {
                index: 1,
                name: "Bo".into(),
                level: 120,
                rod_speed: 3.0,
                saved_lure: 2,
                saved_line: 2,
                toolkit_available: true,
                afk_target: "MotherlodeFISH".into(),
                ..Character::default()
            },
        ],
        trench: Some(trench),
    }
}

#[test]
fn the_trench_character_contributions_sum_into_the_team() {
    let save = trench_roster();
    let account = AccountInputs {
        efficiency: 20_000_000.0,
        afk_rate: 0.9,
        exp_multi: 4.0,
        multi_fish: 50.0,
        speed_bonus: 40.0,
        attack_factor: 1.5,
        prowess: 0.2,
        ..AccountInputs::default()
    };
    let plan = trench_plan(
        &save,
        &account,
        &TrenchRequest {
            character: 0,
            lure: 0,
            line: 4,
            depths: 4,
            mode: HelperMode::Saved,
            helpers: &[false, true],
        },
    )
    .expect("the roster has a Trench");
    assert_eq!(plan.rows.len(), 4);
    assert_eq!(
        plan.helpers.len(),
        1,
        "the selected character is not a helper"
    );
    let helper = &plan.helpers[0];
    assert_eq!(helper.character, 1);
    assert!(helper.enabled);
    assert!(helper.at_trench);
    assert_eq!(helper.rates.len(), plan.rows.len());
    // The current-depth team rate is the selected claim plus every ticked helper.
    let expected_rate = plan.rows[0].selected.outcome.fish_total + helper.rates[0];
    assert!(close(plan.team.rate, expected_rate));
    assert!(close(plan.team.helpers_rate, helper.rates[0]));
    assert!(
        close(
            plan.team.hours,
            plan.rows[0].requirement.remaining / expected_rate
        ),
        "the team ETA divides the requirement by the team rate"
    );
    // Every projected depth sums the same way, and the total accumulates.
    let mut cumulative = 0.0;
    for (depth, (row, team)) in plan.rows.iter().zip(&plan.team.rows).enumerate() {
        let expected = row.selected.outcome.fish_total + helper.rates[depth];
        assert!(close(team.rate, expected), "depth {depth} team rate");
        assert!(
            close(team.hours, row.requirement.remaining / expected),
            "depth {depth} team hours"
        );
        cumulative += team.hours;
        assert!(close(team.cumulative_hours, cumulative));
    }
    assert!(plan.team.rate > plan.rows[0].selected.outcome.fish_total);
}

#[test]
fn an_unticked_helper_leaves_the_team_alone() {
    let save = trench_roster();
    let account = AccountInputs {
        efficiency: 20_000_000.0,
        ..AccountInputs::default()
    };
    let request = |helpers: &'static [bool]| TrenchRequest {
        character: 0,
        lure: 0,
        line: 4,
        depths: 2,
        mode: HelperMode::Saved,
        helpers,
    };
    let alone = trench_plan(&save, &account, &request(&[false, false])).expect("a Trench");
    let together = trench_plan(&save, &account, &request(&[false, true])).expect("a Trench");
    assert_eq!(alone.helpers.len(), 1, "an unticked helper is still listed");
    assert!(!alone.helpers[0].enabled);
    assert_eq!(alone.team.helpers_rate, 0.0);
    assert!(close(
        alone.team.rate,
        alone.rows[0].selected.outcome.fish_total
    ));
    assert!(together.team.rate > alone.team.rate);
}

#[test]
fn the_fastest_helper_toolkit_follows_the_mode() {
    let save = trench_roster();
    let account = AccountInputs {
        efficiency: 20_000_000.0,
        multi_fish: 50.0,
        afk_rate: 0.9,
        ..AccountInputs::default()
    };
    let plan = |mode| {
        trench_plan(
            &save,
            &account,
            &TrenchRequest {
                character: 0,
                lure: 0,
                line: 4,
                depths: 3,
                mode,
                helpers: &[false, true],
            },
        )
        .expect("a Trench")
    };
    let saved = plan(HelperMode::Saved);
    let best = plan(HelperMode::Best);
    assert_eq!((saved.helpers[0].lure, saved.helpers[0].line), (2, 2));
    for (saved, best) in saved.helpers[0].rates.iter().zip(&best.helpers[0].rates) {
        assert!(
            best >= saved,
            "the fastest toolkit never catches less: {best} < {saved}"
        );
    }
    // The selected character's plan does not depend on the helper mode.
    assert_eq!(
        saved.rows[0].best.outcome.fish_total,
        best.rows[0].best.outcome.fish_total
    );
}

#[test]
fn an_empty_roster_and_a_save_without_holes_have_a_state() {
    let save = trench_roster();
    let account = AccountInputs {
        efficiency: 20_000_000.0,
        ..AccountInputs::default()
    };
    let alone = FishingSave {
        characters: save.characters[..1].to_vec(),
        trench: save.trench.clone(),
    };
    let plan = trench_plan(
        &alone,
        &account,
        &TrenchRequest {
            character: 0,
            lure: 0,
            line: 4,
            depths: 2,
            mode: HelperMode::Saved,
            helpers: &[],
        },
    )
    .expect("a Trench");
    assert!(plan.helpers.is_empty(), "no other character to fish with");
    assert_eq!(plan.team.helpers_rate, 0.0);
    assert!(close(
        plan.team.rate,
        plan.rows[0].selected.outcome.fish_total
    ));

    assert!(
        trench_plan(
            &FishingSave {
                characters: save.characters,
                trench: None,
            },
            &account,
            &TrenchRequest {
                character: 0,
                lure: 0,
                line: 4,
                depths: 2,
                mode: HelperMode::Saved,
                helpers: &[],
            },
        )
        .is_none(),
        "a save without Holes has no Trench to project"
    );
}

#[test]
fn the_projection_length_is_clamped_to_the_upstream_range() {
    let save = trench_roster();
    let account = AccountInputs {
        efficiency: 20_000_000.0,
        ..AccountInputs::default()
    };
    for (depths, expected) in [(0, 1), (3, 3), (25, 25), (200, 25)] {
        let plan = trench_plan(
            &save,
            &account,
            &TrenchRequest {
                character: 0,
                lure: 0,
                line: 4,
                depths,
                mode: HelperMode::Saved,
                helpers: &[],
            },
        )
        .expect("a Trench");
        assert_eq!(plan.rows.len(), expected, "{depths} projects {expected}");
        assert_eq!(plan.rows[0].depth, plan.trench.layers() + 1.0);
        assert_eq!(
            plan.rows.last().map(|row| row.layers),
            Some(plan.trench.layers() + expected as f64 - 1.0)
        );
    }
}
