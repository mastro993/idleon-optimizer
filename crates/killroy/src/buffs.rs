//! The saved buffs a Killroy run starts with.
//!
//! `initialBuffs` in `js/stats/systems/common/combat-sim-inputs.js` reads the
//! save's `BuffsActive_<ci>` rows as `[id, stacks, time ?? stacks]`. A run starts
//! as a fresh map entry, so upstream drops every timed buff and keeps only the
//! permanent form toggles (`_withStartBuffs`: Wraith 195, Tempest 420, Arcanist
//! 585) unless the calculator's `useSavedBuffs` option is on.

use super::number;
use idleon_save::Save;
use idleon_save::at;
use idleon_save::row_values;
use serde::Serialize;

/// The buff ids that are permanent form toggles rather than timed buffs.
pub const FORM_TOGGLES: [usize; 3] = [195, 420, 585];

/// One saved `BuffsActive_<ci>` row and whether a run starts with it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedBuff {
    pub id: usize,
    /// The row's stack count; `None` when the save's cell is not a number.
    pub stacks: Option<f64>,
    /// The remaining timer, defaulting to the stack count as upstream does.
    pub time: Option<f64>,
    /// Whether the buff is one of the permanent form toggles.
    pub form: bool,
    /// Whether the run starts with this buff.
    pub kept: bool,
}

/// One saved character and the buffs a Killroy run starts with.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterBuffs {
    pub ci: usize,
    pub name: String,
    pub buffs: Vec<SavedBuff>,
}

/// The buffs the save has active for one character, matching `BuffsActive_<ci>`.
///
/// Rows without a whole-number id are not buffs and are left out; a saved row is
/// otherwise reported as it is stored, never completed with guessed values.
pub fn saved_buffs(save: &Save, ci: usize, keep: bool) -> Vec<SavedBuff> {
    let Some(buffs) = save.get(&format!("BuffsActive_{ci}")) else {
        return Vec::new();
    };
    row_values(&buffs)
        .iter()
        .filter_map(|row| {
            let id = at(row, 0).and_then(number)?;
            if id < 0.0 || id.fract() != 0.0 {
                return None;
            }
            let stacks = at(row, 1).and_then(number);
            let form = FORM_TOGGLES.contains(&(id as usize));
            Some(SavedBuff {
                id: id as usize,
                stacks,
                time: at(row, 2).and_then(number).or(stacks),
                form,
                kept: keep || form,
            })
        })
        .collect()
}

/// The buffs of each character index in `cis`, sorted and deduplicated.
pub fn character_buffs(save: &Save, cis: &[usize], keep: bool) -> Vec<CharacterBuffs> {
    let mut unique = cis.to_vec();
    unique.sort_unstable();
    unique.dedup();
    unique
        .into_iter()
        .map(|ci| CharacterBuffs {
            ci,
            name: save.character_name(ci),
            buffs: saved_buffs(save, ci, keep),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_save::Save;
    use serde_json::json;

    fn save() -> Save {
        Save::new(json!({
            "BuffsActive_0": [
                { "0": 460, "1": 1891, "2": 3360, "length": 3 },
                { "0": 420, "1": 5, "2": 5, "length": 3 },
                { "0": 195, "1": 2, "length": 2 },
            ],
            "BuffsActive_1": [],
        }))
    }

    #[test]
    fn the_saved_rows_match_the_save() {
        let buffs = saved_buffs(&save(), 0, false);
        assert_eq!(
            buffs,
            vec![
                SavedBuff {
                    id: 460,
                    stacks: Some(1891.0),
                    time: Some(3360.0),
                    form: false,
                    kept: false,
                },
                SavedBuff {
                    id: 420,
                    stacks: Some(5.0),
                    time: Some(5.0),
                    form: true,
                    kept: true,
                },
                SavedBuff {
                    id: 195,
                    stacks: Some(2.0),
                    time: Some(2.0),
                    form: true,
                    kept: true,
                },
            ]
        );
    }

    #[test]
    fn keeping_the_saved_buffs_keeps_the_timed_ones_too() {
        let buffs = saved_buffs(&save(), 0, true);
        assert_eq!(buffs.len(), 3);
        assert!(buffs.iter().all(|buff| buff.kept));
        assert!(!buffs[0].form);
    }

    #[test]
    fn a_row_without_a_timer_falls_back_to_its_stacks() {
        let buffs = saved_buffs(&save(), 0, false);
        let form = buffs[2];
        assert_eq!(form.time, Some(2.0));
        assert_eq!(buffs[1].time, Some(5.0));
    }

    #[test]
    fn a_save_without_buffs_reports_none() {
        assert!(saved_buffs(&save(), 1, true).is_empty());
        assert!(saved_buffs(&save(), 7, true).is_empty());
        assert!(saved_buffs(&Save::new(json!({})), 0, true).is_empty());
    }

    #[test]
    fn characters_deduplicate_and_sort_by_index() {
        let characters = character_buffs(&save(), &[1, 0, 1], false);
        let indices: Vec<usize> = characters.iter().map(|character| character.ci).collect();
        assert_eq!(indices, vec![0, 1]);
        assert_eq!(characters[0].buffs.len(), 3);
        assert!(characters[1].buffs.is_empty());
    }
}
