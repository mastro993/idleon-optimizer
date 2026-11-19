//! The Cooking Mastery section: its two tabs.
//!
//! CK-01 builds the Mastery EXP tab's three blocks: the overview figures
//! (mastery rank, free points, EXP rate, EXP to the next level and the hours
//! that takes), the upgrade comparison ordered by what one more point buys, and
//! the EXP rate breakdown in the shared formula tree. CK-02's daily
//! distribution sits between the comparison and the breakdown, and CK-03 adds
//! the page's second tab, the yellow point allocator.
//!
//! Every figure comes from the import; the only inputs are the distribution's
//! goal and budget and the allocator's goal, budget and character, which live in
//! [`State`]. The kit controls and the planned figures live in [`Controls`].

mod controls;
mod distribution;
mod state;
mod yellow;

pub(crate) use controls::Controls;
pub(crate) use state::State;

use crate::state::AppState;
use crate::ui::layout;
use controls::Derived;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Role, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use idleon_cooking_mastery::{Mastery, RIBBON_CHANCE, UPGRADE_NAMES, Upgrade};
use idleon_ui::format;
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, KpiStrip, PANEL_GAP, PANEL_PADDING, Statistic, Tone,
};

/// The page's two tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Tab {
    /// The mastery figures, the recommendation and the daily distribution.
    #[default]
    Exp,
    /// The yellow point allocator.
    Yellow,
}

impl Tab {
    pub(crate) const ALL: [Tab; 2] = [Tab::Exp, Tab::Yellow];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Exp => "exp",
            Tab::Yellow => "yellow",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Exp => "Mastery EXP",
            Tab::Yellow => "Yellow Points",
        }
    }
}

/// The section: the tab strip and its pane.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    state.sync_mastery(cx);
    let (Some(mastery), Some(derived)) = (&state.mastery, &state.mastery_controls.derived) else {
        return prompt(cx);
    };
    let tab = state.mastery_plan.tab;
    let entity = cx.entity();
    let tabs = layout::tab_bar(
        "cooking-mastery-tabs",
        Tab::ALL.map(|tab| {
            (
                format!("cooking-mastery-tab-{}", tab.id()).into(),
                tab.label().into(),
            )
        }),
        Tab::ALL.iter().position(|entry| *entry == tab).unwrap_or(0),
        move |index, _window, cx| {
            entity.update(cx, |state, cx| {
                state.mastery_plan.tab = Tab::ALL[index];
                cx.notify();
            });
        },
    );
    let pane = match tab {
        Tab::Exp => exp_pane(state, mastery, derived, window, cx),
        Tab::Yellow => yellow::pane(state, derived, window, cx),
    };
    v_flex()
        .id("cooking-mastery-section")
        .size_full()
        .min_h_0()
        .gap_3()
        .child(
            div()
                .id("cooking-mastery-tab-strip")
                .flex_none()
                .child(tabs),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(pane)
                .overflow_y_scrollbar()
                .id("cooking-mastery-scroll"),
        )
        .into_any_element()
}

/// The Mastery EXP tab: the summary, the recommendation, the comparison, the
/// daily distribution and the EXP rate breakdown.
fn exp_pane(
    state: &AppState,
    mastery: &Mastery,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    v_flex()
        .id("cooking-mastery-pane")
        .gap(PANEL_GAP)
        .child(summary(mastery))
        .child(recommendation(mastery, &derived.ranking, cx))
        .child(comparison(&derived.ranking, cx))
        .child(distribution::card(state, &derived.plan, window, cx))
        .child(formula_card(state.mastery_breakdown.clone()))
        .into_any_element()
}

