//! The named loadouts: the bar that saves, applies and deletes presets.
//!
//! Upstream keeps named setups in browser storage (`js/ui/combat-sim-loadouts.js`,
//! `combat-sim-loadout:<ci>:<name>`). This build keeps them in the project JSON
//! instead, so an export carries the loadouts and an import restores them. A
//! preset holds item keys and talent levels; applying one is the same edit as
//! equipping each slot by hand.

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::forms::Input;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_core::combat::{Loadout, Preset};

/// The named loadouts: what the project JSON keeps and the bar edits.
#[derive(Clone, Debug, Default)]
pub struct Presets {
    pub items: Vec<Preset>,
}

impl Presets {
    /// How many loadouts one project keeps.
    pub const MAX: usize = 40;
    /// How long a loadout name may be.
    const NAME_MAX: usize = 60;

    /// Save `loadout` under `name`, replacing a loadout of the same name for
    /// the same character, the way upstream's storage key does. Returns the
    /// status line the window shows.
    pub fn store(&mut self, loadout: &Loadout, name: &str) -> Result<String, String> {
        let name: String = name.trim().chars().take(Self::NAME_MAX).collect();
        if name.is_empty() {
            return Err("Type a loadout name before saving.".into());
        }
        let preset = loadout.preset(&name);
        let at_limit = self.items.len() >= Self::MAX;
        let existing = self
            .items
            .iter_mut()
            .find(|entry| entry.name == preset.name && entry.character == preset.character);
        match existing {
            Some(existing) => *existing = preset,
            None if at_limit => {
                return Err(format!(
                    "{} loadouts is the limit; delete one before saving another.",
                    Self::MAX
                ));
            }
            None => {
                self.items.push(preset);
                self.items.sort_by(|left, right| {
                    left.name
                        .cmp(&right.name)
                        .then(left.character.cmp(&right.character))
                });
            }
        }
        Ok(format!("Saved the loadout “{name}”."))
    }

    /// Apply the loadout at `index` to its character's working loadout.
    pub fn apply(&self, index: usize, loadout: &mut Loadout) -> String {
        let Some(preset) = self.items.get(index) else {
            return "No loadout at that position.".into();
        };
        match preset.apply_to(loadout) {
            Ok(()) => format!(
                "Applied “{}”: {} change(s) vs the save.",
                preset.name,
                loadout.changed()
            ),
            Err(error) => format!("Cannot apply “{}”: {error}.", preset.name),
        }
    }

    /// Forget one named loadout.
    pub fn delete(&mut self, index: usize) -> Option<Preset> {
        (index < self.items.len()).then(|| self.items.remove(index))
    }
}

