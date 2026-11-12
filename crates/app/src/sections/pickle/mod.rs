//! The BoneJoePickle calculator: DPS and a kill-time budget against the
//! minibosses that stack pickles.
//!
//! Ported from `pickle-calc.html`; every pickle multiplies a miniboss's HP by
//! 1.10, so the table answers how many still die inside the budget.

mod actions;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{NumberInput, Slider};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Pixels, Role, SharedString,
    Styled, Window, div, px,
};
use idleon_pickle::{self as pickle, DEFAULT_TTK, MAX_DPS, MAX_TTK, MIN_TTK};

/// The section's live inputs, kept in `AppState` while the window is open.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    /// Damage per second; zero means nothing has been entered yet.
    pub(crate) dps: f64,
    /// The kill-time budget in seconds.
    pub(crate) max_ttk: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            dps: 0.0,
            max_ttk: DEFAULT_TTK,
        }
    }
}

impl State {
    /// Fit both inputs to the ranges the controls offer; a non-finite entry
    /// falls back to its default.
    pub(crate) fn clamped(self) -> Self {
        Self {
            dps: if self.dps.is_finite() {
                self.dps.clamp(0.0, MAX_DPS)
            } else {
                0.0
            },
            max_ttk: if self.max_ttk.is_finite() {
                self.max_ttk.clamp(MIN_TTK, MAX_TTK)
            } else {
                DEFAULT_TTK
            },
        }
    }
}

/// The inputs and the miniboss table, side by side or stacked.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let height = layout::table_height(f32::from(window.viewport_size().height));
    let dps = state.pickle.dps;
    let max_ttk = state.pickle.max_ttk;
    layout::split_pane_reversed(
        "pickle-layout",
        Breakpoint::of(window),
        results(dps, max_ttk, height, cx),
        controls(dps, max_ttk, cx),
        px(360.0),
    )
}

fn controls(dps: f64, max_ttk: f64, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let dps_entity = entity.clone();
    let ttk_entity = entity;

    let dps_input = NumberInput::new("pickle-dps", dps)
        .range(0.0, MAX_DPS)
        .step(1_000_000.0)
        .precision(0)
        .blank("e.g. 50000000")
        .label("DPS")
        .on_change(move |value, _window, cx| {
            dps_entity.update(cx, |state, cx| state.set_pickle_dps(value, cx));
        });
    let ttk_slider = Slider::new("pickle-ttk", max_ttk)
        .range(MIN_TTK, MAX_TTK)
        .step(1.0)
        .label("Max time to kill")
        .on_change(move |value, _window, cx| {
            ttk_entity.update(cx, |state, cx| state.set_pickle_ttk(value, cx));
        });

    Card::new()
        .header(CardHeader::new("Combat inputs").description(
            "Hit the Shimmer Island target dummy and enter the damage per second it reports.",
        ))
        .child(
            div()
                .id("pickle-controls")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Your DPS"))
                        .child(dps_input),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(Caption::new("Max time to kill"))
                        .child(
                            div()
                                .id("pickle-ttk-value")
                                .role(Role::Label)
                                .aria_label(format!("{max_ttk:.0} seconds"))
                                .text_size(px(16.0))
                                .child(format!("{max_ttk:.0}s")),
                        ),
                )
                .child(ttk_slider)
                .child(Caption::new(
                    "Pickles only stack on the minibosses of the Revenge of the Pickle Death Note page, not dungeon or pet mobs.",
                ).text_color(muted)),
        )
        .into_any_element()
}

fn results(dps: f64, max_ttk: f64, height: Pixels, cx: &mut Context<AppState>) -> AnyElement {
    let colors = &cx.theme().colors;
    let (success, warning, danger, muted) = (
        colors.success,
        colors.warning,
        colors.danger,
        colors.fg_muted,
    );
    let show_effective = dps > 0.0;

    let mut table = Table::new([
        "Miniboss",
        "Base HP",
        "Max Pickles",
        "Effective HP",
        "Kill Time",
    ]);
    for row in pickle::rows(dps, max_ttk) {
        let (pickle_text, tone) = match row.max_pickles {
            None => ("✗".to_string(), danger),
            Some(0) => ("0".to_string(), warning),
            Some(pickles) => (pickles.to_string(), success),
        };
        let effective = if show_effective {
            pickle::compact(row.effective_hp)
        } else {
            "—".to_string()
        };
        let kill_time = kill_time(row, dps);
        let summary = format!(
            "{} {} · base HP {} · max pickles {} · effective HP {} · kill time {}",
            row.miniboss.name,
            row.miniboss.world,
            pickle::compact(row.miniboss.base_hp),
            pickle_text,
            effective,
            kill_time
        );
        table = table.row([
            div()
                .id(SharedString::from(format!(
                    "pickle-row-{}",
                    row.miniboss.code
                )))
                .role(Role::Label)
                .aria_label(summary)
                .flex()
                .flex_col()
                .gap_0p5()
                .child(row.miniboss.name)
                .child(Caption::new(row.miniboss.world))
                .into_any_element(),
            div()
                .child(pickle::compact(row.miniboss.base_hp))
                .into_any_element(),
            div()
                .text_color(tone)
                .font_weight(FontWeight::SEMIBOLD)
                .child(pickle_text)
                .into_any_element(),
            div().child(effective).into_any_element(),
            div().child(kill_time).text_color(muted).into_any_element(),
        ]);
    }

    let body = div()
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .min_h_0()
        .when(!show_effective, |body| {
            body.child(Banner::new(
                "pickle-empty",
                Severity::Info,
                "Enter your DPS to see how many pickles each miniboss can stack and still die in time.",
            ))
        })
        .child(
            ScrollArea::new("pickle-table-scroll")
                .vertical()
                .h(height)
                .child(table),
        );

    Card::new()
        .header(CardHeader::new("Minibosses").description(format!(
            "How many pickles still fit inside a {max_ttk:.0}s kill-time budget."
        )))
        .child(body)
        .child(
            div()
                .px_3()
                .pb_2()
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "Formula: HP × 1.10^pickles. The Revenge of the Pickle Jade Emporium upgrade is a separate bonus.",
                ),
        )
        .into_any_element()
}

/// The upstream kill-time cell: a dash without DPS, the base time when the
/// budget cannot reach the miniboss, and the stacked time otherwise.
fn kill_time(row: pickle::Row, dps: f64) -> String {
    if dps <= 0.0 {
        "—".into()
    } else if row.max_pickles.is_none() {
        format!("{}s", pickle::compact(row.kill_time))
    } else {
        format!("{:.1}s", row.kill_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_field_has_no_kill_time() {
        let first = pickle::rows(0.0, 30.0).next().expect("a row");
        assert_eq!(kill_time(first, 0.0), "—");
    }

    #[test]
    fn a_miniboss_outside_the_budget_shows_its_base_kill_time() {
        let rows: Vec<pickle::Row> = pickle::rows(10_000.0, 5.0).collect();
        assert_eq!(kill_time(rows[0], 10_000.0), "4.6s");
        assert_eq!(kill_time(rows[1], 10_000.0), "75s");
        assert_eq!(kill_time(rows[8], 10_000.0), "2.50Bs");
    }
}
