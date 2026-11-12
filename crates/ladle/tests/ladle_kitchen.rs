//! The Kitchens & Jellyfish plan against the upstream calculator.
//!
//! `script/ladle-kitchen-oracle.mjs` runs `mealLevelCost`, `optimizeKitchenReset`,
//! `planMealLeveling` and the No Meal Left Behind rules from
//! `js/stats/systems/w4/cooking-ladle.js` with a model whose account terms are
//! neutral, so the only kitchen-speed terms left are the base speed, the speed
//! level and the gem doubler the port carries. The fixture is that calculator's
//! own output; this test compares every field the section shows.

use idleon_ladle::kitchen::{
    self, CostInputs, Kitchen, LayoutChoice, LayoutMode, Meals, Objective, PhaseKind, Policy,
    Reset, Scenario,
};
use serde::Deserialize;

mod common;
use common::close;

const ORACLE: &str = include_str!("ladle-kitchen-oracle.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Oracle {
    cost_grid: Vec<CostCase>,
    batch_scenarios: Vec<Batch>,
    plan_scenarios: Vec<PlanScenario>,
    forecast_scenarios: Vec<ForecastScenario>,
    nmlb_cases: Vec<NmlbCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CostCase {
    level: f64,
    inputs: InputsCase,
    discount_days: f64,
    cost: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputsCase {
    comp162: f64,
    ach233: f64,
    cloud33: f64,
    dream11: f64,
}

impl InputsCase {
    fn cost_inputs(&self) -> CostInputs {
        CostInputs {
            companion_162: self.comp162,
            achievement_233: self.ach233,
            cloud_33: self.cloud33,
            discount_days: 0.0,
            dream_11: self.dream11,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Batch {
    base_speed: f64,
    rates: Vec<f64>,
    speed_total: f64,
    levels: Vec<f64>,
    stocks: Vec<f64>,
    hours: f64,
    max_level: f64,
    discount_days: f64,
    inputs: InputsCase,
    objective: String,
    targets: Vec<f64>,
    kitchen_layout: ChoiceCase,
    kitchens: Vec<KitchenCase>,
    expected: LayoutCase,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChoiceCase {
    mode: String,
    #[serde(default)]
    pins: std::collections::BTreeMap<usize, i32>,
    #[serde(default)]
    excluded: Vec<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KitchenCase {
    index: usize,
    unlocked: bool,
    saved_meal: i32,
    speed_lv: f64,
    doubler: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutCase {
    #[serde(default)]
    ranks: f64,
    #[serde(default)]
    purchase_order: Vec<usize>,
    #[serde(default)]
    meals: Vec<MealCase>,
    #[serde(default)]
    kitchens: Vec<LayoutKitchenCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MealCase {
    meal: usize,
    from: f64,
    to: f64,
    produced: f64,
    stock: f64,
    progress: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutKitchenCase {
    kitchen: usize,
    saved_meal: i32,
    meal: i32,
    pinned: bool,
    speed: f64,
    produced: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanScenario {
    base_speed: f64,
    rates: Vec<f64>,
    levels: Vec<f64>,
    stocks: Vec<f64>,
    kitchens: Vec<KitchenCase>,
    jelly: JellyCase,
    nmlb: f64,
    inputs: InputsCase,
    settings: SettingsCase,
    expected: PlanCase,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JellyCase {
    available: bool,
    unlocked: bool,
    daily_hours: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsCase {
    days: u32,
    hours_to_reset: f64,
    ladles_per_day: f64,
    start_ladles: f64,
    ladle_multi: f64,
    passive: bool,
    ladleable_only: bool,
    max_level: f64,
    policy: PolicyCase,
    #[serde(default)]
    objective: String,
    #[serde(default)]
    targets: Vec<f64>,
    kitchen_layout: ChoiceCase,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PolicyCase {
    kind: String,
    #[serde(default)]
    income_fraction: f64,
    #[serde(default)]
    per_day: f64,
    #[serde(default)]
    owned_fraction: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanCase {
    days: u32,
    speed: f64,
    daily_income: f64,
    ladle_multiplier: f64,
    max_level: f64,
    jelly: JellyPlanCase,
    start_levels: Vec<f64>,
    levels: Vec<f64>,
    stocks: Vec<f64>,
    gained: Vec<f64>,
    by_source: BySourceCase,
    per_meal: Vec<PerMealCase>,
    ladles_used: f64,
    next_nmlb: f64,
    timeline: Vec<DayCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JellyPlanCase {
    donated: f64,
    daily_hours_end: f64,
    total_hours: f64,
    owned_donation: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerMealCase {
    meal: usize,
    start_level: f64,
    ladle_levels: f64,
    jelly_levels: f64,
    nmlb_levels: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForecastScenario {
    base_speed: f64,
    levels: Vec<f64>,
    stocks: Vec<f64>,
    kitchens: Vec<KitchenCase>,
    jelly: JellyCase,
    nmlb: f64,
    inputs: InputsCase,
    settings: SettingsCase,
    expected: ForecastCase,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForecastCase {
    current: SummaryCase,
    layout: SummaryCase,
    recommended: SummaryCase,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummaryCase {
    ranks: f64,
    donated: f64,
    daily_hours_end: f64,
    levels: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BySourceCase {
    ladle: f64,
    jelly: f64,
    nmlb: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DayCase {
    day: u32,
    window_hours: f64,
    ladle_income: f64,
    expected_income: f64,
    jelly_donation: f64,
    jelly_hours: f64,
    levels_bought: usize,
    ladles_used: f64,
    passive_hours_used: f64,
    nmlb_meal: i32,
    nmlb_from: f64,
    nmlb_to: f64,
    nmlb_saved: f64,
    total_levels: f64,
    meals: Vec<GroupCase>,
    savings: Vec<SavingCase>,
    phases: Vec<PhaseCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupCase {
    meal: usize,
    from: f64,
    to: f64,
    levels: usize,
    ladles: f64,
    hours: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavingCase {
    meal: usize,
    to_level: f64,
    hours: f64,
    ladles: f64,
    progress: f64,
    target: bool,
    map_refresh: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhaseCase {
    kind: String,
    #[serde(default)]
    ladles: f64,
    #[serde(default)]
    expected_ladles: f64,
    #[serde(default)]
    passive_hours: f64,
    #[serde(default)]
    jelly_hours: f64,
    #[serde(default)]
    ladles_used: f64,
    #[serde(default)]
    passive_hours_used: f64,
    #[serde(default)]
    levels_bought: usize,
    #[serde(default)]
    saving: Option<SavingCase>,
    #[serde(default)]
    meals: Vec<RowCase>,
    #[serde(default)]
    layout: Option<LayoutCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RowCase {
    meal: usize,
    from: f64,
    to: f64,
    levels: usize,
    ladles: f64,
    hours: f64,
    cumulative_ladles: f64,
    map_refresh: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NmlbCase {
    levels: Vec<f64>,
    max_level: f64,
    gain: f64,
    target: i32,
    result: f64,
}

fn oracle() -> Oracle {
    serde_json::from_str(ORACLE).expect("the ladle kitchen oracle fixture is valid")
}

fn kitchen(case: &KitchenCase) -> Kitchen {
    Kitchen {
        index: case.index,
        unlocked: case.unlocked,
        saved_meal: case.saved_meal,
        speed_level: case.speed_lv,
        doubler: case.doubler,
    }
}

fn choice(case: &ChoiceCase) -> LayoutChoice {
    LayoutChoice {
        mode: match case.mode.as_str() {
            "saved" => LayoutMode::Saved,
            _ => LayoutMode::Optimized,
        },
        pins: case.pins.clone(),
        excluded: case.excluded.clone(),
        beam_width: 0,
    }
}

fn objective(case: &str) -> Objective {
    match case {
        "targets" => Objective::Targets,
        _ => Objective::Ranks,
    }
}

fn policy(case: &PolicyCase) -> Policy {
    match case.kind.as_str() {
        "all" => Policy::All,
        "share" => Policy::Share,
        "fixed" => Policy::Fixed,
        _ => Policy::None,
    }
}

fn close_list(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let scale = expected.abs().max(1.0);
        assert!(
            (actual - expected).abs() <= 1e-9 * scale,
            "index {index}: {actual} != {expected}"
        );
    }
}

#[test]
fn meal_costs_match_the_upstream_formula() {
    let oracle = oracle();
    for case in &oracle.cost_grid {
        let actual =
            kitchen::meal_level_cost(case.level, case.inputs.cost_inputs(), case.discount_days);
        close(actual, case.cost);
    }
}

#[test]
fn the_reset_batch_matches_the_upstream_search() {
    let oracle = oracle();
    for batch in &oracle.batch_scenarios {
        let kitchens: Vec<Kitchen> = batch.kitchens.iter().map(kitchen).collect();
        let reset = Reset {
            meals: Meals {
                levels: batch.levels.clone(),
                stocks: batch.stocks.clone(),
            },
            kitchens: kitchens.clone(),
            jelly: kitchen::Jelly::default(),
            nmlb: 0.0,
        };
        // The oracle reads its base speed back out of the upstream speed model.
        close(reset.speed(batch.base_speed), batch.speed_total);
        let rates: Vec<f64> = kitchens
            .iter()
            .filter(|kitchen| kitchen.unlocked)
            .map(|kitchen| kitchen.speed(batch.base_speed))
            .collect();
        close_list(&rates, &batch.rates);
        let choice = choice(&batch.kitchen_layout);
        let layout = kitchen::layout::optimize(&kitchen::layout::Input {
            levels: &batch.levels,
            stocks: &batch.stocks,
            kitchens: &kitchens,
            hours: batch.hours,
            base_speed: batch.base_speed,
            max_level: batch.max_level,
            cost: batch.inputs.cost_inputs(),
            discount_days: batch.discount_days,
            objective: objective(&batch.objective),
            targets: &batch.targets,
            choice: &choice,
        })
        .expect("the fixture's layout is valid");
        assert_layout(&layout, &batch.expected);
    }
}

#[test]
fn the_daily_plan_matches_the_upstream_simulation() {
    let oracle = oracle();
    for scenario in &oracle.plan_scenarios {
        let kitchens: Vec<Kitchen> = scenario.kitchens.iter().map(kitchen).collect();
        let reset = Reset {
            meals: Meals {
                levels: scenario.levels.clone(),
                stocks: scenario.stocks.clone(),
            },
            kitchens,
            jelly: kitchen::Jelly {
                available: scenario.jelly.available,
                unlocked: scenario.jelly.unlocked,
                daily_hours: scenario.jelly.daily_hours,
            },
            nmlb: scenario.nmlb,
        };
        let settings = &scenario.settings;
        let inputs = kitchen::Inputs {
            days: settings.days,
            hours_to_reset: settings.hours_to_reset,
            ladles_per_day: settings.ladles_per_day,
            start_ladles: settings.start_ladles,
            ladle_multi: settings.ladle_multi,
            passive: settings.passive,
            ladleable_only: settings.ladleable_only,
            max_level: settings.max_level,
            objective: objective(&settings.objective),
            targets: settings.targets.clone(),
            base_speed: scenario.base_speed,
            cost: scenario.inputs.cost_inputs(),
            policy: policy(&settings.policy),
            income_fraction: settings.policy.income_fraction,
            donation_per_day: settings.policy.per_day,
            owned_fraction: settings.policy.owned_fraction,
            layout: choice(&settings.kitchen_layout),
        };
        close(reset.speed(scenario.base_speed), scenario.expected.speed);
        let rates: Vec<f64> = reset
            .unlocked()
            .map(|kitchen| kitchen.speed(scenario.base_speed))
            .collect();
        close_list(&rates, &scenario.rates);
        let plan = kitchen::plan(&reset, &inputs).expect("the fixture's plan is valid");
        let expected = &scenario.expected;
        assert_eq!(plan.days, expected.days);
        close(plan.daily_income, expected.daily_income);
        close(plan.ladle_multi, expected.ladle_multiplier);
        close(plan.max_level, expected.max_level);
        close(plan.jelly.donated, expected.jelly.donated);
        close(plan.jelly.daily_hours_end, expected.jelly.daily_hours_end);
        close(plan.jelly.total_hours, expected.jelly.total_hours);
        close(plan.jelly.owned_donation, expected.jelly.owned_donation);
        close_list(&plan.start_levels, &expected.start_levels);
        close_list(&plan.levels, &expected.levels);
        close_list(&plan.stocks, &expected.stocks);
        close_list(&plan.gained, &expected.gained);
        close(plan.by_source.ladle, expected.by_source.ladle);
        close(plan.by_source.jelly, expected.by_source.jelly);
        close(plan.by_source.nmlb, expected.by_source.nmlb);
        assert_eq!(plan.per_meal.len(), expected.per_meal.len());
        for (outcome, expected) in plan.per_meal.iter().zip(&expected.per_meal) {
            assert_eq!(outcome.meal, expected.meal);
            close(outcome.start_level, expected.start_level);
            close(outcome.ladle_levels, expected.ladle_levels);
            close(outcome.jelly_levels, expected.jelly_levels);
            close(outcome.nmlb_levels, expected.nmlb_levels);
        }
        close(plan.ladles_used, expected.ladles_used);
        assert_eq!(
            plan.next_nmlb.map_or(-1.0, |meal| meal as f64),
            expected.next_nmlb
        );
        assert_eq!(plan.timeline.len(), expected.timeline.len());
        for (actual, expected) in plan.timeline.iter().zip(&expected.timeline) {
            assert_day(actual, expected);
        }
    }
}

#[test]
fn the_forecast_scenarios_match_the_upstream_comparison() {
    let oracle = oracle();
    for scenario in &oracle.forecast_scenarios {
        let kitchens: Vec<Kitchen> = scenario.kitchens.iter().map(kitchen).collect();
        let reset = Reset {
            meals: Meals {
                levels: scenario.levels.clone(),
                stocks: scenario.stocks.clone(),
            },
            kitchens,
            jelly: kitchen::Jelly {
                available: scenario.jelly.available,
                unlocked: scenario.jelly.unlocked,
                daily_hours: scenario.jelly.daily_hours,
            },
            nmlb: scenario.nmlb,
        };
        let settings = &scenario.settings;
        let inputs = kitchen::Inputs {
            days: settings.days,
            hours_to_reset: settings.hours_to_reset,
            ladles_per_day: settings.ladles_per_day,
            start_ladles: settings.start_ladles,
            ladle_multi: settings.ladle_multi,
            passive: settings.passive,
            ladleable_only: settings.ladleable_only,
            max_level: settings.max_level,
            objective: objective(&settings.objective),
            targets: settings.targets.clone(),
            base_speed: scenario.base_speed,
            cost: scenario.inputs.cost_inputs(),
            policy: policy(&settings.policy),
            income_fraction: settings.policy.income_fraction,
            donation_per_day: settings.policy.per_day,
            owned_fraction: settings.policy.owned_fraction,
            layout: choice(&settings.kitchen_layout),
        };
        let result = kitchen::forecast(&reset, &inputs).expect("the fixture's forecast is valid");
        for (scenario, expected) in [
            (Scenario::Current, &scenario.expected.current),
            (Scenario::Layout, &scenario.expected.layout),
            (Scenario::Recommended, &scenario.expected.recommended),
        ] {
            let plan = result.plan(scenario);
            close(result.ranks(scenario), expected.ranks);
            close(plan.jelly.donated, expected.donated);
            close(plan.jelly.daily_hours_end, expected.daily_hours_end);
            close_list(&plan.levels, &expected.levels);
        }
        assert_eq!(
            result.plan(Scenario::Layout).jelly.donated,
            0.0,
            "the assignment-only baseline never donates"
        );
    }
}

fn assert_day(actual: &kitchen::Day, expected: &DayCase) {
    assert_eq!(actual.day, expected.day);
    close(actual.window_hours, expected.window_hours);
    close(actual.ladle_income, expected.ladle_income);
    close(actual.expected_income, expected.expected_income);
    close(actual.jelly_donation, expected.jelly_donation);
    close(actual.jelly_hours, expected.jelly_hours);
    assert_eq!(actual.levels_bought, expected.levels_bought);
    close(actual.ladles_used, expected.ladles_used);
    close(actual.passive_hours_used, expected.passive_hours_used);
    assert_eq!(actual.nmlb_meal, expected.nmlb_meal);
    close(actual.nmlb_from, expected.nmlb_from);
    close(actual.nmlb_to, expected.nmlb_to);
    close(actual.nmlb_saved, expected.nmlb_saved);
    close(actual.total_levels, expected.total_levels);
    assert_eq!(actual.spend.len(), expected.meals.len());
    for (group, expected) in actual.spend.iter().zip(&expected.meals) {
        assert_eq!(group.meal, expected.meal);
        close(group.from, expected.from);
        close(group.to, expected.to);
        assert_eq!(group.levels, expected.levels);
        close(group.ladles, expected.ladles);
        close(group.hours, expected.hours);
    }
    assert_eq!(actual.savings.len(), expected.savings.len());
    for (saving, expected) in actual.savings.iter().zip(&expected.savings) {
        assert_saving(saving, expected);
    }
    assert_eq!(actual.phases.len(), expected.phases.len());
    for (phase, expected) in actual.phases.iter().zip(&expected.phases) {
        assert_phase(phase, expected);
    }
}

fn assert_phase(actual: &kitchen::Phase, expected: &PhaseCase) {
    let kind = match actual.kind {
        PhaseKind::Now => "now",
        PhaseKind::Claim => "claim",
        PhaseKind::Jelly => "jelly",
    };
    assert_eq!(kind, expected.kind);
    close(actual.ladles, expected.ladles);
    close(actual.expected_ladles, expected.expected_ladles);
    close(actual.passive_hours, expected.passive_hours);
    close(actual.jelly_hours, expected.jelly_hours);
    close(actual.ladles_used, expected.ladles_used);
    close(actual.passive_hours_used, expected.passive_hours_used);
    assert_eq!(actual.levels_bought, expected.levels_bought);
    match (&actual.saving, &expected.saving) {
        (Some(saving), Some(expected)) => assert_saving(saving, expected),
        (None, None) => {}
        (saving, expected) => panic!("saving mismatch: {saving:?} vs {expected:?}"),
    }
    assert_eq!(actual.rows.len(), expected.meals.len());
    for (row, expected) in actual.rows.iter().zip(&expected.meals) {
        assert_eq!(row.meal, expected.meal);
        close(row.from, expected.from);
        close(row.to, expected.to);
        assert_eq!(row.levels, expected.levels);
        close(row.ladles, expected.ladles);
        close(row.hours, expected.hours);
        close(row.cumulative_ladles, expected.cumulative_ladles);
        assert_eq!(row.map_refresh, expected.map_refresh);
    }
    match (&actual.layout, &expected.layout) {
        (Some(layout), Some(expected)) => assert_layout(layout, expected),
        (None, None) => {}
        (layout, expected) => panic!("layout mismatch: {layout:?} vs {expected:?}"),
    }
}

fn assert_layout(actual: &kitchen::Layout, expected: &LayoutCase) {
    close(actual.ranks, expected.ranks);
    assert_eq!(actual.purchase_order, expected.purchase_order);
    assert_eq!(actual.meals.len(), expected.meals.len());
    for (meal, expected) in actual.meals.iter().zip(&expected.meals) {
        assert_eq!(meal.meal, expected.meal);
        close(meal.from, expected.from);
        close(meal.to, expected.to);
        close(meal.produced, expected.produced);
        close(meal.stock, expected.stock);
        close(meal.progress, expected.progress);
    }
    assert_eq!(actual.kitchens.len(), expected.kitchens.len());
    for (kitchen, expected) in actual.kitchens.iter().zip(&expected.kitchens) {
        assert_eq!(kitchen.index, expected.kitchen);
        assert_eq!(kitchen.saved_meal, expected.saved_meal);
        assert_eq!(kitchen.meal, expected.meal);
        assert_eq!(kitchen.pinned, expected.pinned);
        close(kitchen.speed, expected.speed);
        close(kitchen.produced, expected.produced);
    }
}

fn assert_saving(actual: &kitchen::Saving, expected: &SavingCase) {
    assert_eq!(actual.meal, expected.meal);
    close(actual.to_level, expected.to_level);
    close(actual.hours, expected.hours);
    close(actual.ladles, expected.ladles);
    close(actual.progress, expected.progress);
    assert_eq!(actual.target, expected.target);
    assert_eq!(actual.map_refresh, expected.map_refresh);
}

#[test]
fn no_meal_left_behind_matches_the_upstream_rules() {
    let oracle = oracle();
    for case in &oracle.nmlb_cases {
        assert_eq!(
            kitchen::nmlb_target(&case.levels, case.max_level).map_or(-1, |meal| meal as i32),
            case.target,
            "target for {:?}",
            case.levels
        );
        close(
            kitchen::nmlb_result_level(2.0, case.gain, case.max_level),
            case.result,
        );
    }
}

#[test]
fn the_committed_fixture_plans_a_reset_horizon() {
    // The full synthetic account: ten kitchens, 74 discovered meals and the
    // Menagerie grant. The plan must stay finite, never lower a level and come
    // out the same twice.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = idleon_import::import(&text).expect("the fixture imports");
    let ladle = imported
        .ladle
        .expect("the fixture carries a character roster");
    let reset = ladle.kitchen.expect("the fixture carries Cooking rows");
    let inputs = kitchen::Inputs {
        days: 7,
        hours_to_reset: ladle.reset_hours.unwrap_or(24.0),
        ladles_per_day: 150.0,
        start_ladles: ladle.ladles.value,
        ladle_multi: 1.5,
        max_level: 150.0,
        base_speed: 1_000.0,
        policy: kitchen::Policy::Share,
        income_fraction: 0.5,
        ..kitchen::Inputs::default()
    };
    let plan = kitchen::plan(&reset, &inputs).expect("the fixture plans");
    assert_eq!(plan.days, 7);
    assert_eq!(plan.timeline.len(), 7);
    for day in &plan.timeline {
        let layout = day
            .phases
            .iter()
            .find_map(|phase| phase.layout.as_ref())
            .expect("every reset cooks a batch");
        assert_eq!(layout.kitchens.len(), 10);
        assert!(day.instructions().contains("At reset:"));
        assert!(day.jelly_hours.is_finite());
    }
    assert!(
        plan.levels
            .iter()
            .zip(&plan.start_levels)
            .all(|(end, start)| end >= start)
    );
    assert!(plan.gained.iter().all(|gained| gained.is_finite()));
    assert_eq!(
        plan,
        kitchen::plan(&reset, &inputs).expect("the fixture plans again")
    );
}
