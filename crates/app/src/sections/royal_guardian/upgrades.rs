//! The Upgrades tab: every Armory shelf, every Orblet and the cost plan for the
//! next levels of one shelf.
//!
//! The Royal statues and the statue flair are the Statue section's own tables,
//! so this pane carries the two upgrades the Royal Guardian owns.

use super::overview::{figure_text, kpi, number_cell, tag};
use super::table::{Col, table, text, text_id};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, Window, div, px};
use gpui_kit::component::select::Select;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use idleon_royal_guardian::{ArmoryRow, PROJECTION_LEVELS, RoyalGuardian};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, KpiStrip, PANEL_GAP, Tone};

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

    pub(crate) fn label(self) -> &'static str {
        match self {
            View::Armory => "Armory",
            View::Orblets => "Orblets",
        }
    }
}

/// The list switch's options.
pub(super) fn view_choices() -> Vec<Choice<View>> {
    View::ALL
        .into_iter()
        .map(|view| Choice::new(view, view.label()))
        .collect()
}

/// The Upgrades pane.
pub(crate) fn upgrades(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let view = state.royal_state.upgrade_view;
    let select = &state.royal_controls.upgrade_view;
    select.show(view_choices(), Some(&view), window, cx);
    let pane = v_flex()
        .id("royal-upgrades")
        .gap(PANEL_GAP)
        .child(
            div().id("royal-upgrade-controls").child(
                Select::new(select.state())
                    .id("royal-upgrade-view")
                    .accessibility_label("Upgrade list")
                    .small()
                    .w(px(200.0)),
            ),
        )
        .child(redux_card(royal));
    match view {
        View::Armory => {
            let rows = royal.armory_rows();
            pane.child(armory_card(royal, &rows, cx))
                .child(plan_card(state, royal, &rows, window, cx))
                .into_any_element()
        }
        View::Orblets => pane.child(orblets_card(royal, cx)).into_any_element(),
    }
}

/// The daily Masterclass reduction the Armory price multiplies in.
fn redux_card(royal: &RoyalGuardian) -> AnyElement {
    let redux = royal.masterclass_redux();
    Card::new()
        .header(
            CardHeader::new("Masterclass costs")
                .description("The prefix every Armory shelf is priced with"),
        )
        .child(
            v_flex()
                .id("royal-masterclass-redux")
                .gap_1()
                .child(format!(
                    "{}x ({}% reduction) · daily prefix {}x · BARGAIN {}x",
                    format::value(redux.value),
                    format::value(redux.reduction_percent),
                    format::value(redux.prefix),
                    format::value(redux.bargain)
                ))
                .child(Caption::new(format!(
                    "{} of {} daily discounted purchases left",
                    figure_text(redux.discounts_left()),
                    figure_text(redux.daily_limit)
                ))),
        )
        .into_any_element()
}

