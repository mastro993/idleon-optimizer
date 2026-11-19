//! The Monument (W5) section: the Bravery, Justice, Wisdom and Sources tabs and
//! the training sources every monument shares.

mod actions;
mod controls;
mod table;

pub(crate) mod bravery;
pub(crate) mod justice;
pub(crate) mod sources;
pub(crate) mod wisdom;

pub(crate) use controls::Controls;

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, v_flex};
use idleon_monument::{BonusRow, Milestone};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, PANEL_GAP, Statistic};
use table::{Col, table, text};

/// The monument tab strip.
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
}

/// The monument section's live inputs; the save supplies the rest.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    pub(crate) tab: Tab,
    /// The character whose Summoning access the Meritoc charge reads.
    pub(crate) character: usize,
    /// The enemy number, 1 for the first enemy; the zero-based wave is one less.
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
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if let Some(save) = state.save.as_ref() {
        let count = save.character_count();
        if count > 0 && state.monument.character >= count {
            state.monument.character = count - 1;
        }
    }
    state.sync_monument(cx);
    let summary = state
        .monument_controls
        .derived
        .snapshot
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
    let muted = cx.theme().muted_foreground;
    let mut header = div()
        .id("monument-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(tab_bar(state, cx))
        .child(div().flex_1());
    if let Some(select) = character_select(state, window, cx) {
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
        .child(pane(state, window, cx))
        .into_any_element()
}

/// The character the Summoning access and Charge read, as a page-level
/// selector.
fn character_select(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> Option<AnyElement> {
    let controls = &state.monument_controls;
    let characters = &controls.derived.characters;
    if characters.is_empty() {
        return None;
    }
    controls.character.show(
        characters.clone(),
        Some(&state.monument.character),
        window,
        cx,
    );
    Some(
        div()
            .flex_none()
            .w(px(180.0))
            .child(
                Select::new(controls.character.state())
                    .id("monument-character")
                    .accessibility_label("Character")
                    .small(),
            )
            .into_any_element(),
    )
}

fn tab_bar(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let selected = Tab::ALL
        .iter()
        .position(|tab| *tab == state.monument.tab)
        .unwrap_or_default();
    let tabs = Tab::ALL.map(|tab| (tab.id().into(), tab.label().into()));
    crate::ui::layout::tab_bar(
        "monument-tabs",
        tabs,
        selected,
        move |index, _window, cx| {
            entity.update(cx, |state, cx| state.set_monument_tab(Tab::ALL[index], cx));
        },
    )
}

fn pane(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    match state.monument.tab {
        Tab::Bravery => bravery::pane(state, window, cx),
        Tab::Justice => justice::pane(state, window, cx),
        Tab::Wisdom => wisdom::pane(state, cx),
        Tab::Sources => sources::pane(state, cx),
    }
}

/// A tab's scrolling body under `pane_id`, its scroll area named `scroll_id`.
pub(crate) fn scroll_pane(
    pane_id: &'static str,
    scroll_id: &'static str,
    body: impl IntoElement,
) -> AnyElement {
    div()
        .id(pane_id)
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(body)
                .overflow_y_scrollbar()
                .id(scroll_id),
        )
        .into_any_element()
}

/// The card an empty tab shows before a save is imported.
pub(crate) fn empty_card(
    title: &'static str,
    detail: &'static str,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(CardHeader::new(title))
        .child(
            div().id("monument-load").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new()
                                    .child(Icon::new(IconName::Upload).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("Load a save"))
                            .description(EmptyDescription::new().child(detail)),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("monument-import")
                                .label("Import save")
                                .icon(IconName::Upload)
                                .on_click(move |_event, window, cx| {
                                    entity.update(cx, |state, cx| state.open_import(window, cx));
                                }),
                        ),
                    ),
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
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(CardHeader::new(title).description("The save has no Monument rows."))
        .child(Caption::new(
            "This save is missing Monument charge, training or reward data.",
        ))
        .children(
            notes
                .iter()
                .map(|note| Caption::new(note.clone()).text_color(muted)),
        )
        .into_any_element()
}

