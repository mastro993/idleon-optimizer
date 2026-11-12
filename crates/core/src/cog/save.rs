//! The cog board and shelf a save carries.
//!
//! Ported from `boardFromSave`, `extractAllCogs` and `recoverRawPlayerStats`
//! in `js/cogs/cog-board.js`. `CogO` is the 252-slot name list (`0..96` board,
//! `96..108` player slots, `108..228` shelf, `228..252` tiny board) and `CogM`
//! is the stat row per slot. The board keeps only the 96 board slots; the shelf
//! keeps the 120 shelf slots and the non-tiny cogs among them.
//!
//! The game stores a player cog's `b` *after* its board bonuses. The optimizer
//! scores from raw stats, so [`CogSave::from_save`] divides those bonuses out
//! once, as upstream does, and every later placement is scored from raw.

use serde_json::{Map, Value};

use super::{
    BOARD_SIZE, Board, Cog, Direction, compute_totals, empty_board, infer_construction_level,
    stat_tier,
};
use crate::save::json;

/// The first shelf slot.
const SHELF_START: usize = 108;
/// The slot after the shelf.
const SHELF_END: usize = 228;
/// The shelf slot count.
const SHELF_SLOTS: usize = SHELF_END - SHELF_START;

/// The board, the shelf and the construction level the save carries.
#[derive(Clone, Debug, PartialEq)]
pub struct CogSave {
    /// The 96-slot save board, with player base stats recovered.
    pub board: Board,
    /// Every non-small shelf cog, the optimizer's pool.
    pub shelf: Vec<Cog>,
    /// The 120 shelf slots by position, blanks included, for the shelf view.
    pub shelf_slots: Vec<Option<Cog>>,
    /// The highest saved Construction level (`Lv0_<n>[8]`), or the upstream
    /// fallback of 100 when the save carries none.
    pub construction_level: u32,
    /// The effective level the board's own rolls reveal, from `findMaxConLevel`:
    /// talents, stamps and star signs lift it above the saved level, and a roll
    /// that could only have happened at a higher level raises it. Roll ranges
    /// and roll targets use this one.
    pub max_construction_level: u32,
    /// Whether the save carried `CogO` at all, even if every slot is Blank.
    pub loaded: bool,
}

impl Default for CogSave {
    fn default() -> Self {
        Self {
            board: empty_board(),
            shelf: Vec::new(),
            shelf_slots: vec![None; SHELF_SLOTS],
            construction_level: 100,
            max_construction_level: 100,
            loaded: false,
        }
    }
}

impl CogSave {
    /// Read the board and shelf a save carries. A save without `CogO` keeps an
    /// empty board and reports the field once; `CogM` without rows leaves the
    /// named cogs at their zero stats.
    pub fn from_save(
        save: &Map<String, Value>,
        construction_level: u32,
        warnings: &mut Vec<String>,
    ) -> Self {
        let order = json::parsed_field(save, "CogO").and_then(|value| match value {
            Value::Array(names) => Some(names),
            _ => None,
        });
        let stats = json::parsed_field(save, "CogM").and_then(|value| match value {
            Value::Object(rows) => Some(rows),
            _ => None,
        });
        if order.is_none() {
            warnings.push("CogO: not in the save; the cog board stays empty".into());
            return Self {
                construction_level,
                max_construction_level: construction_level,
                ..Self::default()
            };
        }
        if stats.is_none() {
            warnings.push("CogM: not in the save; every cog keeps zero stats".into());
        }
        let order = order.unwrap_or_default();
        let empty = Map::new();
        let stats = stats.as_ref().unwrap_or(&empty);
        let name = |slot: usize| {
            order
                .get(slot)
                .and_then(Value::as_str)
                .unwrap_or("Blank")
                .to_string()
        };
        let row = |slot: usize| stats.get(&slot.to_string());

        let mut board = empty_board();
        for (slot, entry) in board.iter_mut().enumerate() {
            let name = name(slot);
            if name != "Blank" {
                *entry = Some(cog(&name, slot, row(slot)));
            }
        }
        recover_raw_player_stats(&mut board);

        let mut shelf_slots = vec![None; SHELF_SLOTS];
        let mut shelf = Vec::new();
        for slot in SHELF_START..SHELF_END {
            let name = name(slot);
            if name == "Blank" {
                continue;
            }
            let cog = cog(&name, slot, row(slot));
            if !name.starts_with("CogSm") {
                shelf.push(cog.clone());
            }
            shelf_slots[slot - SHELF_START] = Some(cog);
        }

        let max_construction_level =
            effective_construction_level(construction_level, &board, &shelf_slots);
        Self {
            board,
            shelf,
            shelf_slots,
            construction_level,
            max_construction_level,
            loaded: true,
        }
    }

    /// Whether the save carried a board at all, even an empty one.
    pub fn has_board(&self) -> bool {
        self.loaded
    }
}

/// One `CogM` row as a cog. A missing row is a cog with zero stats, which is
/// what upstream's `boardFromSave` builds.
fn cog(name: &str, slot: usize, row: Option<&Value>) -> Cog {
    let mut cog = Cog::named(name, slot);
    let Some(row) = row.and_then(Value::as_object) else {
        return cog;
    };
    let number = |key: &str| row.get(key).and_then(json::number).unwrap_or(0.0);
    cog.a = number("a");
    cog.b = number("b");
    cog.c = number("c");
    cog.d = number("d");
    cog.e = number("e");
    cog.f = number("f");
    cog.g = number("g");
    cog.j = number("j");
    cog.i = number("i");
    cog.direction = row
        .get("h")
        .and_then(Value::as_str)
        .and_then(Direction::from_name);
    cog.yin = number("l") != 0.0;
    cog
}

