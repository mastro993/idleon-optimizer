//! The kitchen module's own rules: what a save supplies, what the plan rejects
//! and what the reset instructions say.

use super::layout::{LayoutChoice, LayoutMode, optimize};
use super::*;
use idleon_save::Save;
use serde_json::json;

/// A small save with two kitchens, four meal levels, the Blue Jellyfish grant
/// and No Meal Left Behind's bundle level.
fn save() -> Save {
    let meals = json!([
        [3, 2, 1, 0, 0, 0],
        [],
        [500, 100, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0],
    ]);
    let cooking = json!([
        [2, 1, 0, 0, 0, 0, 20, 0, 0, 0, 0],
        [1, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ]);
    let menagerie = json!([[0, 0, 0, 0, 0, 9], [], [0, 0, 0, 0, 0, 1], []]);
    let mut gems = vec![0; 121];
    gems[120] = 2;
    Save::new(json!({
        "Meals": meals.to_string(),
        "Cooking": cooking.to_string(),
        "Menagerie": menagerie.to_string(),
        "Ninja": { "102": { "9": "p" } },
        "BundlesReceived": { "bun_s": 1 },
        "GemItemsPurchased": gems,
    }))
}

fn reset() -> Reset {
    Reset::from_save(&save()).expect("the save carries the reset state")
}

/// A nine-hour reset batch over the sample save's kitchens and meals.
fn batch<'a>(reset: &'a Reset, choice: &'a LayoutChoice) -> layout::Input<'a> {
    batch_with(reset, choice, Objective::Ranks, &[])
}

/// The same batch, ranked by an objective and its meal targets.
fn batch_with<'a>(
    reset: &'a Reset,
    choice: &'a LayoutChoice,
    objective: Objective,
    targets: &'a [f64],
) -> layout::Input<'a> {
    layout::Input {
        levels: &reset.meals.levels,
        stocks: &reset.meals.stocks,
        kitchens: &reset.kitchens,
        hours: 9.0,
        base_speed: 100.0,
        max_level: 30.0,
        cost: CostInputs::default(),
        discount_days: 0.0,
        objective,
        targets,
        choice,
    }
}

fn inputs() -> Inputs {
    Inputs {
        days: 3,
        hours_to_reset: 6.0,
        ladles_per_day: 12.0,
        start_ladles: 20.0,
        ladle_multi: 1.5,
        max_level: 30.0,
        base_speed: 100.0,
        policy: Policy::Share,
        income_fraction: 0.5,
        ..Inputs::default()
    }
}

