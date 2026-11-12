//! The Monument (W5) section: the Bravery, Justice and Wisdom tabs and the
//! training sources every monument shares.

mod actions;

pub(crate) mod bravery;
pub(crate) mod justice;
pub(crate) mod wisdom;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled,
    Window, div, px,
};
use idleon_monument::{BonusRow, Milestone, Snapshot, Summary};
use idleon_ui::format;

/// The upstream monument tab strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    #[default]
    Bravery,
    Justice,
    Wisdom,
    Sources,
}

impl Tab {
    pub(crate) const ALL: [Tab; 4] = [Tab::Bravery, Tab::Justice, Tab::Wisdom, Tab::Sources];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Bravery => "bravery",
            Tab::Justice => "justice",
            Tab::Wisdom => "wisdom",
            Tab::Sources => "sources",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Bravery => "Bravery",
            Tab::Justice => "Justice",
            Tab::Wisdom => "Wisdom",
            Tab::Sources => "Sources",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The monument section's live inputs; the save supplies the rest.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    pub(crate) tab: Tab,
    /// The character whose Summoning access the Meritoc charge reads.
    pub(crate) character: usize,
    /// The enemy number, 1 for the first enemy; the upstream wave is one less.
    pub(crate) wave: u32,
    /// Damage-boost stacks already earned this story.
    pub(crate) boosts: u32,
    /// Rethrows left; `None` uses the saved maximum.
    pub(crate) rerolls: Option<u32>,
    /// HP before the last sword, when the player knows it.
    pub(crate) remaining_hp: Option<f64>,
    /// The Justice worksheet: its stats, its observed case and its undo trail.
    pub(crate) worksheet: idleon_monument::justice::Worksheet,
    /// What the last Justice step reported, under the advisor.
    #[serde(skip)]
    pub(crate) notice: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::Bravery,
            character: 0,
            wave: 1,
            boosts: 0,
            rerolls: None,
            remaining_hp: None,
            worksheet: idleon_monument::justice::Worksheet::default(),
            notice: String::new(),
        }
    }
}

/// The section: tab strip, training summary and the active pane.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if let Some(save) = state.save.as_ref() {
        let count = save.character_count();
        if count > 0 && state.monument.character >= count {
            state.monument.character = count - 1;
        }
    }
    let snapshot = state.monument_snapshot();
    let summary = snapshot
        .as_ref()
        .filter(|snapshot| snapshot.available)
        .map(|snapshot| {
            format!(
                "{}x training · {} hours per 24h claim",
                format::value(snapshot.training.rate),
                format::value(snapshot.training.per_day)
            )
        })
        .unwrap_or_else(|| "Import a save to calculate".into());
    let muted = cx.theme().colors.fg_muted;
    let mut header = div()
        .id("monument-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(tab_bar(state, cx))
        .child(div().flex_1());
    if let Some(select) = character_select(state, cx) {
        header = header.child(select);
    }
    let header = header.child(
        div()
            .id("monument-summary")
            .role(Role::Label)
            .aria_label(summary.clone())
            .child(Caption::new(summary).text_color(muted)),
    );

    div()
        .id("monument-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(pane(state, cx, snapshot.as_ref()))
        .into_any_element()
}

/// The character the Summoning access and Charge read, as the upstream page's
/// page-level selector.
fn character_select(state: &AppState, cx: &mut Context<AppState>) -> Option<AnyElement> {
    let save = state.save.as_ref()?;
    let count = save.character_count();
    if count == 0 {
        return None;
    }
    let characters: Vec<Choice> = (0..count)
        .map(|index| Choice::new(index.to_string(), save.character_name(index)))
        .collect();
    let entity = cx.entity();
    Some(
        Select::new("monument-character", characters)
            .label("Character")
            .selected(state.monument.character.to_string())
            .size(ControlSize::Sm)
            .on_change(move |value, _window, cx| {
                if let Ok(index) = value.parse::<usize>() {
                    entity.update(cx, |state, cx| state.set_monument_character(index, cx));
                }
            })
            .into_any_element(),
    )
}

fn tab_bar(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let mut tabs =
        TabBar::new("monument-tabs").selected(SharedString::from(state.monument.tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_monument_tab(tab, cx));
        }
    })
}

