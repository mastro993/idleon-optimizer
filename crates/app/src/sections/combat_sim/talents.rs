//! The talent half of the loadout editor.
//!
//! Ported from the upstream `renderTalents` pane (`js/ui/combat-sim-setup-ui.js`)
//! and `readTalents` in `combat-sim-setup.js`: every talent id the save stores a
//! level or a maximum for, its saved maximum, and a level control that stops at
//! that maximum. A save whose stored level is above its own maximum is flagged,
//! not silently clamped, so the editor shows what the save really says.

use crate::state::AppState;
use crate::ui::layout;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::{SearchInput, Stepper};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled,
    Window, div, px,
};

/// The most rows the table renders before the search has to narrow it.
const ROW_CAP: usize = 60;

/// The talents pane: the search field and one editable row per talent.
pub fn pane(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let query = state.combat.talent_query(cx);
    let muted = cx.theme().colors.fg_muted;
    let total = state.combat.loadout.talents.len();
    let needle = query.trim().to_lowercase();
    let matching: Vec<usize> = state
        .combat
        .loadout
        .talents
        .iter()
        .enumerate()
        .filter(|(_, talent)| {
            needle.is_empty()
                || talent.name.to_lowercase().contains(&needle)
                || talent.id.to_string() == needle
        })
        .map(|(index, _)| index)
        .take(ROW_CAP)
        .collect();
    let shown = matching.len();
    let entity = cx.entity();
    let search_focus = state.combat.talent_search.read(cx).focus().clone();

    let mut table = Table::new(["Talent", "Level", "Maximum"]);
    for index in &matching {
        let talent = &state.combat.loadout.talents[*index];
        let id = talent.id;
        let level = talent.level;
        let max = talent.max;
        let over = level > max;
        let edit = entity.clone();
        table = table.row([
            div()
                .id(SharedString::from(format!("combat-talent-{id}")))
                .role(Role::Label)
                .aria_label(format!("{}: {} of {}", talent.name, level, max))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(talent.name.clone()),
                )
                .when(over, |row| {
                    row.child(Badge::new(format!("above maximum {max}")).tone(Tone::Danger))
                })
                .into_any_element(),
            Stepper::new(
                SharedString::from(format!("combat-talent-{id}")),
                f64::from(level),
            )
            .range(0.0, f64::from(max.max(level)))
            .step(1.0)
            .on_change(move |value, _window, cx| {
                edit.update(cx, |state, cx| {
                    state.set_combat_talent(id, value.max(0.0).round() as u32, cx);
                });
            })
            .into_any_element(),
            div()
                .text_size(px(12.0))
                .text_color(muted)
                .child(max.to_string())
                .into_any_element(),
        ]);
    }

    let body = if total == 0 {
        EmptyState::new(
            "combat-talents-empty",
            IconName::SearchX,
            "No talents in this save",
        )
        .body("The save carries no SL_<n> or SM_<n> table for this character.")
        .into_any_element()
    } else if matching.is_empty() {
        EmptyState::new(
            "combat-talents-no-match",
            IconName::SearchX,
            "No talent matches your search",
        )
        .into_any_element()
    } else {
        ScrollArea::new("combat-talent-scroll")
            .vertical()
            .h(layout::table_height(f32::from(
                window.viewport_size().height,
            )))
            .child(table)
            .into_any_element()
    };

    Card::new()
        .header(
            CardHeader::new("Talents")
                .description(format!(
                    "{total} talent(s) in this save; the level stops at the saved maximum"
                ))
                .action(
                    div()
                        .id("combat-talent-count")
                        .role(Role::Label)
                        .aria_label(format!("{shown} of {total} talents shown"))
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(format!("{shown} / {total} shown")),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .min_h_0()
                .p_3()
                .child(
                    div()
                        .id("combat-talent-search")
                        .role(Role::Search)
                        .aria_label("Find a talent")
                        .w(px(280.0))
                        .on_click(move |_event, window, cx| window.focus(&search_focus, cx))
                        .child(SearchInput::new(
                            "combat-talent-search-field",
                            &state.combat.talent_search,
                        )),
                )
                .child(body)
                .when(shown == ROW_CAP && total > ROW_CAP, |panel| {
                    panel.child(Caption::new(format!(
                        "Showing the first {ROW_CAP} talents; search to reach the rest."
                    )))
                }),
        )
        .into_any_element()
}
