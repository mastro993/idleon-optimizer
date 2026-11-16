//! The Mastery EXP tab and the yellow point allocator on the fixture save.

use idleon_cooking_mastery::{EXP_UPGRADES, Goal, Mastery, REQUIRED_LEVEL, UPGRADE_NAMES};
use idleon_import::import;
use serde_json::Value;

/// The imported fixture save's mastery state.
fn imported() -> Mastery {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = import(&save).expect("the fixture imports");
    imported.mastery.expect("the fixture carries CookMaster")
}

/// Equal within a relative tolerance so long products hold.
fn close(got: f64, expected: f64, what: &str) {
    let scale = expected.abs().max(1.0);
    assert!(
        (got - expected).abs() <= scale * 1e-9,
        "{what}: got {got}, expected {expected}"
    );
}

/// The imported fixture's meal state.
fn imported_yellow() -> idleon_cooking_mastery::Yellow {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = import(&save).expect("the fixture imports");
    imported.yellow.expect("the fixture carries CookMaster")
}

#[test]
fn the_fixture_breakdown_root_is_the_rate_and_its_terms_add_up() {
    let mastery = imported();
    let tree = mastery.breakdown();
    assert_eq!(tree.label, "EXP Rate");
    close(tree.value, mastery.exp_rate(), "the tree root");
    let product: f64 = tree.children.iter().map(|child| child.value).product();
    close(product, tree.value, "the factors multiply to the root");
    // Every EXP upgrade is a factor in the tree, and SMOKY is not.
    for index in EXP_UPGRADES {
        assert!(
            tree.children
                .iter()
                .any(|child| child.label == UPGRADE_NAMES[index]),
            "{} is a factor",
            UPGRADE_NAMES[index]
        );
    }
    assert!(!tree.copy_text().contains(UPGRADE_NAMES[3]));
    // The additive group's children still sum to the percentage it reports.
    let building = tree
        .children
        .iter()
        .find(|child| child.label == "External Multipliers")
        .expect("the external group is in the tree");
    let additive = building
        .children
        .iter()
        .find(|child| child.label == "Additive EXP Bonuses")
        .expect("the additive group is in the tree");
    close(
        additive.sum_children(),
        mastery.external.additive(),
        "additive sum",
    );
}

#[test]
fn an_unlocked_rank_changes_the_ranking_but_not_the_save() {
    let mastery = imported();
    assert_eq!(mastery.level, 18.0);
    // SOUR and SAVORY are locked at rank 18: their gain is zero and the ranking
    // lists them anyway, with the requirement it needs.
    for index in [4, 5] {
        let row = mastery
            .ranking()
            .into_iter()
            .find(|row| row.index == index)
            .expect("every EXP upgrade is ranked");
        assert!(row.locked, "{} needs rank {}", row.name, row.required_level);
        assert_eq!(row.required_level, REQUIRED_LEVEL[index]);
        assert_eq!(row.gain, 0.0);
    }
}

#[test]
fn the_allocator_invariant_holds_on_the_fixture() {
    let mastery = imported();
    let total = mastery.points_earned();
    // Every budget the section can ask for spends exactly the clamped budget and
    // never touches a locked upgrade.
    for request in [0.0, 1.0, total / 2.0, total, total + 7.0] {
        let plan = mastery.distribution(Goal::Now, request);
        let spent: f64 = plan.levels.iter().sum();
        close(spent, plan.budget, "the plan spends its budget");
        assert!(plan.budget <= total, "the plan respects the budget");
        for row in &plan.rows {
            assert!(row.planned >= 0.0, "{} has no negative levels", row.name);
            if row.locked {
                assert_eq!(row.planned, 0.0, "{} stays locked", row.name);
            }
        }
        // The plan's rate is its own factors multiplied, so the table explains
        // the number the card shows.
        let factors: f64 = plan.rows.iter().map(|row| row.multiplier).product();
        close(
            plan.rate,
            2.0 * mastery.external.total() * factors,
            "the plan rate",
        );
        // The plan is deterministic: the same save plans the same way.
        assert_eq!(mastery.distribution(Goal::Now, request), plan);
    }
    // The goal switch re-plans on this save: rank 18 leaves SOUR (25) and
    // SAVORY (100) locked, so holding the budget until 25 buys SOUR.
    let now = mastery.distribution(Goal::Now, total);
    let later = mastery.distribution(Goal::NextUnlock, total);
    assert_eq!(later.rank, 25.0);
    assert_ne!(now.levels, later.levels, "the goal switch changes the plan");
    assert_eq!(
        later.levels[4].min(1.0),
        1.0,
        "SOUR takes points at rank 25"
    );
}

