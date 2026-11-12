//! The kitchen module's own rules: what a save supplies, what the plan rejects
//! and what the reset instructions say.

use super::layout::{LayoutChoice, LayoutMode, optimize};
use super::*;
use crate::save::Save;
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
    let mut reset = reset();
    reset.jelly.unlocked = false;
    let refused = plan(
        &reset,
        &Inputs {
            policy: Policy::All,
            ..inputs()
        },
    );
    assert_eq!(refused, Err(Problem::Locked));
    // Nothing is donated, so a locked jellyfish still plans the meals.
    let quiet = plan(
        &reset,
        &Inputs {
            policy: Policy::None,
            ..inputs()
        },
    )
    .expect("a plan that donates nothing needs no unlock");
    assert_eq!(quiet.jelly.donated, 0.0);
    // A fixed donation above the day's income is capped, never refused.
    let unlocked = super::tests::reset();
    let fixed = plan(
        &unlocked,
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
    let locked = optimize(&layout::Input {
        levels: &reset.meals.levels,
        stocks: &reset.meals.stocks,
        kitchens: &reset.kitchens,
        hours: 9.0,
        base_speed: 100.0,
        max_level: 30.0,
        cost: CostInputs::default(),
        discount_days: 0.0,
        choice: &LayoutChoice {
            pins,
            ..LayoutChoice::default()
        },
    });
    assert_eq!(locked, Err(Problem::PinnedKitchen));

    let mut pins = Pins::new();
    pins.insert(0, 1);
    let excluded = optimize(&layout::Input {
        levels: &reset.meals.levels,
        stocks: &reset.meals.stocks,
        kitchens: &reset.kitchens,
        hours: 9.0,
        base_speed: 100.0,
        max_level: 30.0,
        cost: CostInputs::default(),
        discount_days: 0.0,
        choice: &LayoutChoice {
            pins,
            excluded: vec![1],
            ..LayoutChoice::default()
        },
    });
    assert_eq!(excluded, Err(Problem::PinnedMeal));
}

#[test]
fn the_saved_mode_cooks_what_the_save_saved() {
    let reset = reset();
    let layout = optimize(&layout::Input {
        levels: &reset.meals.levels,
        stocks: &reset.meals.stocks,
        kitchens: &reset.kitchens,
        hours: 9.0,
        base_speed: 100.0,
        max_level: 30.0,
        cost: CostInputs::default(),
        discount_days: 0.0,
        choice: &LayoutChoice {
            mode: LayoutMode::Saved,
            ..LayoutChoice::default()
        },
    })
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
    assert!(text.contains("Then ").eq(&false), "{text}");
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
    let layout = optimize(&layout::Input {
        levels: &reset.meals.levels,
        stocks: &reset.meals.stocks,
        kitchens: &reset.kitchens,
        hours: 9.0,
        base_speed: 100.0,
        max_level: 30.0,
        cost: CostInputs::default(),
        discount_days: 0.0,
        choice: &LayoutChoice::default(),
    })
    .expect("every meal is capped, so nothing may level");
    assert_eq!(layout.ranks, 0.0);
    assert!(layout.purchase_order.is_empty());
    assert!(layout.kitchens.iter().all(|kitchen| kitchen.meal == -1));
}
