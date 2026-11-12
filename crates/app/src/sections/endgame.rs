//! The Endgame Check section: the joke verdict and the leaderboard audit.
//!
//! Ported from the upstream `endgame.html`: a huge red `NO` over the account's
//! leaderboard statistics. The verdict never changes; the audit renders the save
//! table metrics [`idleon_endgame`] can read and says why the rest are
//! unavailable, standing in for the upstream grid and its `Unavailable` cells.

use crate::state::AppState;
use ely_gpui_component::feedback::{Alert, EmptyState};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, Styled, Window, div, px, rgb,
};
use idleon_ui::format;

/// The endgame section's own state: the audit of the last applied save.
#[derive(Default)]
pub(crate) struct State {
    /// `None` until a save is applied; the verdict is `NO` either way.
    pub(crate) stats: Option<Result<Vec<idleon_endgame::Stat>, String>>,
}

impl State {
    /// Audit an applied save, exactly as the upstream page does on load.
    pub(crate) fn audit(imported: &idleon_import::Imported) -> Self {
        Self {
            stats: Some(idleon_endgame::collect(
                &imported.save,
                imported.named_characters,
            )),
        }
    }
}

/// The section: the verdict, then the account audit.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let size = (f32::from(window.viewport_size().height) * 0.2).clamp(96.0, 180.0);
    let verdict = div()
        .id("endgame-verdict")
        .role(Role::Status)
        .aria_label(idleon_endgame::VERDICT)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .py_4()
        .child(
            div()
                .text_size(px(size))
                .font_weight(FontWeight::BLACK)
                .text_color(rgb(0xff_44_44))
                .child(idleon_endgame::VERDICT),
        );

    div()
        .id("endgame-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(verdict)
        .child(
            div()
                .id("endgame-audit")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(
                    ScrollArea::new("endgame-stats-scroll")
                        .vertical()
                        .size_full()
                        .child(stats_card(state, muted)),
                ),
        )
        .into_any_element()
}

/// The `Account statistics` panel: a hint, an error, or the audit table.
fn stats_card(state: &AppState, muted: gpui::Hsla) -> impl IntoElement {
    let audit = state.endgame.stats.as_ref();
    let description = match audit {
        None => "Import a decoded save to audit its leaderboard metrics.".to_string(),
        Some(Err(_)) => "The imported save could not be audited.".to_string(),
        Some(Ok(stats)) => {
            let computed = stats.iter().filter(|stat| stat.value.is_some()).count();
            format!(
                "{computed} of {} leaderboard metrics computed from the imported save.",
                stats.len()
            )
        }
    };
    let card = Card::new().header(CardHeader::new("Account statistics").description(description));
    match audit {
        None => card.child(
            EmptyState::new(
                "endgame-stats-empty",
                IconName::SearchX,
                "Nothing to audit yet",
            )
            .body("The verdict is NO before a save and after one; the table is the fun part."),
        ),
        Some(Err(error)) => card.child(
            Alert::new(
                "endgame-stats-error",
                Severity::Danger,
                format!("Unable to calculate account statistics: {error}"),
            )
            .body("Import another decoded save, or check the one this came from."),
        ),
        Some(Ok(stats)) => card.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    Caption::new(
                        "Read from the imported save only; Unavailable means it does not carry the field.",
                    )
                    .text_color(muted),
                )
                .child(stats_table(stats, muted)),
        ),
    }
}

/// One row per leaderboard metric, in the upstream category order.
fn stats_table(stats: &[idleon_endgame::Stat], muted: gpui::Hsla) -> AnyElement {
    let mut table = Table::new(["Category", "Metric", "Value", "Note"]);
    for stat in stats {
        let shown = format::stat_value(stat.value);
        let note = match (&stat.value, &stat.detail) {
            (None, Some(detail)) => detail.clone(),
            (Some(_), _) if stat.partial => "Partial".to_string(),
            _ => String::new(),
        };
        table = table.row([
            div()
                .text_size(px(11.0))
                .child(stat.group)
                .into_any_element(),
            div().min_w_0().child(stat.label.clone()).into_any_element(),
            div()
                .id(format!("endgame-stat-{}", stat.id))
                .role(Role::Label)
                .aria_label(format!("{}: {shown}", stat.label))
                .text_color(rgb(0xff_d7_00))
                .child(shown)
                .into_any_element(),
            div()
                .min_w_0()
                .text_size(px(11.0))
                .text_color(muted)
                .child(note)
                .into_any_element(),
        ]);
    }
    div()
        .id("endgame-stats-table")
        .min_w_0()
        .child(table)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_import::import;

    #[test]
    fn an_applied_save_fills_the_audit_and_a_broken_one_reports_why() {
        let imported = import(&harness_save()).expect("the harness save imports");
        let state = State::audit(&imported);
        let stats = state
            .stats
            .expect("the save was audited")
            .expect("complete rows");
        let total_levels = stats
            .iter()
            .find(|stat| stat.id == "totalLevels")
            .and_then(|stat| stat.value);
        assert_eq!(total_levels, Some(160.0));

        // A usable save whose research rows are incomplete still imports, and
        // the audit reports the failure the error panel renders.
        let broken = import("{\"Research\":[]}").expect("the research field imports");
        let state = State::audit(&broken);
        assert_eq!(
            state.stats.expect("the audit ran").unwrap_err(),
            "Research data is missing or incomplete."
        );
    }

    /// The save the section harness pastes: two characters and the nine
    /// research rows the endgame audit validates, as the game's JSON strings.
    fn harness_save() -> String {
        let research = format!("[{}]", ["[0]"; 9].join(","));
        format!(
            "{{\"Research\":{},\"Lv0_0\":\"[100]\",\"Lv0_1\":\"[60]\",\"MoneyBANK\":100,\"Money_0\":5}}",
            serde_json::json!(research)
        )
    }
}