/// The figures the page leads with: rank, points, rate, requirement and hours.
fn summary(mastery: &Mastery) -> AnyElement {
    let rate = mastery.exp_rate();
    let (rate_value, rate_suffix) = format::scale(rate);
    let (exp_value, exp_suffix) = format::scale(mastery.exp_required());
    let label = format!(
        "Mastery level {} · {} points available · {} EXP per hour · {} EXP to the next level",
        whole(mastery.level),
        whole(mastery.points_available()),
        format::rate(rate),
        format::number(mastery.exp_required()),
    );
    let hours = mastery.hours_to_next_level();
    div()
        .id("cooking-mastery-summary")
        .role(Role::Label)
        .aria_label(label)
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "cooking-mastery-kpi-level",
                        "Mastery Level",
                        whole(mastery.level),
                    )
                    .caption(format!(
                        "rank {} · {} ribbons",
                        whole(mastery.level),
                        format::count(mastery.ribbons as u64)
                    )),
                )
                .child(
                    Statistic::new(
                        "cooking-mastery-kpi-points",
                        "Points Available",
                        whole(mastery.points_available()),
                    )
                    .caption(format!(
                        "{} spent of the {} earned",
                        whole(mastery.points_spent()),
                        whole(mastery.points_earned())
                    )),
                )
                .child(
                    Statistic::new(
                        "cooking-mastery-kpi-rate",
                        "EXP Rate",
                        format!("{rate_value:.2}{rate_suffix}"),
                    )
                    .suffix("/hr")
                    .caption(format!(
                        "{}× external",
                        format::value(mastery.external.total())
                    )),
                )
                .child(
                    Statistic::new(
                        "cooking-mastery-kpi-exp",
                        "EXP to Next Lv",
                        format!("{exp_value:.2}{exp_suffix}"),
                    )
                    .caption("100 × 2.5^lv × 5^max(0, lv − 40)"),
                )
                .child(
                    Statistic::new("cooking-mastery-kpi-hours", "Hrs to Next Lv", span(hours))
                        .caption(format!("at {}", format::rate(rate))),
                ),
        )
        .into_any_element()
}

/// The page's recommendation box: where the next point should go.
///
/// A save with nothing left to spend gets the page's "No points available"
/// notice instead, under the same node so the harness reads one place either
/// way.
fn recommendation(
    mastery: &Mastery,
    ranking: &[Upgrade],
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let available = mastery.points_available();
    let best = ranking.first().filter(|_| available > 0.0);
    let (tone, title, detail, label) = match best {
        Some(best) => {
            let next_level = best.level + 1.0;
            (
                Tone::Success,
                format!("Next point → {}", best.name),
                format!(
                    "+{:.4}% EXP rate · Lv {} → {} · {} now, {} after · {} of {} points free",
                    best.gain_pct,
                    whole(best.level),
                    whole(next_level),
                    format::rate(mastery.exp_rate()),
                    format::rate(best.rate_after),
                    whole(available),
                    whole(mastery.level + 1.0),
                ),
                format!(
                    "Next point: {} · plus {:.4}% EXP rate · Lv {} to {}",
                    best.name,
                    best.gain_pct,
                    whole(best.level),
                    whole(next_level),
                ),
            )
        }
        None => (
            Tone::Info,
            "No points available".to_string(),
            "Level up mastery to gain more points.".to_string(),
            format!(
                "No points available · {} of {} points spent",
                whole(mastery.points_spent()),
                whole(mastery.level + 1.0)
            ),
        ),
    };
    div()
        .id("cooking-mastery-recommendation")
        .role(Role::Label)
        .aria_label(label)
        .child(Callout::new(tone, title).child(Caption::new(detail).text_color(muted)))
        .into_any_element()
}

