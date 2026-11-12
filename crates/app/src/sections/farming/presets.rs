//! The named presets: save the current goal, baseline and crop target, then
//! apply or forget a saved one.
//!
//! Upstream's preset list is fixed — the page's seven `OBJECTIVES` goals drive
//! the Preset Comparison table. A named preset here keeps the same three
//! section inputs in the project document, so a project round trip restores it
//! and applying one is the same edit as picking them by hand.

use crate::sections::presets;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::forms::Input;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_farming::{Mode, Objective, data};

/// One named preset: the goal, the baseline and the crop target.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct CustomPreset {
    pub(crate) name: String,
    pub(crate) objective: Objective,
    pub(crate) mode: Mode,
    pub(crate) target_crop: u32,
}

impl Default for CustomPreset {
    fn default() -> Self {
        Self {
            name: String::new(),
            objective: Objective::default(),
            mode: Mode::default(),
            target_crop: 1,
        }
    }
}

impl CustomPreset {
    /// Save the current inputs under `name`, replacing a preset of the same
    /// name. Returns the status line the window shows.
    pub(crate) fn store(
        presets: &mut Vec<Self>,
        name: &str,
        objective: Objective,
        mode: Mode,
        target_crop: u32,
    ) -> Result<String, String> {
        let name = presets::clean_name(name);
        if name.is_empty() {
            return Err("Type a preset name before saving.".into());
        }
        let preset = Self {
            name: name.clone(),
            objective,
            mode,
            target_crop: clamp_crop(target_crop),
        };
        presets::upsert(presets, preset, |entry| entry.name == name, "presets")?;
        Ok(format!("Saved the farming preset “{name}”."))
    }

    /// Forget the preset at `index`.
    pub(crate) fn delete(presets: &mut Vec<Self>, index: usize) -> Option<Self> {
        presets::remove(presets, index)
    }

    /// Drop nameless and duplicate presets read from a document, clamp their
    /// crop target and keep the project's limit.
    pub(crate) fn sanitise(presets: Vec<Self>) -> Vec<Self> {
        let mut kept: Vec<Self> = Vec::new();
        for mut preset in presets {
            preset.name = presets::clean_name(&preset.name);
            if preset.name.is_empty() || kept.iter().any(|other| other.name == preset.name) {
                continue;
            }
            if kept.len() >= presets::MAX {
                break;
            }
            preset.target_crop = clamp_crop(preset.target_crop);
            kept.push(preset);
        }
        kept
    }

    /// A one-line description of what the preset sets.
    pub(crate) fn detail(&self) -> String {
        format!(
            "{} · {} · crop #{}",
            self.objective.label(),
            self.mode.label(),
            self.target_crop
        )
    }
}

/// The crop ids the target stepper accepts.
fn clamp_crop(crop: u32) -> u32 {
    let max = data().seeds.last().map(|seed| seed.end as u32).unwrap_or(1);
    crop.clamp(1, max)
}

/// The preset bar: the name field, Save, and one row per stored preset.
pub(super) fn presets_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let name_focus = state.farming_name.read(cx).focus().clone();
    let presets = state.farming.presets.clone();
    let entity = cx.entity();
    let save = entity.clone();

    let mut rows = div().id("farming-preset-list").flex().flex_col().gap_1();
    if presets.is_empty() {
        rows = rows.child(
            div()
                .id("farming-presets-empty")
                .role(Role::Label)
                .aria_label("No farming presets saved")
                .child(
                    Caption::new(
                        "No presets yet; name the current goal, baseline and crop target, then save them.",
                    )
                    .text_color(muted),
                ),
        );
    }
    for (index, preset) in presets.iter().enumerate() {
        let apply = entity.clone();
        let delete = entity.clone();
        let detail = preset.detail();
        rows = rows.child(
            div()
                .id(SharedString::from(format!("farming-preset-{index}")))
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
                        SharedString::from(format!("farming-preset-apply-{index}")),
                        "Apply",
                    )
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        apply.update(cx, |state, cx| state.apply_farming_preset(index, cx));
                    }),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("farming-preset-delete-{index}")),
                        "Delete",
                    )
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        delete.update(cx, |state, cx| state.delete_farming_preset(index, cx));
                    }),
                ),
        );
    }

    Card::new()
        .header(
            CardHeader::new("Presets")
                .description("Named goals kept in the project JSON"),
        )
        .child(
            div()
                .id("farming-presets")
                .role(Role::Group)
                .aria_label("Farming presets")
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
                                .id("farming-preset-name")
                                .role(Role::TextInput)
                                .aria_label("Preset name")
                                .w(px(240.0))
                                .on_click(move |_event, window, cx| window.focus(&name_focus, cx))
                                .child(Input::new(&state.farming_name).size(ControlSize::Sm)),
                        )
                        .child(
                            Button::new("farming-preset-save", "Save preset")
                                .size(ControlSize::Sm)
                                .on_click(move |_event, _window, cx| {
                                    save.update(cx, |state, cx| state.save_farming_preset(cx));
                                }),
                        ),
                )
                .child(rows)
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(
                            "A preset keeps the objective, the Land Rank baseline and the target crop. The seven upstream goals stay in the objective selector.",
                        ),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_preset_replaces_the_one_with_its_name() {
        let mut presets = Vec::new();
        CustomPreset::store(
            &mut presets,
            "  Beans run ",
            Objective::Beans,
            Mode::Reset,
            230,
        )
        .expect("saved");
        CustomPreset::store(
            &mut presets,
            "Beans run",
            Objective::Evolution,
            Mode::Unspent,
            240,
        )
        .expect("saved");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].name, "Beans run");
        assert_eq!(presets[0].objective, Objective::Evolution);
        assert_eq!(presets[0].mode, Mode::Unspent);
        assert_eq!(presets[0].target_crop, 240);
    }

    #[test]
    fn sanitise_drops_nameless_and_duplicate_presets() {
        let presets = CustomPreset::sanitise(vec![
            CustomPreset {
                name: "  ".into(),
                ..CustomPreset::default()
            },
            CustomPreset {
                name: "Minehead".into(),
                objective: Objective::Growth,
                target_crop: 900,
                ..CustomPreset::default()
            },
            CustomPreset {
                name: "Minehead".into(),
                objective: Objective::Beans,
                ..CustomPreset::default()
            },
        ]);
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].objective, Objective::Growth);
        assert_eq!(presets[0].target_crop, 329, "the crop target clamps");
    }
}