fn pane(
    state: &mut AppState,
    cx: &mut Context<AppState>,
    snapshot: Option<&Snapshot>,
) -> AnyElement {
    match state.monument.tab {
        Tab::Bravery => bravery::pane(state, snapshot, cx),
        Tab::Justice => {
            let justice = state.monument_justice();
            justice::pane(state, justice.as_ref(), cx)
        }
        Tab::Wisdom => {
            let wisdom = state.monument_wisdom();
            wisdom::pane(state, wisdom.as_ref(), cx)
        }
        Tab::Sources => {
            let summary = state.monument_summary();
            sources_pane(summary.as_ref(), cx)
        }
    }
}

/// The permanent reward totals, next-run charges, the training pool and both
/// source lists every tab shares.
fn sources_pane(summary: Option<&Summary>, cx: &mut Context<AppState>) -> AnyElement {
    let body = match summary {
        Some(summary) if summary.available => loaded_sources(summary, cx),
        Some(summary) => unavailable_card("Bonus sources", &summary.notes, cx),
        None => empty_card(
            "Bonus sources",
            "Import a decoded save to read the permanent rewards, the next-run charge and the training pool.",
            cx,
        ),
    };
    div()
        .id("monument-pane-sources")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            ScrollArea::new("monument-sources-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(body),
        )
        .into_any_element()
}

