use super::*;

#[test]
fn the_currency_type_changes_every_five_levels() {
    assert_eq!(level_currency(0.0), 0);
    assert_eq!(level_currency(4.9), 0);
    assert_eq!(level_currency(5.0), 1);
    assert_eq!(level_currency(10.0), 2);
    assert_eq!(level_currency(44.0), 8);
    // Levels past the last currency read as the unknown type, as upstream caps.
    assert_eq!(level_currency(50.0), CURRENCIES);
}

#[test]
fn marble_levels_scale_the_bonus() {
    assert_eq!(Fountain::marble_bonus(0.0), 1.0);
    assert_eq!(Fountain::marble_bonus(2.0), 2.5);
    assert_eq!(Fountain::marble_bonus(4.0), 3.5);
}

#[test]
fn the_opal_cost_grows_with_every_level() {
    let mut previous = 0.0;
    for level in 0..20 {
        let cost = Fountain::opal_cost(level as f64);
        assert!(cost > previous, "level {level} costs more than the last");
        previous = cost;
    }
    assert!((Fountain::opal_cost(0.0) - 100.0).abs() < 1e-9);
}

#[test]
fn the_game_logarithm_floors_at_one() {
    assert_eq!(get_log(0.0), 0.0);
    assert!((get_log(10.0) - 1.0).abs() < 1e-4);
    assert!(get_log(1e12) > 10.0);
}

#[test]
fn leading_numbers_survive_the_holes_info_text() {
    assert_eq!(super::currency::leading_number("10TOT"), 10.0);
    assert_eq!(super::currency::leading_number("45TOT"), 45.0);
    assert_eq!(super::currency::leading_number("2"), 2.0);
    assert_eq!(super::currency::leading_number("—"), 0.0);
}
