//! The board grid: 12 columns × 8 slots, each cell showing its cog and the
//! objective stat.

use super::tag;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Hsla, IntoElement, ParentElement, Role, Styled, div, px};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Theme};
use idleon_cog::{
    BOARD_SIZE, BOARD_W, Board, BoardInsights, Cog, CogKind, Goal, classify, slot_to_pos,
};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// What the insights measured a slot to be, so the grid can mark it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CellRole {
    #[default]
    None,
    Weakest,
    Best,
}

/// The placement card: the grid, with the changed slots tinted and the two
/// measured extremes marked.
pub(crate) fn card(
    state: &AppState,
    board: &Board,
    insights: Option<&BoardInsights>,
    cx: &App,
) -> AnyElement {
    let changed = (0..BOARD_SIZE)
        .filter(|slot| board[*slot] != state.cog.save.board[*slot])
        .count();
    let description = if changed == 0 {
        "The save board. Run Optimize to place the shelf cogs.".to_string()
    } else {
        format!("{changed} slot(s) differ from the save board.")
    };
    let label = if state.cog.weighted {
        "Custom weights".to_string()
    } else {
        state.cog.goal.label().to_string()
    };
    Card::new()
        .header(
            CardHeader::new("Board")
                .description(description)
                .action(tag(format!("{label} · 12×8"), Tone::Accent)),
        )
        .child(grid(state, board, insights, cx))
        .into_any_element()
}

fn grid(state: &AppState, board: &Board, insights: Option<&BoardInsights>, cx: &App) -> AnyElement {
    let grid = div()
        .id("cog-board")
        .grid()
        .grid_cols(BOARD_W as u16)
        .gap_1()
        .children((0..BOARD_SIZE).map(|slot| {
            let cog = &board[slot];
            let changed = cog != &state.cog.save.board[slot];
            let role = insights.map_or(CellRole::None, |insights| {
                if insights.weakest.slot == slot {
                    CellRole::Weakest
                } else if insights.best.slot == slot {
                    CellRole::Best
                } else {
                    CellRole::None
                }
            });
            cell(cog, slot, state.cog.goal, changed, role, cx)
        }));
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(grid)
        .child(Caption::new(
            "The tint marks a slot the optimizer changed, and the red and green borders mark the weakest and strongest cog the insights measured. Player cogs and Yin blocks move too; the game only pays a Yin surround in its 2×2 order.",
        ))
        .into_any_element()
}

fn cell(
    cog: &Option<Cog>,
    slot: usize,
    goal: Goal,
    changed: bool,
    role: CellRole,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let (col, row) = slot_to_pos(slot);
    let Some(cog) = cog else {
        return div()
            .id(format!("cog-cell-{slot}"))
            .role(Role::Label)
            .aria_label(format!("Slot {slot}, column {col}, row {row}: empty"))
            .h(px(46.0))
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .into_any_element();
    };
    let kind = classify(&cog.name);
    let tint = tint(&kind, theme);
    let stat = match goal {
        Goal::Build => format::fmt_val(cog.a),
        Goal::ConExp => format!("{:.0}%", cog.d),
        Goal::Flaggy => format::fmt_val(cog.c),
    };
    let surround = match goal {
        Goal::Build => cog.e,
        Goal::ConExp => cog.f,
        Goal::Flaggy => cog.g,
    };
    let detail = if cog.direction.is_some() && surround > 0.0 {
        format!("{stat} · +{surround:.0}%")
    } else {
        stat
    };
    let marker = match role {
        CellRole::None => "",
        CellRole::Weakest => ", the board's weakest cog",
        CellRole::Best => ", the board's strongest cog",
    };
    let label = format!(
        "Slot {slot}, column {col}, row {row}: {}, {detail}{marker}",
        cog.display_name()
    );
    div()
        .id(format!("cog-cell-{slot}"))
        .role(Role::Label)
        .aria_label(label.clone())
        .h(px(46.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(theme.radius)
        .bg(if changed {
            theme.primary.opacity(0.12)
        } else {
            theme.secondary
        })
        .border_1()
        .border_color(match role {
            CellRole::Weakest => theme.danger,
            CellRole::Best => theme.success,
            CellRole::None if changed => theme.primary,
            CellRole::None => theme.border,
        })
        .child(div().h(px(2.0)).w_full().bg(tint))
        .child(
            div()
                .p_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(cog.short_label()),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(theme.muted_foreground)
                        .child(detail),
                ),
        )
        .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
        .into_any_element()
}

fn tint(kind: &CogKind, theme: &Theme) -> Hsla {
    match kind {
        CogKind::Player(_) => theme.primary,
        CogKind::Crystal { .. } => theme.warning,
        CogKind::Excogia { .. } => theme.success,
        CogKind::Yellow => theme.chart_2,
        CogKind::Standard { .. } => theme.chart_1,
        CogKind::Small { .. } => theme.chart_5,
        CogKind::Empty | CogKind::Unknown => theme.border,
    }
}
