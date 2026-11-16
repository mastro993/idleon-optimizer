use super::*;
use idleon_save::Save;
use serde_json::json;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0)
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
fn a_zero_hour_claim_uses_one_hour() {
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
fn the_projection_length_is_clamped_to_its_range() {
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
