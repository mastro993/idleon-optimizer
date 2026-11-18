//! The Bonuses tab: every obstruction reward, the account system it feeds and
//! whether the save has cleared the obstruction that grants it.
//!
//! A 72-row table, whose "corrected account bonus" column is the game's
//! description and whose effect column is its normalised effect text. The game
//! hides the bonuses behind Operational Bonuses (upgrade 39); the tab shows the
//! rows and says the interface is still locked instead of printing a guessed
//! bonus.
//!
//! The rows are the game's fixed 72 and are read once per import. Each holds
//! wrapped text and a tag, so they stay in a plain kit `Table` rather than the
//! uniform rows of a virtualized one.

use super::parts::{INDEX_WIDTH, cell, number_cell};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::table::{Table, TableBody, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use idleon_jelly::Jelly;
use idleon_ui::widgets::{Callout, Caption, Card, CardHeader, PANEL_GAP, Tone};

/// The Bonuses pane.
pub(crate) fn pane(state: &AppState, jelly: &Jelly) -> AnyElement {
    let bonuses = &state.jelly_state.save.bonuses;
    let cleared = bonuses.iter().filter(|bonus| bonus.active).count();
    let mut body = div()
        .id("jelly-bonuses")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(
            Card::new()
                .header(CardHeader::new("Obstruction rewards").description(format!(
                    "{cleared} of {} rewards are active: obstruction {} of 72 cleared",
                    bonuses.len(),
                    jelly.progress.obstruction + 1
                )))
                .child(Caption::new(
                    "Each cleared obstruction grants one account-wide bonus. The rewards are \
                 part of the saved account whichever tab the game shows them on.",
                )),
        );
    if !jelly.bonuses_unlocked() {
        body = body.child(div().id("jelly-bonuses-warning").child(
            Callout::new(Tone::Warning, "Operational Bonuses is not unlocked").child(Caption::new(
                format!(
                    "The game reveals this list with upgrade 39; the save's own rewards still \
                         apply to the account systems that read them. Cleared so far: {cleared}."
                ),
            )),
        ));
    }
    let table = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().w(INDEX_WIDTH).text_right().child("#"))
                    .child(TableHead::new().child("Obstruction"))
                    .child(TableHead::new().child("Target system"))
                    .child(TableHead::new().child("Reward text"))
                    .child(TableHead::new().child("Effect")),
            ),
        )
        .child(TableBody::new().children(bonuses.iter().map(|bonus| {
            let (status, tone) = if bonus.active {
                ("Active", Tone::Success)
            } else {
                ("Not cleared", Tone::Neutral)
            };
            TableRow::new()
                .child(
                    number_cell(
                        div()
                            .id(format!("jelly-bonus-row-{}", bonus.index))
                            .child(format!("{}", bonus.index + 1)),
                    )
                    .w(INDEX_WIDTH),
                )
                .child(cell(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(bonus.name.clone())
                        .child(
                            div().flex().child(
                                Tag::new()
                                    .with_variant(tone.into())
                                    .outline()
                                    .small()
                                    .child(status),
                            ),
                        ),
                ))
                .child(cell(bonus.target.clone()))
                .child(cell(Caption::new(bonus.description.clone())))
                .child(cell(bonus.effect.clone()))
        })));
    body.child(
        Card::new()
            .header(
                CardHeader::new("72 obstruction rewards")
                    .description("The reward each obstruction grants, in clear order"),
            )
            .child(div().id("jelly-bonuses-table").child(table)),
    )
    .into_any_element()
}
