//! The Bonuses tab: every obstruction reward, the account system it feeds and
//! whether the save has cleared the obstruction that grants it.
//!
//! Ported from `bonusesHtml` in `jelly-operator-calc.html`: the page's 72-row
//! table, whose "corrected account bonus" column is `gameDescription` and whose
//! effect column is `rewardEffectText`. The page hides the table behind
//! Operational Bonuses (upgrade 39); the port shows the rows and says the
//! interface is still locked instead of printing a guessed bonus.

use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_jelly::Jelly;

/// The Bonuses pane.
pub(crate) fn pane(jelly: &Jelly, cx: &mut Context<AppState>) -> AnyElement {
    let bonuses = jelly.bonuses();
    let cleared = bonuses.iter().filter(|bonus| bonus.active).count();
    let muted = cx.theme().colors.fg_muted;
    let mut body = div().id("jelly-bonuses").flex().flex_col().gap_3().child(
        Card::new()
            .header(CardHeader::new("Obstruction rewards").description(format!(
                "{cleared} of {} rewards are active: obstruction {} of 72 cleared",
                bonuses.len(),
                jelly.progress.obstruction + 1
            )))
            .child(
                div().p_3().child(
                    Caption::new(
                        "Each cleared obstruction grants one account-wide bonus. The rewards are \
                         part of the saved account whichever tab the game shows them on.",
                    )
                    .text_color(muted),
                ),
            ),
    );
    if !jelly.bonuses_unlocked() {
        body = body.child(
            div().id("jelly-bonuses-warning").child(
                Callout::new(Severity::Warning)
                    .title("Operational Bonuses is not unlocked")
                    .child(Caption::new(format!(
                        "The game reveals this list with upgrade 39; the save's own rewards still \
                         apply to the account systems that read them. Cleared so far: {cleared}."
                    ))),
            ),
        );
    }
    let mut table = Table::new(["#", "Obstruction", "Target system", "Reward text", "Effect"]);
    for bonus in &bonuses {
        table = table.row([
            div()
                .id(format!("jelly-bonus-row-{}", bonus.index))
                .child(format!("{}", bonus.index + 1))
                .into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(bonus.name.clone())
                .child(div().flex().items_center().child(if bonus.active {
                    Badge::new("Active").tone(Tone::Success)
                } else {
                    Badge::new("Not cleared").tone(Tone::Neutral)
                }))
                .into_any_element(),
            div().child(bonus.target.clone()).into_any_element(),
            div()
                .child(Caption::new(bonus.description.clone()).text_color(muted))
                .into_any_element(),
            div().child(bonus.effect.clone()).into_any_element(),
        ]);
    }
    body.child(
        Card::new()
            .header(
                CardHeader::new("72 obstruction rewards")
                    .description("The reward each obstruction grants, in clear order"),
            )
            .child(div().id("jelly-bonuses-table").p_3().child(table)),
    )
    .into_any_element()
}
