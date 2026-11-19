//! The BoneJoePickle calculator: DPS and a kill-time budget against the
//! minibosses that stack pickles.
//!
//! Every pickle multiplies a miniboss's HP by 1.10, so the table answers how
//! many still die inside the budget.

mod actions;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use gpui::Focusable as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, FontWeight, IntoElement, ParentElement, Pixels, Role,
    SharedString, Styled, Subscription, Window, div, px,
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use idleon_pickle::{self as pickle, DEFAULT_TTK, MAX_DPS, MAX_TTK, MIN_TTK};
use idleon_ui::widgets::{
    Caption, Card, CardHeader, PANEL_GAP, TEXT_CAPTION, TEXT_MONO, Tone, number_value, show_number,
};

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

/// The kit controls that show [`State`], built once (`docs/ui-kit/forms.md`).
/// Edits flow back through the section's setters; render brings the controls
/// in line with a restored or clamped value.
pub(crate) struct Controls {
    dps: Entity<InputState>,
    ttk: Entity<SliderState>,
    _subscriptions: [Subscription; 2],
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let dps = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. 50000000")
                .min(0.0)
                .max(MAX_DPS)
                .step(1_000_000.0)
        });
        let ttk = cx.new(|_| {
            SliderState::new()
                .min(MIN_TTK as f32)
                .max(MAX_TTK as f32)
                .step(1.0)
                .default_value(DEFAULT_TTK as f32)
        });
        let subscriptions = [
            cx.subscribe(&dps, |state, field, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                // A cleared field means nothing entered, as the empty table says.
                let field = field.read(cx);
                let dps = match number_value(field) {
                    Some(dps) => dps,
                    None if field.value().trim().is_empty() => 0.0,
                    None => return,
                };
                state.set_pickle_dps(dps, cx);
            }),
            cx.subscribe(&ttk, |state, _, event: &SliderEvent, cx| {
                let (SliderEvent::Change(value) | SliderEvent::Release(value)) = event;
                state.set_pickle_ttk(f64::from(value.end()), cx);
            }),
        ];
        Self {
            dps,
            ttk,
            _subscriptions: subscriptions,
        }
    }

    /// Bring both controls in line with the section's values. Zero DPS shows
    /// the blank field and its hint.
    fn show(&self, pickle: State, window: &mut Window, cx: &mut Context<AppState>) {
        if pickle.dps > 0.0 {
            show_number(&self.dps, pickle.dps, 0, window, cx);
        } else if number_value(self.dps.read(cx)).is_some_and(|dps| dps > 0.0)
            && !self.dps.focus_handle(cx).is_focused(window)
        {
            self.dps
                .update(cx, |field, cx| field.set_value("", window, cx));
        }
        if f64::from(self.ttk.read(cx).value().end()) != pickle.max_ttk {
            self.ttk.update(cx, |slider, cx| {
                slider.set_value(pickle.max_ttk as f32, window, cx);
            });
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
    let pickle = state.pickle;
    state.pickle_controls.show(pickle, window, cx);
    layout::split_pane_reversed(
        "pickle-layout",
        Breakpoint::of(window, cx),
        results(pickle.dps, pickle.max_ttk, height, cx),
        controls(&state.pickle_controls, pickle.max_ttk, cx),
        px(360.0),
    )
}

fn controls(controls: &Controls, max_ttk: f64, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(CardHeader::new("Combat inputs").description(
            "Hit the Shimmer Island target dummy and enter the damage per second it reports.",
        ))
        .child(
            div()
                .id("pickle-controls")
                .flex()
                .flex_col()
                .gap(PANEL_GAP)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Your DPS"))
                        .child(crate::ui::number_field("pickle-dps", &controls.dps, "DPS").w_full()),
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
                                .text_size(TEXT_MONO)
                                .font_family(cx.theme().mono_font_family.clone())
                                .child(format!("{max_ttk:.0}s")),
                        ),
                )
                .child(
                    div()
                        .id("pickle-ttk")
                        .child(Slider::new(&controls.ttk).accessibility_label("Max time to kill")),
                )
                .child(Caption::new(
                    "Pickles only stack on the minibosses of the Revenge of the Pickle Death Note page, not dungeon or pet mobs.",
                ).text_color(muted)),
        )
        .into_any_element()
}

fn results(dps: f64, max_ttk: f64, height: Pixels, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let show_effective = dps > 0.0;

    let head = TableRow::new()
        .child(TableHead::new().child("Miniboss"))
        .child(TableHead::new().text_right().child("Base HP"))
        .child(TableHead::new().text_right().child("Max Pickles"))
        .child(TableHead::new().text_right().child("Effective HP"))
        .child(TableHead::new().text_right().child("Kill Time"));
    let rows = pickle::rows(dps, max_ttk).map(|row| {
        let (pickle_text, tone) = match row.max_pickles {
            None => ("✗".to_string(), Tone::Danger),
            Some(0) => ("0".to_string(), Tone::Warning),
            Some(pickles) => (pickles.to_string(), Tone::Success),
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
        TableRow::new()
            .id(SharedString::from(format!(
                "pickle-tr-{}",
                row.miniboss.code
            )))
            .child(
                TableCell::new().child(
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
                        .child(Caption::new(row.miniboss.world)),
                ),
            )
            .child(
                TableCell::new()
                    .text_right()
                    .child(pickle::compact(row.miniboss.base_hp)),
            )
            .child(
                TableCell::new().text_right().child(
                    div()
                        .text_color(tone.color(cx))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(pickle_text),
                ),
            )
            .child(TableCell::new().text_right().child(effective))
            .child(
                TableCell::new()
                    .text_right()
                    .child(div().text_color(muted).child(kill_time)),
            )
    });
    let table = Table::new()
        .child(TableHeader::new().child(head))
        .child(TableBody::new().children(rows.collect::<Vec<_>>()));

    let body = div()
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .min_h_0()
        .when(!show_effective, |body| {
            body.child(
                Alert::new(
                    "pickle-empty",
                    "Enter your DPS to see how many pickles each miniboss can stack and still die in time.",
                )
                .with_variant(Tone::Info.into())
                .banner(),
            )
        })
        .child(
            div()
                .h(height)
                .child(table)
                .overflow_y_scrollbar()
                .id("pickle-table-scroll"),
        );

    Card::new()
        .header(CardHeader::new("Minibosses").description(format!(
            "How many pickles still fit inside a {max_ttk:.0}s kill-time budget."
        )))
        .child(body)
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(
                    "Formula: HP × 1.10^pickles. The Revenge of the Pickle Jade Emporium upgrade is a separate bonus.",
                ),
        )
        .into_any_element()
}

/// The kill-time cell: a dash without DPS, the base time when the
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