/// One rejected input, under the card that read it.
pub(crate) fn error_line(
    id: &'static str,
    message: String,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let danger = cx.theme().danger;
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
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(CardHeader::new("Partial save").description("Some sources cannot be read"))
        .child(
            v_flex()
                .id(id)
                .role(Role::Label)
                .aria_label(notes.join(" "))
                .gap_1()
                .children(
                    notes
                        .iter()
                        .map(|note| Caption::new(note.clone()).text_color(muted)),
                ),
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
pub(crate) fn milestones_card(
    rows: &[Milestone],
    first: MilestoneFirst,
    cx: &gpui::App,
) -> AnyElement {
    let cols = [
        Col::Text("Milestone"),
        Col::Number("Training hours required"),
        Col::Text("Saved status"),
        Col::Text("One-character ETA"),
    ];
    let cells = rows.iter().enumerate().map(|(index, row)| {
        vec![
            text(milestone_text(index, row, first)),
            text(format::value(row.hours)),
            text(milestone_status(row)),
            text(milestone_eta(row)),
        ]
    });
    Card::new()
        .header(CardHeader::new("Paying Respect milestones"))
        .child(table("monument-milestone", &cols, cells, cx))
        .into_any_element()
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
pub(crate) fn bonuses_card(rows: &[BonusRow], cx: &gpui::App) -> AnyElement {
    let cols = [
        Col::Text("Reward"),
        Col::Number("Saved level"),
        Col::Text("Current effect"),
        Col::Text("Effect at +1 level"),
    ];
    let cells = rows.iter().map(|row| {
        let label = clean_text(&row.text);
        vec![
            text(bonus_label(&label)),
            text(format::value(row.level)),
            text(effect_text(&label, row.value)),
            text(effect_text(&label, row.next)),
        ]
    });
    Card::new()
        .header(CardHeader::new("Permanent reward levels & total effects"))
        .child(table("monument-bonus", &cols, cells, cx))
        .into_any_element()
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

/// One figure in a strip: the element id the harness reads, its title, the
/// formatted value and the line under it.
pub(crate) struct Metric {
    pub(crate) id: &'static str,
    pub(crate) title: String,
    pub(crate) value: String,
    pub(crate) sub: String,
}

impl Metric {
    pub(crate) fn new(
        id: &'static str,
        title: impl Into<String>,
        value: impl Into<String>,
        sub: impl Into<String>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            value: value.into(),
            sub: sub.into(),
        }
    }
}

/// Figures in strips of at most four under `id`, one strip per line so a
/// strip never wraps, and the lines as even as they can be (six figures are
/// two strips of three). Each figure keeps its id and reads
/// `title: value. sub`.
pub(crate) fn metrics(id: &'static str, metrics: Vec<Metric>) -> AnyElement {
    let lines = metrics.len().div_ceil(4).max(1);
    let per_line = metrics.len().div_ceil(lines).max(1);
    let mut strips = v_flex().id(id).gap(PANEL_GAP);
    let mut metrics = metrics.into_iter().peekable();
    while metrics.peek().is_some() {
        strips = strips.child(
            KpiStrip::new().children(metrics.by_ref().take(per_line).map(|metric| {
                div()
                    .id(metric.id)
                    .role(Role::Label)
                    .aria_label(format!(
                        "{}: {}. {}",
                        metric.title, metric.value, metric.sub
                    ))
                    .child(
                        Statistic::new(format!("{}-figure", metric.id), metric.title, metric.value)
                            .caption(metric.sub),
                    )
            })),
        );
    }
    strips.into_any_element()
}

/// Underscores and pipes read as spaces.
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
    fn reward_labels_and_effects_follow_the_text_cleanup() {
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
