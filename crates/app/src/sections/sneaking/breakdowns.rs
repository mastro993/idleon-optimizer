//! The Sneaking breakdowns workspace: the twin selector, the five formula
//! tabs and the shared tree.
//!
//! The workspace builds the Stealth, Jade, EXP, Door and Runtime trees for one
//! twin, and the shared [`FormulaTree`] supplies the search, expand/collapse
//! and copy controls.

use super::controls::Figures;
use super::state::FormulaTab;
use crate::state::AppState;
use crate::ui::layout::tab_bar;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::select::Select;
use idleon_sneaking::Loadout;
use idleon_ui::widgets::{Caption, Card, CardHeader};

/// The breakdowns workspace. The tree's root is set before render, when the
/// loadout, the twin or the tab changes.
pub(crate) fn pane(
    state: &AppState,
    loadout: &Loadout,
    figures: &Figures,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let twin = state.sneaking_state.breakdown_twin(loadout.players());
    let tab = state.sneaking_state.formula_tab;
    let selected = loadout
        .twin(twin)
        .map_or_else(|| "the first twin".to_string(), |entry| entry.name.clone());
    let picker = &state.sneaking_controls.breakdown_twin;
    picker.show(figures.twin_choices.clone(), Some(&twin), window, cx);
    let select = div().flex_none().w(px(260.0)).h_6().child(
        Select::new(picker.state())
            .id("sneak-breakdown-twin")
            .accessibility_label("Twin")
            .small(),
    );

    let entity = cx.entity();
    let tabs = tab_bar(
        "sneak-formula-tabs",
        FormulaTab::ALL.map(|entry| {
            (
                format!("sneak-formula-{}", entry.id()).into(),
                entry.label().into(),
            )
        }),
        FormulaTab::ALL
            .iter()
            .position(|entry| *entry == tab)
            .unwrap_or_default(),
        move |index, _window, cx| {
            entity.update(cx, |state, cx| {
                state.sneak_formula_tab(FormulaTab::ALL[index], cx)
            });
        },
    );

    let description = match tab {
        FormulaTab::Stealth => {
            "Base stealth times the charm, account, ally and funeral multipliers."
        }
        FormulaTab::Jade => {
            "Floor base Jade times every Jade multiplier, over the display actions per hour."
        }
        FormulaTab::Exp => {
            "Floor base EXP times the account, charm and talent factors of one successful action."
        }
        FormulaTab::Door => {
            "Nunchaku damage per hit, the door's hit chance and the runtime hits per hour."
        }
        FormulaTab::Runtime => {
            "The native action loop's attempts, successes, flowers, Jade, items, door damage and \
             untie progress."
        }
    };

    div()
        .id("sneaking-breakdowns")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(select)
                .child(tabs)
                .child(div().flex_1())
                .child(Caption::new(format!("{selected} · {description}"))),
        )
        .child(
            Card::new()
                .header(
                    CardHeader::new(format!("{} breakdown", tab.label())).description(description),
                )
                .child(
                    div()
                        .h(px(560.0))
                        .min_h_0()
                        .child(state.sneaking_breakdown.clone()),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_formula_tab_has_a_label_and_its_own_id() {
        let ids: std::collections::BTreeSet<_> = FormulaTab::ALL.map(FormulaTab::id).into();
        assert_eq!(ids.len(), FormulaTab::ALL.len());
        assert!(FormulaTab::ALL.iter().all(|tab| !tab.label().is_empty()));
    }
}