fn loaded_sources(summary: &Summary, cx: &mut Context<AppState>) -> AnyElement {
    let mut permanent = div()
        .id("monument-permanent-summary")
        .flex()
        .flex_wrap()
        .gap_3();
    for (index, monument) in summary.monuments.iter().enumerate() {
        permanent = permanent.child(metric_card(
            [
                "monument-permanent-bravery",
                "monument-permanent-justice",
                "monument-permanent-wisdom",
            ][index],
            &monument.name,
            format!("{} levels", format::value(monument.levels)),
            "Sum of its ten saved permanent reward levels".to_string(),
            cx,
        ));
    }
    permanent = permanent.child(metric_card(
        "monument-reward-multi",
        "Reward-level multiplier",
        format!("{}x", format::value(summary.reward_level_multi)),
        "Companion bonus; applied after each reward roll".to_string(),
        cx,
    ));

    let mut charge = div()
        .id("monument-charge-summary")
        .flex()
        .flex_wrap()
        .gap_3();
    for (index, monument) in summary.monuments.iter().enumerate() {
        let ready = if monument.charge.can_start {
            "story ready"
        } else {
            "needs at least 1x to start"
        };
        charge = charge.child(metric_card(
            [
                "monument-charge-bravery",
                "monument-charge-justice",
                "monument-charge-wisdom",
            ][index],
            &monument.name,
            format!("{}x", format::value(monument.charge.multi)),
            format!(
                "{} real hours waited; {ready}. After another 24h: {}x.",
                format::value(monument.charge.seconds / 3600.0),
                format::value(monument.tomorrow.multi),
            ),
            cx,
        ));
    }

    let training = &summary.training;
    let training_cards = div()
        .id("monument-training-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(metric_card(
            "monument-training-rate",
            "One-character training rate",
            format!("{}x", format::value(training.rate)),
            "Training hours per real hour, before claim flooring".to_string(),
            cx,
        ))
        .child(metric_card(
            "monument-training-day",
            "Training per 24-hour claim",
            format!("{} hours", format::value(training.per_day)),
            "One character Paying Respect for the full claim".to_string(),
            cx,
        ))
        .child(metric_card(
            "monument-training-additive",
            "Additive training gain",
            format!("+{}%", format::value(training.pct)),
            "All integrated Monument AFK sources".to_string(),
            cx,
        ))
        .child(metric_card(
            "monument-training-multi",
            "Clutch Overtime Block",
            format!("{}x", format::value(training.multi)),
            "Saved Minehead reward".to_string(),
            cx,
        ));

    let mut pool = Table::new(["Source", "Bonus"]);
    let mut total = 0.0;
    for source in &training.sources {
        total += source.value;
        pool = pool.row([
            div()
                .min_w_0()
                .child(source.name.clone())
                .into_any_element(),
            div()
                .child(format!("+{}%", format::value(source.value)))
                .into_any_element(),
        ]);
    }
    pool = pool.row([
        div()
            .font_weight(FontWeight::SEMIBOLD)
            .child("Total additive pool")
            .into_any_element(),
        div()
            .id("monument-sources-total")
            .role(Role::Label)
            .aria_label(format!("+{}%", format::value(total)))
            .child(format!("+{}%", format::value(total)))
            .into_any_element(),
    ]);

    let mut factors = Table::new(["Monument", "Linear window", "Meritoc 7", "Legend 27"]);
    for monument in &summary.monuments {
        factors = factors.row([
            div().child(monument.name.clone()).into_any_element(),
            div()
                .child(format!(
                    "{} days",
                    format::value(monument.charge.max_linear / 86_400.0)
                ))
                .into_any_element(),
            div()
                .child(format!("+{}%", format::value(monument.charge.meritoc)))
                .into_any_element(),
            div()
                .child(format!("+{}%", format::value(monument.charge.legend)))
                .into_any_element(),
        ]);
    }

    let muted = cx.theme().colors.fg_muted;
    let mut column = div()
        .id("monument-sources-body")
        .flex()
        .flex_col()
        .gap_3()
        .child(group_title("monument-permanent-title", "Permanent rewards"))
        .child(permanent)
        .child(group_title("monument-charge-title", "Next-run reward charge"))
        .child(charge)
        .child(Caption::new(
            "Charge grows with real waiting time, not Paying Respect training. Starting a normal story consumes it. After the linear window it keeps growing, more slowly. Run performance determines the levels earned; there is no fixed levels/day conversion.",
        ))
        .child(group_title("monument-training-title", "Paying Respect training"))
        .child(training_cards)
        .child(Caption::new(
            "Training hours strengthen minigame stats and unlock milestones. They are not permanent reward levels and do not speed up reward charge. The daily figure is one character's single 24-hour claim; claims must exceed 3,599 seconds and round down.",
        ))
        .child(
            Card::new()
                .header(CardHeader::new("Paying Respect: additive pool").description(format!(
                    "{} named sources sum to the additive training gain",
                    training.sources.len()
                )))
                .child(pool),
        )
        .child(
            Card::new()
                .header(CardHeader::new("Reward-charge factors (separate from training)"))
                .child(factors)
                .child(Caption::new(
                    "The linear window includes both storage schematics, Monumental Gaming superbit, Legend 27, and Justice study ownership. Waiting beyond it still contributes diminished charge.",
                ).text_color(muted)),
        );
    if !summary.notes.is_empty() {
        column = column.child(notes_card("monument-sources-notes", &summary.notes, cx));
    }
    column.into_any_element()
}

