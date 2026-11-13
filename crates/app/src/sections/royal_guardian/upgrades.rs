//! The Upgrades tab: every Armory shelf, every Orblet and the cost plan for the
//! next levels of one shelf.
//!
//! The upstream page splits this into Armory, Statues, Flair and Orblets. The
//! Royal statues and the statue flair are the Statue section's own tables, so
//! this pane carries the two upgrades the Royal Guardian owns.

use super::State;
use super::overview::{figure_text, kpi, number_cell};
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, div};
use idleon_royal_guardian::{ArmoryRow, PROJECTION_LEVELS, RoyalGuardian};
use idleon_ui::format;

/// The two upgrade lists the pane switches between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum View {
    /// The Armory shelves.
    #[default]
    Armory,
    /// The Orblet market.
    Orblets,
}

impl View {
    pub(crate) const ALL: [View; 2] = [View::Armory, View::Orblets];

    pub(crate) fn id(self) -> &'static str {
        match self {
            View::Armory => "armory",
            View::Orblets => "orblets",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            View::Armory => "Armory",
            View::Orblets => "Orblets",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.id() == id)
    }
}

/// The Upgrades pane.
pub(crate) fn upgrades(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let view = state.royal_state.upgrade_view;
    let pane = div()
        .id("royal-upgrades")
        .flex()
        .flex_col()
        .gap_3()
        .child(view_select(view, cx))
        .child(redux_card(royal, cx));
    match view {
        View::Armory => pane
            .child(armory_card(royal, cx))
            .child(plan_card(royal, &state.royal_state, cx))
            .into_any_element(),
        View::Orblets => pane.child(orblets_card(royal, cx)).into_any_element(),
    }
}

/// The list switch.
fn view_select(view: View, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let choices: Vec<Choice> = View::ALL
        .iter()
        .map(|entry| Choice::new(entry.id(), entry.label()))
        .collect();
    div()
        .id("royal-upgrade-controls")
        .flex()
        .items_end()
        .gap_2()
        .child(
            Select::new("royal-upgrade-view", choices)
                .label("Upgrade list")
                .selected(view.id())
                .size(ControlSize::Sm)
                .on_change(move |value, _window, cx| {
                    if let Some(view) = View::from_id(value.as_ref()) {
                        entity.update(cx, |state, cx| state.set_royal_upgrade_view(view, cx));
                    }
                }),
        )
        .into_any_element()
}

