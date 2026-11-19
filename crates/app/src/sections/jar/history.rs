//! JR-04: the Enchant History tab.
//!
//! The snapshots are part of the project document and the spread reads as a
//! table rather than a sparkline, so the section can describe it to assistive
//! technology.

use super::table::{Col, table, text};
use crate::sections::jar::{metric, row};
use crate::session::time_label;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use idleon_jar::{COLLECTIBLES, EnchantSnapshot, Jar, collectible_name};
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// How many snapshots the recent table lists.
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
        .child(count(snapshots.len()));
    if snapshots.is_empty() {
        return body
            .child(
                Card::new()
                    .header(CardHeader::new("Enchant History"))
                    .child(Caption::new(
                        "Snapshots are saved each time your save is applied. Apply a save to \
                         start tracking.",
                    )),
            )
            .into_any_element();
    }
    let entity = cx.entity();
    body.child(current_spread(jar, snapshots, cx))
        .child(recent(snapshots, cx))
        .child(
            div().child(
                Button::new("jar-history-clear")
                    .label("Clear History")
                    .icon(IconName::X)
                    .on_click(move |_event, _window, cx| {
                        entity.update(cx, |state, cx| state.clear_jar_history(cx));
                    }),
            ),
        )
        .into_any_element()
}

/// `N snapshots recorded.`
fn count(len: usize) -> AnyElement {
    row("jar-history-count")
        .child(metric(
            "jar-history-snapshots",
            "Snapshots",
            len.to_string(),
            "A spread is recorded when an applied save changes it".to_owned(),
            Tone::Info,
        ))
        .into_any_element()
}

/// Every found collectible's current level, beside the first snapshot's.
fn current_spread(jar: &Jar, snapshots: &[EnchantSnapshot], cx: &App) -> AnyElement {
    let first = &snapshots[0];
    let last = snapshots.last().expect("a non-empty history has a last");
    let cols = [
        Col::Text("Collectible"),
        Col::Number("Current"),
        Col::Number("First snapshot"),
        Col::Number("Change"),
    ];
    let rows = (0..COLLECTIBLES)
        .filter(|index| jar.collectible(*index).is_some_and(|item| item.level >= 1))
        .map(|index| {
            let current = last.level(index);
            let started = first.level(index);
            vec![
                div()
                    .id(format!("jar-history-spread-{index}"))
                    .child(collectible_name(index))
                    .into_any_element(),
                text(format!("+{current}")),
                text(format!("+{started}")),
                text(match current.cmp(&started) {
                    std::cmp::Ordering::Greater => format!("+{}", current - started),
                    std::cmp::Ordering::Less => format!("-{}", started - current),
                    std::cmp::Ordering::Equal => "—".to_owned(),
                }),
            ]
        });
    Card::new()
        .header(CardHeader::new("Current Spread").description(
            "Enchant levels above the collectible's own level, against the first snapshot.",
        ))
        .child(table("jar-history-spread", &cols, rows, cx))
        .into_any_element()
}

/// The most recent snapshots, newest first, with each one's change.
fn recent(snapshots: &[EnchantSnapshot], cx: &App) -> AnyElement {
    let start = snapshots.len().saturating_sub(RECENT);
    let rows = snapshots
        .iter()
        .enumerate()
        .skip(start)
        .rev()
        .map(|(index, snapshot)| {
            let previous = index.checked_sub(1).map(|previous| &snapshots[previous]);
            let delta = match previous {
                Some(previous) => i64::from(snapshot.total) - i64::from(previous.total),
                None => 0,
            };
            vec![
                div()
                    .id(format!("jar-history-row-{index}"))
                    .child(time_label(snapshot.at))
                    .into_any_element(),
                text(snapshot.total.to_string()),
                text(match delta {
                    0 => "—".to_owned(),
                    positive if positive > 0 => format!("+{positive}"),
                    negative => negative.to_string(),
                }),
            ]
        });
    Card::new()
        .header(CardHeader::new("Recent Snapshots").description(format!(
            "The newest {} of {} snapshots.",
            snapshots.len().min(RECENT),
            snapshots.len()
        )))
        .child(table(
            "jar-history-recent",
            &[
                Col::Text("Time"),
                Col::Number("Total levels"),
                Col::Number("Δ"),
            ],
            rows,
            cx,
        ))
        .into_any_element()
}