/// A group heading above its cards, as the upstream `h2` does.
fn group_title(id: &'static str, text: &'static str) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(text)
        .text_size(px(15.0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
        .into_any_element()
}

/// The card an empty tab shows before a save is imported.
pub(crate) fn empty_card(
    title: &'static str,
    detail: &'static str,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    Card::new()
        .header(CardHeader::new(title))
        .child(
            EmptyState::new("monument-load", IconName::Upload, "Load a save")
                .body(detail)
                .action(
                    Button::new("monument-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            entity.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}

/// The card a tab shows when the save has no Monument rows.
pub(crate) fn unavailable_card(
    title: &'static str,
    notes: &[String],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut card = Card::new()
        .header(CardHeader::new(title).description("The save has no Monument rows."))
        .child(Caption::new(
            "This save is missing Monument charge, training or reward data.",
        ));
    if !notes.is_empty() {
        card = card.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .children(notes.iter().map(|note| {
                    div()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(note.clone())
                })),
        );
    }
    card.into_any_element()
}

/// One rejected input, under the card that read it.
pub(crate) fn error_line(
    id: &'static str,
    message: String,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let danger = cx.theme().colors.danger;
    div()
        .id(id)
        .role(Role::Alert)
        .text_color(danger)
        .child(message)
        .into_any_element()
}

/// The partial-save warnings a loaded tab shows.
pub(crate) fn notes_card(
    id: &'static str,
    notes: &[String],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    Card::new()
        .header(CardHeader::new("Partial save").description("Some sources cannot be read"))
        .child(
            div()
                .id(id)
                .role(Role::Label)
                .aria_label(notes.join(" "))
                .flex()
                .flex_col()
                .gap_1()
                .children(notes.iter().map(|note| {
                    div()
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(note.clone())
                })),
        )
        .into_any_element()
}

/// What the first milestone row's placeholder is replaced with: Bravery prints
/// its damage range, Justice its starting coins, Wisdom its starting attempts.
#[derive(Clone, Copy, Debug)]
pub(crate) enum MilestoneFirst {
    Damage { min: f64, max: f64 },
    Coins(f64),
    Attempts(f64),
}

/// One monument's eight Paying Respect milestones.
pub(crate) fn milestones_card(rows: &[Milestone], first: MilestoneFirst) -> AnyElement {
    let mut table = Table::new([
        "Milestone",
        "Training hours required",
        "Saved status",
        "One-character ETA",
    ]);
    for (index, row) in rows.iter().enumerate() {
        table = table.row(milestone_row(index, row, first));
    }
    Card::new()
        .header(CardHeader::new("Paying Respect milestones"))
        .child(table)
        .into_any_element()
}

fn milestone_row(index: usize, row: &Milestone, first: MilestoneFirst) -> [AnyElement; 4] {
    [
        div()
            .min_w_0()
            .child(milestone_text(index, row, first))
            .into_any_element(),
        div().child(format::value(row.hours)).into_any_element(),
        div().child(milestone_status(row)).into_any_element(),
        div().child(milestone_eta(row)).into_any_element(),
    ]
}

/// The row's text, with the first row's placeholder filled in.
fn milestone_text(index: usize, row: &Milestone, first: MilestoneFirst) -> String {
    let text = clean_text(&row.text);
    if index != 0 {
        return text;
    }
    match first {
        MilestoneFirst::Damage { min, max } => text
            .replace('{', &format::value(min))
            .replace('}', &format::value(max)),
        MilestoneFirst::Coins(coins) => text.replacen('$', &format::value(coins), 1),
        MilestoneFirst::Attempts(attempts) => text.replacen('#', &format::value(attempts), 1),
    }
}

pub(crate) fn milestone_status(row: &Milestone) -> &'static str {
    if row.unlocked {
        "Unlocked"
    } else if row.ready {
        "Ready: unlock in game"
    } else {
        "Not reached"
    }
}

pub(crate) fn milestone_eta(row: &Milestone) -> String {
    if row.unlocked || row.ready {
        "-".to_string()
    } else {
        format!("{} real hours", format::value(row.real_hours))
    }
}

/// A monument's ten permanent reward levels and their effects.
pub(crate) fn bonuses_card(rows: &[BonusRow]) -> AnyElement {
    let mut table = Table::new([
        "Reward",
        "Saved level",
        "Current effect",
        "Effect at +1 level",
    ]);
    for row in rows {
        table = table.row(bonus_row(row));
    }
    Card::new()
        .header(CardHeader::new("Permanent reward levels & total effects"))
        .child(table)
        .into_any_element()
}

fn bonus_row(row: &BonusRow) -> [AnyElement; 4] {
    let text = clean_text(&row.text);
    [
        div().min_w_0().child(bonus_label(&text)).into_any_element(),
        div().child(format::value(row.level)).into_any_element(),
        div()
            .child(effect_text(&text, row.value))
            .into_any_element(),
        div().child(effect_text(&text, row.next)).into_any_element(),
    ]
}

fn bonus_label(text: &str) -> String {
    let mut label: String = text
        .chars()
        .filter(|character| !matches!(character, '+' | '{' | '}' | '%'))
        .collect();
    label = label.trim().to_string();
    if let Some(rest) = label.strip_prefix("x ") {
        label = rest.trim().to_string();
    }
    label
}

fn effect_text(text: &str, value: f64) -> String {
    text.replace('{', &format::value(value))
        .replace('}', &format::value(1.0 + value / 100.0))
}

/// A metric card with a formatted value, matching the upstream cards.
pub(crate) fn metric_card(
    id: &'static str,
    title: &str,
    value: String,
    sub: String,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    div()
        .flex_1()
        .min_w(px(180.0))
        .child(
            Card::new().header(CardHeader::new(title)).child(
                div()
                    .id(id)
                    .role(Role::Label)
                    .aria_label(format!("{title}: {value}. {sub}"))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(accent)
                            .child(value),
                    )
                    .child(Caption::new(sub).text_color(muted)),
            ),
        )
        .into_any_element()
}

/// Upstream `clean`: underscores and pipes read as spaces.
pub(crate) fn clean_text(text: &str) -> String {
    text.replace(['_', '|'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn milestone(unlocked: bool, ready: bool, real_hours: f64) -> Milestone {
        Milestone {
            text: String::new(),
            hours: 0.0,
            unlocked,
            ready,
            hours_needed: 0.0,
            real_hours,
        }
    }

    #[test]
    fn reward_labels_and_effects_follow_the_upstream_cleanup() {
        assert_eq!(bonus_label("+{% Mining Efficiency"), "Mining Efficiency");
        assert_eq!(bonus_label("x Bucket Fill Rate"), "Bucket Fill Rate");
        assert_eq!(
            bonus_label("Chance for Bravery Opals"),
            "Chance for Bravery Opals"
        );
        assert_eq!(
            effect_text("+{% Mining Efficiency", 63_443.52),
            "+63.4K% Mining Efficiency"
        );
        assert_eq!(
            effect_text("}x Bucket Fill Rate", 108_812.16),
            "1.1Kx Bucket Fill Rate"
        );
    }

    #[test]
    fn milestone_status_and_eta_read_the_saved_row() {
        assert_eq!(milestone_status(&milestone(true, true, 0.0)), "Unlocked");
        assert_eq!(
            milestone_status(&milestone(false, true, 0.0)),
            "Ready: unlock in game"
        );
        assert_eq!(
            milestone_status(&milestone(false, false, 12.0)),
            "Not reached"
        );
        assert_eq!(milestone_eta(&milestone(true, true, 0.0)), "-");
        assert_eq!(
            milestone_eta(&milestone(false, false, 12.0)),
            "12.0 real hours"
        );
    }

    #[test]
    fn the_first_milestone_reads_each_monuments_placeholder() {
        let row = Milestone {
            text: "Start_with_#_Attempts".into(),
            ..milestone(false, false, 0.0)
        };
        assert_eq!(
            milestone_text(0, &row, MilestoneFirst::Attempts(47.0)),
            "Start with 47.0 Attempts"
        );
        let damage = Milestone {
            text: "Swords_deal_{~}_damage".into(),
            ..row.clone()
        };
        assert_eq!(
            milestone_text(
                0,
                &damage,
                MilestoneFirst::Damage {
                    min: 3.0,
                    max: 25.0,
                },
            ),
            "Swords deal 3.0~25.0 damage"
        );
        // Only the first row takes the values; later rows keep their text.
        assert_eq!(
            milestone_text(1, &row, MilestoneFirst::Attempts(47.0)),
            "Start with # Attempts"
        );
    }
}
