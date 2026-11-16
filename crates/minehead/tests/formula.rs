//! The Minehead cost path and seeded RNG on the committed synthetic save.

use idleon_minehead::math::DamageInputs;
use idleon_minehead::sim::Options;
use idleon_minehead::{Params, Rng};
use serde_json::Value;

fn levels() -> Vec<f64> {
    // The fixture save's Research row 8.
    let save: Value = serde_json::from_str(include_str!("../../../fixtures/data.json"))
        .expect("the save fixture");
    let rows: Vec<Vec<Value>> =
        serde_json::from_str(save["Research"].as_str().expect("the research string"))
            .expect("the research rows");
    rows[8]
        .iter()
        .map(|value| value.as_f64().unwrap_or(0.0))
        .collect()
}

fn options() -> Options {
    let save: Value =
        serde_json::from_str(include_str!("../../../fixtures/data.json")).expect("the save");
    let rows: Vec<Vec<Value>> =
        serde_json::from_str(save["Research"].as_str().expect("rows")).expect("the research rows");
    let floor = rows[7][4].as_f64().unwrap_or(0.0) as usize;
    Options {
        floor,
        levels: levels(),
        doublers: Vec::new(),
        grid_bonus_166_1: 4.0,
        damage: DamageInputs {
            grid_167: 37.5,
            grid_146: 21.5,
            wep_pow_dmg_pct: 10.0,
            sailing38: 3.0,
            dancing_coral_5: 12.0,
            jelly_damage_9: 8.0,
            legend40: 25.0,
        },
        svar_hp: 1.15,
        max_turns: 200,
        mine_reduction: 1.0,
        legend40_pts: 17.0,
        jelly_gold_tiles45: 3.0,
    }
}

#[test]
fn the_cost_path_spends_until_nothing_is_affordable() {
    let options = options();
    let inputs = idleon_minehead::PathInputs {
        currency: 1e12,
        research_level: 200,
        qty26: options.levels[26],
        server_var_cost: 1.0,
        rog_cost_pct: 0.0,
        max_steps: 3,
    };
    let path =
        idleon_minehead::cost_path(&options, &Params::default(), &inputs, 10, 7, &mut |_| {
            idleon_common::Control::Continue
        })
        .expect("a path");
    assert_eq!(path.steps.len(), 3);
    assert!(path.spent > 0.0);
    assert!(path.remaining_currency < inputs.currency);
    for step in &path.steps {
        assert!(path.final_levels[step.upg_idx] > options.levels[step.upg_idx]);
    }
    assert!(path.baseline.trials > 0);
}

#[test]
fn the_rng_is_the_mulberry32_stream() {
    let mut rng = Rng::new(42);
    assert!((rng.draw() - 0.601_103_751_920_163_6).abs() < 1e-12);
}
