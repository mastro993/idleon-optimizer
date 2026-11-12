//! The Sneaking breakdowns workspace: the twin selector, the five formula
//! tabs and the shared tree.
//!
//! Ported from the `breakdowns` workspace panel in `sneaking-calc.html`:
//! `_renderFormulaBreakdowns` builds the Stealth, Jade, EXP, Door and Runtime
//! trees for one twin, and the shared [`FormulaTree`] supplies the search,
//! expand/collapse and copy controls.

use super::state::FormulaTab;
use crate::state::AppState;
use crate::ui::formula::FormulaTree;
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, px,
};
use idleon_core::sneaking::breakdown;

/// The breakdowns workspace.
pub fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(loadout) = state.sneaking.as_ref() else {
        return div().into_any_element();
    };
    let players = loadout.players();
    let twin = state.sneaking_state.breakdown_twin(players);
    let tab = state.sneaking_state.formula_tab;
    let root = match tab {
        FormulaTab::Stealth => breakdown::stealth(loadout, twin),
        FormulaTab::Jade => breakdown::jade(loadout, twin),
        FormulaTab::Exp => breakdown::exp(loadout, twin),
        FormulaTab::Door => breakdown::door(loadout, twin),
        FormulaTab::Runtime => breakdown::runtime(loadout, twin, 1.0),
    };
    let selected = loadout
        .twin(twin)
        .map_or_else(|| "the first twin".to_string(), |entry| entry.name.clone());
    let choices: Vec<Choice> = loadout
        .twins()
        .iter()
        .take(players)
        .map(|entry| {
            Choice::new(entry.index.to_string(), entry.name.clone())
                .note(format!("Lv {} · floor {}", entry.level, entry.floor))
        })
        .collect();
    let tree: Entity<FormulaTree> = state.sneaking_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(root, cx));

    let colors = cx.theme().colors.clone();
    let entity = cx.entity();
    let select = Select::new("sneak-breakdown-twin", choices)
        .label("Twin")
        .selected(twin.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.sneak_breakdown_twin(index, cx));
            }
        });

    let entity = cx.entity();
    let mut tabs = TabBar::new("sneak-formula-tabs")
        .selected(SharedString::from(format!("sneak-formula-{}", tab.id())));
    for entry in FormulaTab::ALL {
        tabs = tabs.tab(WindowTab::new(
            format!("sneak-formula-{}", entry.id()),
            entry.label(),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        let id = id.as_ref();
        if let Some(tab) = id
            .strip_prefix("sneak-formula-")
            .and_then(FormulaTab::from_id)
        {
            entity.update(cx, |state, cx| state.sneak_formula_tab(tab, cx));
        }
    });

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
                .child(
                    Caption::new(format!("{selected} · {description}")).text_color(colors.fg_muted),
                ),
        )
        .child(
            Card::new()
                .header(
                    CardHeader::new(format!("{} breakdown", tab.label())).description(description),
                )
                .child(div().h(px(560.0)).min_h_0().child(tree)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_formula_tab_has_a_label_and_an_id() {
        for tab in FormulaTab::ALL {
            assert!(!tab.label().is_empty());
            assert_eq!(FormulaTab::from_id(tab.id()), Some(tab));
        }
        assert_eq!(FormulaTab::from_id("missing"), None);
    }
}
