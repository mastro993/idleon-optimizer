//! The Jelly Operator save setups the `jelly` test binaries share.
//!
//! Every setup starts from `fixtures/data.json` and applies a documented
//! override, so a test can reach a small board, a cleared stage or a board with
//! its sections bought without a purpose-built save.
//!
//! Only some helpers apply to each binary, so the module allows dead code.
#![allow(dead_code)]

use idleon_import::import;
use idleon_jelly::Jelly;
use idleon_jelly::board::Layout;

/// The committed save's Jelly Operator, untouched.
pub fn save() -> Jelly {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("fixtures/data.json is committed");
    import(&fixture)
        .expect("the fixture imports")
        .jelly
        .expect("the fixture carries a jelly board")
}

/// The committed save with most of its board sections bought, so a search has
/// room to improve on the saved board.
pub fn bought_sections() -> Jelly {
    let mut jelly = save();
    jelly.plots = vec![
        0, 2, 3, 4, 5, 7, 8, 9, 10, 13, 14, 15, 16, 17, 18, 19, 20, 22, 23, 24, 25, 26, 27, 28, 30,
        31, 32, 33, 34, 35, 37, 38,
    ];
    jelly
}

/// An early account: eight cell types on a small board at the last stage.
pub fn small_board() -> Jelly {
    let mut jelly = save();
    set(
        &mut jelly,
        1,
        32,
        &[
            (0, 6),
            (4, 2),
            (59, 1),
            (65, 0),
            (67, 5),
            (72, 4),
            (84, 3),
            (94, 7),
        ],
        &[
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 1),
            (5, 1),
            (6, 1),
            (7, 1),
            (10, 1),
            (12, 1),
            (13, 2),
            (14, 1),
            (15, 1),
            (16, 2),
            (17, 3),
            (18, 5),
            (23, 2),
            (28, 1),
            (29, 10),
            (35, 2),
            (36, 1),
            (38, 2),
        ],
        [3, 2, 1, 0, 0, 0, 0, 0, 0],
        [10.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    );
    jelly
}

/// A fresh account at the first stage with two cells on the board.
pub fn clear() -> Jelly {
    let mut jelly = save();
    set(
        &mut jelly,
        2,
        0,
        &[(77, 1), (95, 1)],
        &[(0, 1), (1, 1), (12, 1), (16, 3), (28, 1), (36, 1)],
        [0; 9],
        [0.0; 9],
    );
    jelly
}

fn set(
    jelly: &mut Jelly,
    fever: u32,
    obstruction: usize,
    layout: &[(usize, usize)],
    upgrades: &[(usize, u32)],
    cell_level: [u32; 9],
    cell_exp: [f64; 9],
) {
    jelly.progress.fever = fever;
    jelly.progress.obstruction = obstruction;
    jelly.layout = layout.iter().copied().collect::<Layout>();
    for &(id, level) in upgrades {
        if jelly.upgrades.len() <= id {
            jelly.upgrades.resize(id + 1, 0);
        }
        jelly.upgrades[id] = level;
    }
    jelly.cell_level = cell_level;
    jelly.cell_exp = cell_exp;
}

pub fn close(actual: f64, expected: f64, what: &str) {
    let tolerance = 1e-9_f64.max(expected.abs() * 1e-9);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} != {expected}"
    );
}
