//! Statue table search and sort.

use super::*;
use idleon_core::statue::Progress;

fn sample(level: u32, exp: f64) -> StatueSave {
    let mut rows = vec![Some(Progress { level, exp }); statue::count()];
    rows[0] = Some(Progress { level, exp });
    StatueSave::new(vec![rows], vec!["main".into()])
}

fn row(save: &StatueSave, index: usize) -> Row {
    Row {
        index,
        info: statue::info(index).expect("a statue"),
        progress: save.progress(0, index),
        next: statue::next_level(save, 0, index),
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
fn sort_ids_round_trip() {
    for sort in Sort::ALL {
        assert_eq!(Sort::from_id(sort.id()), Some(sort));
    }
    assert_eq!(Sort::from_id("nope"), None);
}
