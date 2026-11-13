//! JR-04: the Enchant History tab.
//!
//! Ported from `_renderEnchantHistory` in `jar-calc.html`. The page draws a
//! sparkline behind each tile and keeps the snapshots in `localStorage`; here
//! the snapshots are part of the project document and the spread reads as a
//! table, which is the same information the sparkline's endpoint gives without
//! a canvas the section cannot describe to assistive technology.

use crate::sections::jar::metric;
use crate::session::time_label;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::Tone;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_jar::{COLLECTIBLES, Jar, collectible_name};

/// How many snapshots the recent table lists, the page's own tail.
const RECENT: usize = 20;

/// The history pane: its count, the current spread, the recent snapshots and
/// the clear action.
pub(crate) fn history(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let jar = state
        .jar
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let snapshots = &state.jar_state.history;
    let body = div()
        .id("jar-enchant-history")
        .flex()
        .flex_col()
        .gap_3()
        .child(count(snapshots.len(), cx));
    if snapshots.is_empty() {
        return body
            .child(
                Card::new()
                    .header(CardHeader::new("Enchant History"))
                    .child(div().p_3().child(Caption::new(
                        "Snapshots are saved each time your save is applied. Apply a save to \
                         start tracking.",
                    ))),
            )
            .into_any_element();
    }
    let entity = cx.entity();
    body.child(current_spread(jar, snapshots))
        .child(recent(snapshots))
        .child(
            div().child(
                Button::new("jar-history-clear", "Clear History")
                    .icon(IconName::X)
                    .on_click(move |_event, _window, cx| {
                        entity.update(cx, |state, cx| state.clear_jar_history(cx));
                    }),
            ),
        )
        .into_any_element()
}

/// `N snapshots recorded.`
fn count(len: usize, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("jar-history-count")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric(
            "jar-history-snapshots",
            "Snapshots",
            len.to_string(),
            "A spread is recorded when an applied save changes it".to_owned(),
            Tone::Info,
            cx,
        ))
        .into_any_element()
}

/// Every found collectible's current level, beside the first snapshot's.
fn current_spread(jar: &Jar, snapshots: &[idleon_jar::EnchantSnapshot]) -> AnyElement {
    let first = &snapshots[0];
    let last = snapshots.last().expect("a non-empty history has a last");
    let mut table = Table::new(["Collectible", "Current", "First snapshot", "Change"]);
    for index in 0..COLLECTIBLES {
        if jar.collectible(index).is_none_or(|item| item.level < 1) {
            continue;
        }
        let current = last.level(index);
        let started = first.level(index);
        table = table.row([
            div()
                .id(format!("jar-history-spread-{index}"))
                .child(collectible_name(index))
                .into_any_element(),
            div().child(format!("+{current}")).into_any_element(),
            div().child(format!("+{started}")).into_any_element(),
            div()
                .child(match current.cmp(&started) {
                    std::cmp::Ordering::Greater => format!("+{}", current - started),
                    std::cmp::Ordering::Less => format!("-{}", started - current),
                    std::cmp::Ordering::Equal => "—".to_owned(),
                })
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Current Spread").description(
            "Enchant levels above the collectible's own level, against the first snapshot.",
        ))
        .child(div().p_3().child(table))
        .into_any_element()
}

/// The most recent snapshots, newest first, with each one's change.
fn recent(snapshots: &[idleon_jar::EnchantSnapshot]) -> AnyElement {
    let mut table = Table::new(["Time", "Total levels", "Δ"]);
    let start = snapshots.len().saturating_sub(RECENT);
    for (index, snapshot) in snapshots.iter().enumerate().skip(start).rev() {
        let previous = index.checked_sub(1).map(|previous| &snapshots[previous]);
        let delta = match previous {
            Some(previous) => i64::from(snapshot.total) - i64::from(previous.total),
            None => 0,
        };
        table = table.row([
            div()
                .id(format!("jar-history-row-{index}"))
                .child(time_label(snapshot.at))
                .into_any_element(),
            div().child(snapshot.total.to_string()).into_any_element(),
            div()
                .child(match delta {
                    0 => "—".to_owned(),
                    positive if positive > 0 => format!("+{positive}"),
                    negative => negative.to_string(),
                })
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Recent Snapshots").description(format!(
            "The newest {} of {} snapshots.",
            snapshots.len().min(RECENT),
            snapshots.len()
        )))
        .child(div().p_3().child(table))
        .into_any_element()
}