/// The preset bar: the name field, Save, and one row per stored loadout.
pub fn bar(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let name_focus = state.combat.name.read(cx).focus().clone();
    let presets = state.combat.presets.items.clone();
    let character = state.combat.character;
    let entity = cx.entity();
    let save = entity.clone();

    let mut rows = div().id("combat-preset-list").flex().flex_col().gap_1();
    if presets.is_empty() {
        rows = rows.child(
            div()
                .id("combat-presets-empty")
                .role(Role::Label)
                .aria_label("No presets saved")
                .child(
                    Caption::new("No presets yet; name the current loadout and save it.")
                        .text_color(muted),
                ),
        );
    }
    for (index, preset) in presets.iter().enumerate() {
        let load = entity.clone();
        let delete = entity.clone();
        let items = preset
            .items
            .iter()
            .filter(|key| !idleon_core::combat::data::is_empty(key))
            .count();
        let detail = format!(
            "{items} item(s) · {} talent(s){}",
            preset.talents.len(),
            if preset.character == character {
                String::new()
            } else {
                format!(" · character {}", preset.character + 1)
            }
        );
        rows = rows.child(
            div()
                .id(SharedString::from(format!("combat-preset-{index}")))
                .role(Role::Label)
                .aria_label(format!("{}: {detail}", preset.name))
                .flex()
                .items_center()
                .gap_2()
                .h(px(26.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(12.0))
                        .child(preset.name.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(detail),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("combat-preset-load-{index}")),
                        "Load",
                    )
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        load.update(cx, |state, cx| state.load_combat_preset(index, cx));
                    }),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("combat-preset-delete-{index}")),
                        "Delete",
                    )
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        delete.update(cx, |state, cx| state.delete_combat_preset(index, cx));
                    }),
                ),
        );
    }

    Card::new()
        .header(
            CardHeader::new("Loadout presets")
                .description("Named loadouts kept in the project JSON"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_2()
                        .child(
                            div()
                                .id("combat-preset-name")
                                .role(Role::TextInput)
                                .aria_label("Loadout name")
                                .w(px(240.0))
                                .on_click(move |_event, window, cx| window.focus(&name_focus, cx))
                                .child(Input::new(&state.combat.name).size(ControlSize::Sm)),
                        )
                        .child(
                            Button::new("combat-preset-save", "Save preset")
                                .size(ControlSize::Sm)
                                .on_click(move |_event, _window, cx| {
                                    save.update(cx, |state, cx| state.save_combat_preset(cx));
                                }),
                        ),
                )
                .child(rows),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_core::save::Save;
    use serde_json::json;

    fn save() -> Save {
        Save::new(json!({
            "charNames": ["Ana", "Bo"],
            "Lv0_0": "[120]",
            "CharacterClass_0": 7,
            "EquipOrder_0": [["EquipmentHats1", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""], ["", "", "", "", "", "", ""], []],
            "SL_0": { "0": 5, "3": 2 },
            "SM_0": { "0": 20, "3": 10 },
            "Lv0_1": "[10]",
            "EquipOrder_1": [[], [], []],
        }))
    }

    #[test]
    fn a_preset_saves_applies_and_deletes() {
        let save = save();
        let mut loadout = Loadout::from_save(&save, 0);
        let mut presets = Presets::default();
        loadout.set_item(0, 1, "EquipmentSword1");
        let saved = presets
            .store(&loadout, "  Bossing  ")
            .expect("a name saves");
        assert!(saved.contains("Bossing"), "{saved}");
        assert_eq!(presets.items.len(), 1);
        assert_eq!(presets.items[0].name, "Bossing");
        assert_eq!(presets.items[0].character, 0);

        loadout.set_item(0, 1, "Blank");
        let applied = presets.apply(0, &mut loadout);
        assert!(applied.contains("Applied"), "{applied}");
        assert_eq!(loadout.slot(0, 1).expect("weapon").item, "EquipmentSword1");
        assert_eq!(
            presets.apply(9, &mut loadout),
            "No loadout at that position."
        );

        let removed = presets.delete(0).expect("the preset deletes");
        assert_eq!(removed.name, "Bossing");
        assert!(presets.items.is_empty());
        assert!(presets.delete(0).is_none());
    }

    #[test]
    fn the_same_name_replaces_instead_of_duplicating() {
        let save = save();
        let mut loadout = Loadout::from_save(&save, 0);
        let mut presets = Presets::default();
        presets.store(&loadout, "Bossing").expect("saves");
        loadout.set_item(0, 1, "EquipmentSword1");
        presets.store(&loadout, "Bossing").expect("replaces");
        assert_eq!(presets.items.len(), 1);
        assert_eq!(presets.items[0].items[1], "EquipmentSword1");
        presets.store(&loadout, "Farming").expect("saves");
        assert_eq!(
            presets
                .items
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Bossing", "Farming"]
        );
    }

    #[test]
    fn an_empty_name_and_a_full_list_are_refused() {
        let save = save();
        let loadout = Loadout::from_save(&save, 0);
        let mut presets = Presets::default();
        let refused = presets.store(&loadout, "   ").expect_err("no name");
        assert!(refused.contains("name"), "{refused}");
        for index in 0..Presets::MAX {
            presets
                .store(&loadout, &format!("Loadout {index}"))
                .expect("under the limit");
        }
        let full = presets
            .store(&loadout, "One more")
            .expect_err("the list is full");
        assert!(full.contains("limit"), "{full}");
        assert_eq!(presets.items.len(), Presets::MAX);
    }

    #[test]
    fn a_preset_from_another_character_is_refused() {
        let save = save();
        let mut other = Loadout::from_save(&save, 1);
        let mut presets = Presets::default();
        presets.store(&other, "Bo's build").expect("saves");
        let mut loadout = Loadout::from_save(&save, 0);
        let refused = presets.apply(0, &mut loadout);
        assert!(refused.contains("Cannot apply"), "{refused}");
        assert!(refused.contains("character 2"), "{refused}");
        // The working loadout is untouched.
        assert_eq!(loadout.slot(0, 0).expect("helmet").item, "EquipmentHats1");
        other.set_item(0, 0, "Blank");
    }
}