/// Divide each player's stored `b` by the bonuses its board position already
/// applied, so later placements score from raw stats. Ported from
/// `recoverRawPlayerStats`.
fn recover_raw_player_stats(board: &mut Board) {
    let totals = compute_totals(board);
    for slot in 0..BOARD_SIZE {
        let Some(cog) = board.get_mut(slot).and_then(Option::as_mut) else {
            continue;
        };
        if !cog.is_player || cog.b <= 0.0 {
            continue;
        }
        let surround = totals
            .per_slot
            .get(slot)
            .map_or(0.0, |slot| slot.surround_con_exp);
        let divisor = (1.0 + surround / 100.0) * (1.0 + totals.total_con_exp / 100.0);
        if divisor > 1.0 {
            cog.b /= divisor;
        }
    }
}

/// `findMaxConLevel`: the saved level, raised when a cog's own `d` could only
/// have rolled at a higher level. The player and tiny-board slots carry no stat
/// tier, so the board and shelf the section decodes hold every cog that counts.
fn effective_construction_level(
    construction_level: u32,
    board: &Board,
    shelf_slots: &[Option<Cog>],
) -> u32 {
    let mut max = construction_level;
    for cog in board.iter().chain(shelf_slots.iter()).flatten() {
        let Some(tier) = stat_tier(&cog.name) else {
            continue;
        };
        if cog.d <= 0.0 {
            continue;
        }
        max = max.max(infer_construction_level(cog.d, tier));
    }
    max
}

/// Whether a save carries cog fields, so a cog-only document still imports.
pub(crate) fn has_cog(save: &Map<String, Value>) -> bool {
    save.contains_key("CogO") || save.contains_key("CogM")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn save() -> Map<String, Value> {
        let mut order = vec!["Blank".to_string(); 252];
        order[0] = "Player_Ana".into();
        order[1] = "Cog3ri".into();
        order[108] = "Cog3up".into();
        order[109] = "CogSmb4".into();
        let stats = json!({
            "0": { "b": 120.0, "d": 20.0 },
            "1": { "f": 10.0, "h": "right" },
            "108": { "a": 50.0 },
        });
        let mut save = Map::new();
        save.insert("CogO".into(), json!(order));
        save.insert("CogM".into(), stats);
        save
    }

    #[test]
    fn the_board_reads_its_names_and_stats() {
        let mut warnings = Vec::new();
        let cog_save = CogSave::from_save(&save(), 42, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(cog_save.has_board());
        assert_eq!(cog_save.construction_level, 42);
        let player = cog_save.board[0].as_ref().expect("the player");
        assert_eq!(player.name, "Player_Ana");
        assert!(player.is_player);
        // The saved b already carries the board's totalD multiplier: 120 is a
        // raw 100 at +20% construction EXP from the player's own d.
        assert_eq!(player.b, 100.0);
        let directional = cog_save.board[1].as_ref().expect("the directional");
        assert_eq!(directional.direction, Some(Direction::Right));
        assert_eq!(directional.f, 10.0);
    }

    #[test]
    fn the_player_bonus_is_divided_out_of_the_saved_b() {
        let mut save = save();
        // Move the directional left of the player so its +10% reaches it.
        save.insert(
            "CogM".into(),
            json!({
                "0": { "b": 132.0, "d": 20.0 },
                "1": { "f": 10.0, "h": "right" },
            }),
        );
        let mut warnings = Vec::new();
        let cog_save = CogSave::from_save(&save, 42, &mut warnings);
        // The Cog3ri at slot 1 points right and reaches the player at slot 0?
        // No: slot 1's right is slot 2, so the player's saved 132 is its raw
        // 100 times the board's totalD multiplier (1.2), not a surround.
        let player = cog_save.board[0].as_ref().expect("the player");
        assert!((player.b - 110.0).abs() < 1e-9, "b = {}", player.b);
    }

    #[test]
    fn the_shelf_keeps_its_position_and_excludes_small_cogs() {
        let mut warnings = Vec::new();
        let cog_save = CogSave::from_save(&save(), 100, &mut warnings);
        assert_eq!(cog_save.shelf.len(), 1);
        assert_eq!(cog_save.shelf[0].name, "Cog3up");
        assert_eq!(cog_save.shelf_slots.len(), SHELF_SLOTS);
        assert_eq!(
            cog_save.shelf_slots[0]
                .as_ref()
                .map(|cog| cog.name.as_str()),
            Some("Cog3up")
        );
        assert_eq!(
            cog_save.shelf_slots[1]
                .as_ref()
                .map(|cog| cog.name.as_str()),
            Some("CogSmb4")
        );
    }

    #[test]
    fn a_save_without_cog_order_warns_and_stays_empty() {
        let mut warnings = Vec::new();
        let cog_save = CogSave::from_save(&Map::new(), 100, &mut warnings);
        assert!(!cog_save.has_board());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].starts_with("CogO:"));
    }

    #[test]
    fn a_small_cog_is_recognised_from_the_shelf() {
        let mut warnings = Vec::new();
        let cog_save = CogSave::from_save(&save(), 100, &mut warnings);
        let small = cog_save.shelf_slots[1].as_ref().expect("the small cog");
        assert_eq!(
            super::super::classify(&small.name),
            super::super::CogKind::Small { kind: 2, level: 4 }
        );
    }
}