/// The upgrade comparison: every EXP upgrade by the gain one point brings.
fn comparison(ranking: &[Upgrade], cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let table = table(
        &[
            ("#", false),
            ("Upgrade", false),
            ("Level", true),
            ("Per Lv", true),
            ("Scaling", true),
            ("Current Bonus%", true),
            ("+1 Gain%", true),
        ],
        ranking.iter().enumerate().map(|(position, row)| {
            TableRow::new()
                .id(("cooking-mastery-comparison-row", row.index))
                .children(comparison_row(position, row))
        }),
    );
    Card::new()
        .header(
            CardHeader::new("Upgrade Comparison")
                .description("Every upgrade that feeds the EXP rate, ranked by one more point."),
        )
        .child(div().id("cooking-mastery-upgrades").child(table))
        .child(
            Caption::new(format!(
                "{} is excluded: it raises ribbon rank chance, not the EXP rate.",
                UPGRADE_NAMES[RIBBON_CHANCE]
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// One comparison row; the page marks the top gain and flags the locked rows.
fn comparison_row(position: usize, row: &Upgrade) -> [TableCell; 7] {
    let place = h_flex()
        .id(("cooking-mastery-place", row.index))
        .gap_1()
        .child((position + 1).to_string())
        .when(position == 0 && !row.locked, |place| {
            place.child(
                Tag::new()
                    .with_variant(Tone::Accent.into())
                    .outline()
                    .small()
                    .child("BEST"),
            )
        });
    let name = v_flex()
        .id(("cooking-mastery-upgrade", row.index))
        .gap_0p5()
        .child(row.name)
        .when(row.locked, |name| {
            name.child(Caption::new(format!(
                "needs mastery rank {}",
                row.required_level
            )))
        });
    [
        TableCell::new().child(place),
        text_cell(name),
        number_cell(whole(row.level)),
        number_cell(whole(row.per_lv)),
        number_cell(format!("{:.2}", row.scaling)),
        number_cell(format!("{:.2}%", row.bonus)),
        number_cell(format!("+{:.4}%", row.gain_pct)),
    ]
}

/// A level or a point count: the page prints these as whole numbers.
fn whole(value: f64) -> String {
    format!("{value:.0}")
}

/// A span of hours in the section's own style.
fn span(hours: f64) -> String {
    if hours.is_finite() {
        format::hours(hours)
    } else {
        "∞".into()
    }
}

/// A kit table with these `(title, numeric)` columns over `rows`. Number
/// columns sit right-aligned, as `docs/ui-kit/data.md` sets them.
fn table(columns: &[(&'static str, bool)], rows: impl IntoIterator<Item = TableRow>) -> Table {
    Table::new()
        .child(
            TableHeader::new().child(TableRow::new().children(columns.iter().map(
                |(title, numeric)| {
                    TableHead::new()
                        .when(*numeric, TableHead::text_right)
                        .child(*title)
                },
            ))),
        )
        .child(TableBody::new().children(rows))
}

/// A cell whose content wraps inside its column.
fn text_cell(content: impl IntoElement) -> TableCell {
    TableCell::new().child(div().min_w_0().child(content))
}

/// A right-aligned number cell.
fn number_cell(text: String) -> TableCell {
    TableCell::new().text_right().child(text)
}

/// The shared formula tree, sized like the other sections' breakdown cards.
fn formula_card(tree: Entity<FormulaTree>) -> impl IntoElement {
    Card::new()
        .header(
            CardHeader::new("EXP Rate Breakdown")
                .description("Search every factor and copy the full tree"),
        )
        .child(
            div()
                .id("cooking-mastery-formula-card-body")
                .h(px(260.0))
                .min_h_0()
                .child(tree),
        )
}

/// A save without `CookMaster` has no mastery state: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let muted = cx.theme().muted_foreground;
    v_flex()
        .id("cooking-mastery-section")
        .size_full()
        .min_h_0()
        .p(PANEL_PADDING)
        .child(
            div().id("cooking-mastery-empty").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("No Cooking Mastery data"))
                            .description(EmptyDescription::new().child(
                                "Cooking Mastery needs the Rift's mastery ranks: import a save \
                                 that has opened them to read the rank, the upgrades and the EXP \
                                 rate.",
                            )),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("cooking-mastery-import")
                                .label("Import save")
                                .icon(IconName::Upload)
                                .on_click(move |_event, window, cx| {
                                    import.update(cx, |state, cx| state.open_import(window, cx));
                                }),
                        ),
                    ),
            ),
        )
        .into_any_element()
}