/// The daily Masterclass reduction the Armory price multiplies in.
fn redux_card(royal: &RoyalGuardian, cx: &mut Context<AppState>) -> AnyElement {
    let redux = royal.masterclass_redux();
    let muted = cx.theme().colors.fg_muted;
    Card::new()
        .header(
            CardHeader::new("Masterclass costs")
                .description("The prefix every Armory shelf is priced with"),
        )
        .child(
            div()
                .id("royal-masterclass-redux")
                .flex()
                .flex_col()
                .gap_1()
                .child(format!(
                    "{}x ({}% reduction) · daily prefix {}x · BARGAIN {}x",
                    format::value(redux.value),
                    format::value(redux.reduction_percent),
                    format::value(redux.prefix),
                    format::value(redux.bargain)
                ))
                .child(
                    Caption::new(format!(
                        "{} of {} daily discounted purchases left",
                        figure_text(redux.discounts_left()),
                        figure_text(redux.daily_limit)
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// Every Armory shelf the order lists.
fn armory_card(royal: &RoyalGuardian, cx: &mut Context<AppState>) -> AnyElement {
    let rows = royal.armory_rows();
    let unlocked = royal.armory_unlocked_count();
    let affordable = rows
        .iter()
        .filter(|row| row.affordable && !row.maxed)
        .count() as f64;
    let mut table = Table::new(["Shelf", "Name", "Level", "Effect", "Cost", "Balance"]);
    let entity = cx.entity();
    for row in &rows {
        let order = row.order;
        let selectable = !row.maxed;
        let entity = entity.clone();
        let cell = div()
            .id(format!("royal-armory-row-{order}"))
            .child(format!("{}", order + 1));
        let cell = if selectable {
            cell.cursor_pointer().on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| state.set_royal_armory_plan(Some(order), cx));
            })
        } else {
            cell
        };
        table = table.row([
            cell.into_any_element(),
            div()
                .child(format!(
                    "{}{}",
                    row.name,
                    if row.unlocked { "" } else { " (locked)" }
                ))
                .into_any_element(),
            div()
                .child(format!(
                    "{} / {}",
                    figure_text(row.level),
                    figure_text(row.max_level)
                ))
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .child(format!("+{} now", format::value(row.current_bonus)))
                .child(Caption::new(format!(
                    "+{} per level",
                    format::value(row.bonus_per_level)
                )))
                .into_any_element(),
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(if row.maxed {
                    "MAX".to_owned()
                } else {
                    format!(
                        "{} (slot {})",
                        format::value(row.cost.value),
                        row.cost.currency_slot
                    )
                })
                .when(!row.maxed, |cell| {
                    cell.child(if row.affordable {
                        Badge::new("Affordable").tone(Tone::Success)
                    } else {
                        Badge::new("Saving").tone(Tone::Neutral)
                    })
                })
                .into_any_element(),
            number_cell(row.balance),
        ]);
    }
    div()
        .id("royal-upgrade-armory")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(kpi(
                    "royal-kpi-armory-shelves",
                    "Shelves reached",
                    unlocked as f64,
                    "",
                    format!("of {}", rows.len()),
                ))
                .child(kpi(
                    "royal-kpi-armory-affordable",
                    "Affordable now",
                    affordable,
                    "",
                    "At the saved balance",
                )),
        )
        .child(
            Card::new()
                .header(CardHeader::new("Armory").description(
                    "Every shelf in the Research catalog order; pick one to price its next levels",
                ))
                .child(div().id("royal-armory-table").child(table)),
        )
        .into_any_element()
}

/// The cost plan for one shelf: upstream `armoryCostProjection`.
fn plan_card(royal: &RoyalGuardian, state: &State, cx: &mut Context<AppState>) -> AnyElement {
    if state.upgrade_view != View::Armory {
        return div().into_any_element();
    }
    let rows = royal.armory_rows();
    let open: Vec<&ArmoryRow> = rows.iter().filter(|row| !row.maxed).collect();
    let choices: Vec<Choice> = open
        .iter()
        .map(|row| {
            Choice::new(
                row.order.to_string(),
                format!(
                    "{} · {} (slot {})",
                    row.order + 1,
                    row.name,
                    row.cost.currency_slot
                ),
            )
        })
        .collect();
    let order = state
        .armory_plan
        .filter(|order| open.iter().any(|row| row.order == *order))
        .or_else(|| open.first().map(|row| row.order));
    let Some(order) = order else {
        return Card::new()
            .header(CardHeader::new("Cost plan"))
            .child(
                div()
                    .id("royal-armory-plan-empty")
                    .child(Caption::new("Every shelf is maxed.")),
            )
            .into_any_element();
    };
    let entity = cx.entity();
    let select = Select::new("royal-plan-shelf", choices)
        .label("Shelf")
        .selected(order.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(order) = value.parse::<usize>() {
                entity.update(cx, |state, cx| state.set_royal_armory_plan(Some(order), cx));
            }
        });
    let plan = royal.armory_cost_plan(order, PROJECTION_LEVELS);
    let Some(plan) = plan else {
        return div().into_any_element();
    };
    let mut table = Table::new(["Level", "Discounted", "Full price", "Total (full)"]);
    for step in &plan.rows {
        table = table.row([
            div()
                .child(format!(
                    "{} → {}",
                    figure_text(step.level),
                    figure_text(step.next_level)
                ))
                .into_any_element(),
            number_cell(step.discounted),
            number_cell(step.full),
            number_cell(step.cumulative_full),
        ]);
    }
    let summary = format!(
        "{} · level {} of {} · next {} levels · balance {} covers {} at full price{}",
        plan.name,
        figure_text(plan.level),
        figure_text(plan.max_level),
        figure_text(plan.rows.len() as f64),
        format::value(plan.balance),
        figure_text(plan.covered_full() as f64),
        if plan.discount_available {
            format!(
                " or {} with the daily discount",
                figure_text(plan.covered_discounted() as f64)
            )
        } else {
            String::new()
        }
    );
    Card::new()
        .header(
            CardHeader::new(format!("Cost plan · {}", plan.name))
                .description("The next levels priced one at a time"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .id("royal-armory-plan")
                        .role(gpui::Role::Label)
                        .aria_label(summary.clone())
                        .child(summary),
                )
                .child(select)
                .child(div().id("royal-armory-plan-table").child(table)),
        )
        .into_any_element()
}

/// Every Orblet market row.
fn orblets_card(royal: &RoyalGuardian, _cx: &mut Context<AppState>) -> AnyElement {
    let rows = royal.orblet_rows();
    let owned = royal.orblet_owned();
    let mut table = Table::new(["Orblet", "Level", "Effect", "Cost", "Affordable"]);
    for row in &rows {
        table = table.row([
            div()
                .id(format!("royal-orblet-row-{}", row.index))
                .child(format!(
                    "{}{}{}",
                    row.name,
                    if row.unlocked { "" } else { " (locked)" },
                    if row.action { " (action)" } else { "" }
                ))
                .into_any_element(),
            div()
                .child(format!(
                    "{} / {}",
                    figure_text(row.level),
                    figure_text(row.max_level)
                ))
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .child(format!("+{} now", format::value(row.bonus)))
                .when_some(row.next_bonus, |cell, next| {
                    cell.child(Caption::new(format!("+{} next", format::value(next))))
                })
                .into_any_element(),
            div()
                .child(if row.maxed {
                    "MAX".to_owned()
                } else {
                    format::value(row.cost)
                })
                .into_any_element(),
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(if !row.unlocked {
                    Badge::new("Locked").tone(Tone::Neutral)
                } else if row.maxed {
                    Badge::new("Maxed").tone(Tone::Accent)
                } else if row.affordable {
                    Badge::new("Affordable").tone(Tone::Success)
                } else {
                    Badge::new("Saving").tone(Tone::Warning)
                })
                .into_any_element(),
        ]);
    }
    div()
        .id("royal-upgrade-orblets")
        .flex()
        .flex_col()
        .gap_3()
        .child(kpi(
            "royal-kpi-orblets-owned",
            "Orblets owned",
            owned.value,
            "",
            if owned.partial {
                "Some inventories did not read"
            } else {
                "Chest and every character inventory"
            },
        ))
        .child(
            Card::new()
                .header(
                    CardHeader::new("Orblets")
                        .description("Every market row; the previous orblet opens the next"),
                )
                .child(div().id("royal-orblet-table").child(table)),
        )
        .into_any_element()
}
