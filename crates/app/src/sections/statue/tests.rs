//! Statue table search and sort.

use super::*;
use idleon_core::statue::Progress;

fn sample(level: u32, exp: f64) -> StatueSave {
    let mut rows = vec![Some(Progress { level, exp }); statue::count()];
    rows[0] = Some(Progress { level, exp });
    StatueSave::new(vec![rows], vec!["main".into()])
}

fn row(save: &StatueSave, index: usize) -> Row {
    let progress = save.progress(0, index);
    let target = progress.map(|progress| progress.level + 1);
    let cost = target
        .map(|target| statue::target_cost(save, save.leveling(), 0, index, target))
        .unwrap_or(Err(Unavailable::NoRow(index)));
    Row {
        index,
        info: statue::info(index).expect("a statue"),
        progress,
        next: statue::next_level(save, 0, index),
        target,
        cost,
        rate_text: String::new(),
        rate: Ok(0.0),
        statues: None,
        hours: f64::INFINITY,
    }
}

#[test]
fn search_reads_the_name_and_the_effect() {
    let power = statue::info(0).expect("POWER");
    assert!(matches(power, ""));
    assert!(matches(power, "power"));
    assert!(matches(power, "DAMAGE"));
    assert!(!matches(power, "mining"));

    let mining = statue::info(2).expect("MINING");
    assert!(matches(mining, "mining"));
    assert!(matches(mining, "power"));
    assert!(!matches(mining, "fishing"));
}

#[test]
fn sort_orders_by_level_then_by_the_cheapest_next_level() {
    let same = sample(10, 0.0);
    let mut rows = vec![row(&same, 2), row(&same, 0), row(&same, 1)];
    sort_rows(&mut rows, Sort::Game);
    assert_eq!(
        rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        [2, 0, 1]
    );

    sort_rows(&mut rows, Sort::Level);
    assert_eq!(
        rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        [0, 1, 2],
        "equal levels fall back to game order"
    );

    let mut rows = vec![row(&sample(100, 0.0), 0), row(&sample(1, 0.0), 1)];
    sort_rows(&mut rows, Sort::Remaining);
    assert_eq!(rows[0].index, 1, "the low-level statue needs less EXP");
}

#[test]
fn unavailable_rows_sort_after_available_ones() {
    let mut rows = vec![Some(Progress { level: 5, exp: 0.0 }); statue::count()];
    rows[1] = None;
    let save = StatueSave::new(vec![rows], vec!["main".into()]);
    let mut entries = vec![row(&save, 1), row(&save, 0)];
    sort_rows(&mut entries, Sort::Remaining);
    assert_eq!(entries[0].index, 0);
    sort_rows(&mut entries, Sort::Level);
    assert_eq!(entries[0].index, 0);
}

#[test]
fn the_deposit_estimate_round_trips_through_its_selector_id() {
    for estimate in Estimate::ALL {
        assert_eq!(estimate_from_id(estimate_id(estimate)), Some(estimate));
    }
    assert_eq!(estimate_from_id("nope"), None);
    assert_eq!(Estimate::Average.label(), "Average with vial doubles");
}

#[test]
fn sort_ids_round_trip() {
    for sort in Sort::ALL {
        assert_eq!(Sort::from_id(sort.id()), Some(sort));
    }
    assert_eq!(Sort::from_id("nope"), None);
}

#[test]
fn availability_reads_the_core_label_and_tone() {
    let available = row(&sample(10, 0.0), 0);
    assert_eq!(
        available.availability(false),
        ("Available".to_string(), Tone::Success)
    );

    let mut rows = vec![
        Some(Progress {
            level: 10,
            exp: 0.0
        });
        statue::count()
    ];
    rows[5] = None;
    rows[1] = None;
    let save = StatueSave::new(vec![rows], vec!["main".into()]);
    assert_eq!(row(&save, 5).availability(false).0, "Missing row");
    assert_eq!(
        row(&save, 10).availability(false).0,
        format!("Needs {}", statue::info(1).expect("SPEED").name)
    );
    assert_eq!(
        row(&save, 5).availability(true),
        ("No save".to_string(), Tone::Neutral)
    );

    let mut unavailable = row(&save, 5);
    unavailable.next = Err(Unavailable::OutOfRange);
    assert_eq!(
        unavailable.availability(false),
        ("Out of range".to_string(), Tone::Danger)
    );
    unavailable.next = Err(Unavailable::TargetOutOfRange);
    assert_eq!(
        unavailable.availability(false),
        ("Out of range".to_string(), Tone::Danger)
    );
}