/// Every Armory shelf the order lists.
fn armory_card(
    royal: &RoyalGuardian,
    rows: &[ArmoryRow],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let unlocked = royal.armory_unlocked_count();
    let affordable = rows
        .iter()
        .filter(|row| row.affordable && !row.maxed)
        .count() as f64;
    let entity = cx.entity();
    let cells = rows.iter().map(|row| {
        let order = row.order;
        let entity = entity.clone();
        let shelf = div()
            .id(SharedString::from(format!("royal-armory-row-{order}")))
            .child(format!("{}", order + 1))
            .when(!row.maxed, |cell| {
                cell.cursor_pointer().on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.set_royal_armory_plan(Some(order), cx));
                })
            });
        vec![
            shelf.into_any_element(),
            text(format!(
                "{}{}",
                row.name,
                if row.unlocked { "" } else { " (locked)" }
            )),
            text(format!(
                "{} / {}",
                figure_text(row.level),
                figure_text(row.max_level)
            )),
            v_flex()
                .child(format!("+{} now", format::value(row.current_bonus)))
                .child(Caption::new(format!(
                    "+{} per level",
                    format::value(row.bonus_per_level)
                )))
                .into_any_element(),
            h_flex()
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
                        tag("Affordable", Tone::Success)
                    } else {
                        tag("Saving", Tone::Neutral)
                    })
                })
                .into_any_element(),
            number_cell(row.balance),
        ]
    });
    let table = table(
        "royal-armory",
        &[
            Col::Text("Shelf"),
            Col::Text("Name"),
            Col::Text("Level"),
            Col::Text("Effect"),
            Col::Text("Cost"),
            Col::Number("Balance"),
        ],
        cells,
        cx,
    );
    v_flex()
        .id("royal-upgrade-armory")
        .gap(PANEL_GAP)
        .child(
            KpiStrip::new()
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

/// The cost plan for one shelf.
fn plan_card(
    state: &AppState,
    royal: &RoyalGuardian,
    rows: &[ArmoryRow],
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let open: Vec<&ArmoryRow> = rows.iter().filter(|row| !row.maxed).collect();
    let order = state
        .royal_state
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
    let Some(plan) = royal.armory_cost_plan(order, PROJECTION_LEVELS) else {
        return div().into_any_element();
    };
    let choices: Vec<Choice<usize>> = open
        .iter()
        .map(|row| {
            Choice::new(
                row.order,
                format!(
                    "{} · {} (slot {})",
                    row.order + 1,
                    row.name,
                    row.cost.currency_slot
                ),
            )
        })
        .collect();
    let select = &state.royal_controls.plan_shelf;
    select.show(choices, Some(&order), window, cx);
    let steps = plan.rows.iter().map(|step| {
        vec![
            text(format!(
                "{} → {}",
                figure_text(step.level),
                figure_text(step.next_level)
            )),
            number_cell(step.discounted),
            number_cell(step.full),
            number_cell(step.cumulative_full),
        ]
    });
    let table = table(
        "royal-armory-plan",
        &[
            Col::Text("Level"),
            Col::Number("Discounted"),
            Col::Number("Full price"),
            Col::Number("Total (full)"),
        ],
        steps,
        cx,
    );
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
            v_flex()
                .gap_2()
                .child(
                    div()
                        .id("royal-armory-plan")
                        .role(gpui::Role::Label)
                        .aria_label(summary.clone())
                        .child(summary),
                )
                .child(
                    Select::new(select.state())
                        .id("royal-plan-shelf")
                        .accessibility_label("Shelf")
                        .small()
                        .w(px(320.0)),
                )
                .child(div().id("royal-armory-plan-table").child(table)),
        )
        .into_any_element()
}

/// Every Orblet market row.
fn orblets_card(royal: &RoyalGuardian, cx: &mut Context<AppState>) -> AnyElement {
    let rows = royal.orblet_rows();
    let owned = royal.orblet_owned();
    let cells = rows.iter().map(|row| {
        vec![
            text_id(
                SharedString::from(format!("royal-orblet-row-{}", row.index)),
                format!(
                    "{}{}{}",
                    row.name,
                    if row.unlocked { "" } else { " (locked)" },
                    if row.action { " (action)" } else { "" }
                ),
            ),
            text(format!(
                "{} / {}",
                figure_text(row.level),
                figure_text(row.max_level)
            )),
            v_flex()
                .child(format!("+{} now", format::value(row.bonus)))
                .when_some(row.next_bonus, |cell, next| {
                    cell.child(Caption::new(format!("+{} next", format::value(next))))
                })
                .into_any_element(),
            text(if row.maxed {
                "MAX".to_owned()
            } else {
                format::value(row.cost)
            }),
            if !row.unlocked {
                tag("Locked", Tone::Neutral)
            } else if row.maxed {
                tag("Maxed", Tone::Accent)
            } else if row.affordable {
                tag("Affordable", Tone::Success)
            } else {
                tag("Saving", Tone::Warning)
            }
            .into_any_element(),
        ]
    });
    let table = table(
        "royal-orblet",
        &[
            Col::Text("Orblet"),
            Col::Text("Level"),
            Col::Text("Effect"),
            Col::Number("Cost"),
            Col::Text("Affordable"),
        ],
        cells,
        cx,
    );
    v_flex()
        .id("royal-upgrade-orblets")
        .gap(PANEL_GAP)
        .child(KpiStrip::new().child(kpi(
            "royal-kpi-orblets-owned",
            "Orblets owned",
            owned.value,
            "",
            if owned.partial {
                "Some inventories did not read"
            } else {
                "Chest and every character inventory"
            },
        )))
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