#[test]
fn the_fixture_yellow_budget_counts_the_companion_and_the_event_shop() {
    let yellow = imported_yellow();
    // The fixture save already owns the event shop perk (`OptLacc[311]` holds
    // its letter); the variant also owns the Rift Spooker, through the Pet
    // Bonus Tokens at `OptLacc[606]`, which is worth five more points.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let mut save: Value = serde_json::from_str(
        &std::fs::read_to_string(path).expect("fixtures/data.json is committed"),
    )
    .expect("the fixture is JSON");
    let event = "肢";
    let companion = "87";
    let olalacc = save["OptLacc"].as_array_mut().expect("OptLacc is an array");
    let tokens = olalacc[606]
        .as_str()
        .expect("the token list is a string")
        .to_string();
    olalacc[311] = Value::String(event.to_string());
    olalacc[606] = Value::String(format!("{tokens},{companion}"));
    let imported = import(&serde_json::to_string(&save).expect("the variant serialises"))
        .expect("the variant imports");
    let boosted = imported
        .yellow
        .expect("the variant carries CookMaster")
        .budget;
    close(
        boosted - yellow.budget,
        5.0,
        "the Rift Spooker's five points",
    );
}

#[test]
fn the_yellow_allocator_invariant_holds_on_the_fixture() {
    let yellow = imported_yellow();
    let budget = yellow.budget;
    for goal in yellow.goals() {
        let full = yellow
            .allocation(goal.id, budget, 0)
            .expect("an offered goal has a plan");
        // The plan spends its budget and never more, whatever it was asked for.
        close(
            full.spent,
            budget,
            &format!("{} spends the budget", goal.id),
        );
        assert_eq!(
            full.rows.iter().map(|row| row.allocated).sum::<f64>(),
            full.spent,
            "{}: the rows add up to the spend",
            goal.id
        );
        assert!(full.rows.iter().all(|row| row.allocated >= 0.0));
        assert!(
            full.rows
                .iter()
                .all(|row| goal.categories.contains(&row.category.as_str())),
            "{} only spends on its own categories",
            goal.id
        );
        // The rows are ordered by the points they take.
        assert!(
            full.rows
                .windows(2)
                .all(|pair| pair[0].allocated >= pair[1].allocated),
            "{}: the table is ordered by the allocation",
            goal.id
        );
        // The plan is deterministic and never spends on a negative request.
        assert_eq!(yellow.allocation(goal.id, budget, 0), Some(full.clone()));
        let negative = yellow
            .allocation(goal.id, -5.0, 0)
            .expect("an offered goal has a plan");
        assert_eq!(negative.spent, 0.0);
        assert_eq!(negative.value, negative.current_value);
        assert!(
            negative.rejected(),
            "{}: a negative budget is rejected",
            goal.id
        );
        let over = yellow
            .allocation(goal.id, budget + 10.0, 0)
            .expect("an offered goal has a plan");
        assert_eq!(over.spent, budget, "{}: the budget caps the spend", goal.id);
        assert!(
            over.rejected(),
            "{}: an over-budget request is rejected",
            goal.id
        );
    }
    // Switching the goal re-plans on this save: the points land on other meals.
    let first = yellow.allocation("seff", budget, 0).expect("seff");
    let second = yellow.allocation("totdmg", budget, 0).expect("totdmg");
    assert_ne!(first.rows, second.rows, "the goal switch changes the plan");
    assert_ne!(first.value, second.value);
    // The character context moves the stat goals that scale with a character.
    let anna = yellow.allocation("mcook", budget, 0).expect("mcook");
    let other = yellow.allocation("mcook", budget, 1).expect("mcook");
    assert!(anna.value > 0.0 && other.value > 0.0);
    assert_eq!(anna.character_name(&yellow), "Character 1");
    assert!(!yellow.characters.is_empty());
}