#[test]
fn the_save_supplies_the_meals_kitchens_grant_and_nmlb_levels() {
    let reset = reset();
    assert_eq!(reset.meals.levels, vec![3.0, 2.0, 1.0, 0.0, 0.0, 0.0]);
    assert_eq!(reset.meals.stocks, vec![500.0, 100.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        reset.kitchens,
        vec![
            Kitchen {
                index: 0,
                unlocked: true,
                saved_meal: 1,
                speed_level: 20.0,
                doubler: true,
            },
            Kitchen {
                index: 1,
                unlocked: true,
                saved_meal: -1,
                speed_level: 0.0,
                doubler: true,
            },
            Kitchen {
                index: 2,
                unlocked: false,
                saved_meal: -1,
                speed_level: 0.0,
                doubler: false,
            },
        ]
    );
    // `Menagerie[0][5]` is the saved grant and `Menagerie[2][5]` its unlock.
    assert_eq!(
        reset.jelly,
        Jelly {
            available: true,
            unlocked: true,
            daily_hours: 9.0,
        }
    );
    // `bun_s` awards two levels and emporium 16 (`N2L[16] = 'p'`) one.
    assert_eq!(reset.nmlb, 3.0);
    assert!(reset.nmlb_active());
    // Every unlocked kitchen: base × (1 + Lv / 10) × (1 + the gem doubler).
    assert_eq!(reset.speed(10.0), 10.0 * 3.0 * 3.0 + 10.0 * 3.0);
    assert_eq!(reset.unlocked().count(), 2);
}

#[test]
fn a_save_without_meals_or_kitchens_has_no_reset() {
    assert!(Reset::from_save(&Save::new(json!({}))).is_none());
    assert!(
        Reset::from_save(&Save::new(json!({ "Meals": "[[1],[],[]]" }))).is_none(),
        "meals without kitchens cannot be planned"
    );
    let without_meals = Save::new(json!({
        "Cooking": "[[1,-1,0,0,0,0,0,0,0,0,0]]",
    }));
    assert!(Reset::from_save(&without_meals).is_none());
    // A save without Menagerie or Ninja rows still plans, with no grant or NMLB.
    let bare = Reset::from_save(&Save::new(json!({
        "Meals": "[[1,1],[],[0,0]]",
        "Cooking": "[[1,-1,0,0,0,0,0,0,0,0,0]]",
    })))
    .expect("a bare save still has a reset");
    assert_eq!(bare.jelly, Jelly::default());
    assert_eq!(bare.nmlb, 0.0);
}

#[test]
fn the_horizon_clamps_to_the_upstream_range() {
    let reset = reset();
    let short = plan(
        &reset,
        &Inputs {
            days: 0,
            ..inputs()
        },
    )
    .expect("a zero-day horizon clamps to one");
    assert_eq!(short.days, 1);
    assert_eq!(short.timeline.len(), 1);
    let long = plan(
        &reset,
        &Inputs {
            days: 99_999,
            ..inputs()
        },
    )
    .expect("a long horizon clamps");
    assert_eq!(long.days, MAX_DAYS);
    // The first day's window is the reset countdown, clamped to a day.
    let stretched = plan(
        &reset,
        &Inputs {
            days: 1,
            hours_to_reset: 90.0,
            ..inputs()
        },
    )
    .expect("a stretched window clamps");
    assert_eq!(stretched.timeline[0].window_hours, 24.0);
    assert_eq!(stretched.hours_to_reset, 24.0);
}

#[test]
fn donations_must_be_whole_ladles_from_an_unlocked_jellyfish() {
    let mut locked = reset();
    locked.jelly.unlocked = false;
    let refused = plan(
        &locked,
        &Inputs {
            policy: Policy::All,
            ..inputs()
        },
    );
    assert_eq!(refused, Err(Problem::Locked));
    // Nothing is donated, so a locked jellyfish still plans the meals.
    let quiet = plan(
        &locked,
        &Inputs {
            policy: Policy::None,
            ..inputs()
        },
    )
    .expect("a plan that donates nothing needs no unlock");
    assert_eq!(quiet.jelly.donated, 0.0);
    // A fixed donation above the day's income is capped, never refused.
    let fixed = plan(
        &reset(),
        &Inputs {
            policy: Policy::Fixed,
            donation_per_day: 1_000.0,
            owned_fraction: 0.0,
            ..inputs()
        },
    )
    .expect("a fixed donation caps at the guaranteed income");
    // Three days of guaranteed income, never more.
    assert!(fixed.jelly.donated <= 12.0 * 3.0);
    assert_eq!(fixed.jelly.owned_donation, 0.0);
}

#[test]
fn a_non_finite_meal_state_is_refused() {
    let mut reset = reset();
    reset.meals.levels[0] = f64::NAN;
    assert_eq!(
        plan(&reset, &inputs()),
        Err(Problem::Numbers),
        "a NaN level cannot be planned"
    );
}

#[test]
fn the_layout_refuses_pins_the_reset_cannot_use() {
    let reset = reset();
    let mut pins = Pins::new();
    pins.insert(2, 1);
    let locked = optimize(&batch(
        &reset,
        &LayoutChoice {
            pins,
            ..LayoutChoice::default()
        },
    ));
    assert_eq!(locked, Err(Problem::PinnedKitchen));

    let mut pins = Pins::new();
    pins.insert(0, 1);
    let excluded = optimize(&batch(
        &reset,
        &LayoutChoice {
            pins,
            excluded: vec![1],
            ..LayoutChoice::default()
        },
    ));
    assert_eq!(excluded, Err(Problem::PinnedMeal));
}

#[test]
fn the_saved_mode_cooks_what_the_save_saved() {
    let reset = reset();
    let layout = optimize(&batch(
        &reset,
        &LayoutChoice {
            mode: LayoutMode::Saved,
            ..LayoutChoice::default()
        },
    ))
    .expect("the saved layout is legal");
    // Kitchen 0 saved meal 1, kitchen 1 saved nothing, the locked one is dropped.
    assert_eq!(layout.kitchens.len(), 2);
    assert_eq!(layout.kitchens[0].meal, 1);
    assert_eq!(layout.kitchens[0].saved_meal, 1);
    assert_eq!(layout.kitchens[1].meal, -1);
    assert_eq!(layout.kitchens[1].produced, 0.0);
}

#[test]
fn the_same_inputs_plan_the_same_horizon() {
    let reset = reset();
    let first = plan(&reset, &inputs()).expect("the plan builds");
    let second = plan(&reset, &inputs()).expect("the plan builds again");
    assert_eq!(first, second);
    assert_eq!(first.timeline.len(), 3);
    // The days chain: each day starts from the one before.
    assert!(first.timeline[1].jelly_hours > first.timeline[0].jelly_hours);
}

#[test]
fn the_reset_instructions_list_the_spend_order_the_grant_and_the_batch() {
    let reset = reset();
    let plan = plan(&reset, &inputs()).expect("the plan builds");
    let day = &plan.timeline[0];
    let text = day.instructions();
    assert!(text.starts_with(&day.headline()), "{text}");
    assert!(text.contains("Before reset:"), "{text}");
    assert!(text.contains("donate "), "{text}");
    assert!(text.contains("At reset:"), "{text}");
    assert!(!text.contains("Then "), "{text}");
    assert!(text.contains("Kitchen 1"), "{text}");
    assert!(text.contains("After reset:"), "{text}");
    // The headline names what the reset earned, donated and spent.
    assert!(
        day.headline().starts_with("Reset 1 · "),
        "{}",
        day.headline()
    );
    assert!(day.headline().contains("ranks"), "{}", day.headline());
    // A day with purchases names the meal it levels.
    let rows: Vec<String> = day
        .phases
        .iter()
        .flat_map(|phase| phase.rows.iter())
        .map(|row| meal_name(row.meal).to_owned())
        .collect();
    if let Some(meal) = rows.first() {
        assert!(text.contains(meal), "{meal} missing from {text}");
    }
}

#[test]
fn a_grant_without_a_saved_meal_still_lays_out_the_kitchens() {
    let mut reset = reset();
    reset.kitchens[0].saved_meal = -1;
    reset.meals.levels = vec![200.0; 6];
    let layout = optimize(&batch(&reset, &LayoutChoice::default()))
        .expect("every meal is capped, so nothing may level");
    assert_eq!(layout.ranks, 0.0);
    assert!(layout.purchase_order.is_empty());
    assert!(layout.kitchens.iter().all(|kitchen| kitchen.meal == -1));
}

/// A bare reset: one kitchen at Speed Lv 0 with no doubler, three discovered
/// meals and no Blue Jellyfish or No Meal Left Behind.
fn bare() -> Reset {
    Reset {
        meals: Meals {
            levels: vec![3.0, 3.0, 3.0],
            stocks: vec![0.0; 3],
        },
        kitchens: vec![Kitchen {
            index: 0,
            unlocked: true,
            saved_meal: 0,
            speed_level: 0.0,
            doubler: false,
        }],
        jelly: Jelly::default(),
        nmlb: 0.0,
    }
}

/// One day of direct spending only, so a test reads the picks themselves.
fn bare_inputs() -> Inputs {
    Inputs {
        days: 1,
        hours_to_reset: 24.0,
        ladles_per_day: 30.0,
        passive: false,
        max_level: 30.0,
        base_speed: 100.0,
        ..Inputs::default()
    }
}

#[test]
fn the_objective_switch_changes_which_meal_is_ranked_first() {
    let reset = bare();
    // The cheapest next level is Turkey, so the rank objective spends there.
    let ranks = plan(&reset, &bare_inputs()).expect("the rank objective plans");
    assert!(ranks.gained[0] > 0.0, "{:?}", ranks.gained);
    assert_eq!(ranks.gained[2], 0.0, "{:?}", ranks.gained);
    // A target only on Salad makes it the one direct candidate.
    let targets = plan(
        &reset,
        &Inputs {
            objective: Objective::Targets,
            targets: vec![0.0, 0.0, 10.0],
            ..bare_inputs()
        },
    )
    .expect("the target objective plans");
    assert_eq!(targets.gained[0], 0.0, "{:?}", targets.gained);
    assert_eq!(targets.gained[2], 1.0, "{:?}", targets.gained);
    // The outcome row keeps the level the horizon started from.
    assert_eq!(targets.per_meal[2].start_level, 3.0);
    assert_eq!(targets.per_meal[2].ladle_levels, 1.0);
    assert_eq!(targets.per_meal[2].jelly_levels, 0.0);
}

#[test]
fn the_target_objective_ranks_the_reset_layout_by_target_progress() {
    let reset = bare();
    let ranks = optimize(&batch(&reset, &LayoutChoice::default())).expect("the batch runs");
    assert_eq!(ranks.kitchens[0].meal, 0, "{:?}", ranks.kitchens);
    let targets = [0.0, 0.0, 10.0];
    let targeted = optimize(&batch_with(
        &reset,
        &LayoutChoice::default(),
        Objective::Targets,
        &targets,
    ))
    .expect("the targeted batch runs");
    assert_eq!(targeted.kitchens[0].meal, 2, "{:?}", targeted.kitchens);
}

#[test]
fn meal_costs_use_the_dream_discount_once_a_purchase() {
    let reset = bare();
    let discount = CostInputs {
        discount_days: 10.0,
        dream_11: 10.0,
        ..CostInputs::default()
    };
    // A discounted Turkey level fits in 0.6 ladles; the full price needs 5.5.
    let discounted = plan(
        &reset,
        &Inputs {
            ladles_per_day: 0.6,
            cost: discount,
            ..bare_inputs()
        },
    )
    .expect("the discounted plan runs");
    let full = plan(
        &reset,
        &Inputs {
            ladles_per_day: 0.6,
            ..bare_inputs()
        },
    )
    .expect("the full-price plan runs");
    assert_eq!(discounted.gained[0], 1.0, "{:?}", discounted.gained);
    assert_eq!(full.gained[0], 0.0, "{:?}", full.gained);
    // One purchase spends the day's discount: 20 ladles buy the discounted level
    // and exactly one more at full price, not the four a standing discount would.
    let pool = plan(
        &reset,
        &Inputs {
            ladles_per_day: 20.0,
            cost: discount,
            ..bare_inputs()
        },
    )
    .expect("the one-day plan runs");
    assert_eq!(pool.gained[0], 2.0, "{:?}", pool.gained);
}

#[test]
fn the_target_objective_needs_a_target() {
    let reset = bare();
    assert_eq!(
        forecast(
            &reset,
            &Inputs {
                objective: Objective::Targets,
                targets: vec![0.0, 0.0, 0.0],
                ..bare_inputs()
            }
        ),
        Err(Problem::Targets)
    );
    assert!(
        forecast(
            &reset,
            &Inputs {
                objective: Objective::Targets,
                targets: vec![0.0, 0.0, 10.0],
                ..bare_inputs()
            }
        )
        .is_ok()
    );
}

#[test]
fn the_forecast_compares_the_saved_layout_the_optimized_one_and_the_policy() {
    let reset = reset();
    let inputs = Inputs {
        policy: Policy::Share,
        income_fraction: 0.5,
        owned_fraction: 0.0,
        ..inputs()
    };
    let result = forecast(&reset, &inputs).expect("the forecast builds");
    // Only the donation policy separates the optimized scenario from its
    // assignment-only baseline.
    assert!(result.recommended.jelly.donated > 0.0);
    assert_eq!(result.layout.jelly.donated, 0.0);
    assert_eq!(result.current.jelly.donated, 0.0);
    // The saved kitchen 0 cooks meal 1 while the optimizer may assign both.
    let current = result
        .current
        .timeline
        .iter()
        .find_map(|day| day.phases.iter().find_map(|phase| phase.layout.as_ref()))
        .expect("the current scenario cooks a batch");
    assert_eq!(current.kitchens[0].meal, 1);
    assert_eq!(current.kitchens[1].meal, -1);
    let optimized = result
        .layout
        .timeline
        .iter()
        .find_map(|day| day.phases.iter().find_map(|phase| phase.layout.as_ref()))
        .expect("the optimized scenario cooks a batch");
    assert!(
        optimized.kitchens.iter().any(|kitchen| kitchen.meal != -1),
        "{:?}",
        optimized.kitchens
    );
    // Every scenario summary comes from its own plan.
    assert_eq!(
        result.ranks(Scenario::Layout),
        result.layout.gained.iter().sum::<f64>()
    );
    assert_eq!(
        result.ranks(Scenario::Current),
        result.current.gained.iter().sum::<f64>()
    );
    assert_eq!(
        result,
        forecast(&reset, &inputs).expect("the forecast builds again")
    );
}
