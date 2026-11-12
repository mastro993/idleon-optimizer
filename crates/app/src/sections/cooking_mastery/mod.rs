//! The Cooking Mastery section: the upstream page's two tabs.
//!
//! CK-01 builds the Mastery EXP tab's three blocks, ported from
//! `cooking-mastery-calc.html`: the overview figures (mastery rank, free points,
//! EXP rate, EXP to the next level and the hours that takes), the upgrade
//! comparison ordered by what one more point buys, and the EXP rate breakdown in
//! the shared formula tree. CK-02's daily distribution sits between the
//! comparison and the breakdown, and CK-03 adds the page's second tab, the
//! yellow point allocator.
//!
//! Every figure comes from the import; the only inputs are the distribution's
//! goal and budget and the allocator's goal, budget and character, which live in
//! [`State`], so the section keeps no state beyond them and the shared tree.

mod distribution;
mod state;
mod yellow;

pub(crate) use state::State;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::feedback::{Callout, EmptyState};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Role, SharedString, Styled, Window,
    div, px,
};
use idleon_cooking_mastery::{Mastery, RIBBON_CHANCE, UPGRADE_NAMES, Upgrade};
use idleon_ui::format;
use idleon_ui::formula::FormulaTree;

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

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The section: the tab strip and its pane.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(mastery) = state.mastery.clone() else {
        return prompt(cx);
    };
    let tab = state.mastery_plan.tab;
    let entity = cx.entity();
    let mut tabs = TabBar::new("cooking-mastery-tabs").selected(SharedString::from(format!(
        "cooking-mastery-tab-{}",
        tab.id()
    )));
    for entry in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(
            format!("cooking-mastery-tab-{}", entry.id()),
            entry.label(),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        let tab = id
            .as_ref()
            .strip_prefix("cooking-mastery-tab-")
            .and_then(Tab::from_id);
        if let Some(tab) = tab {
            let entity = entity.clone();
            entity.update(cx, |state, cx| {
                state.mastery_plan.tab = tab;
                cx.notify();
            });
        }
    });
    let strip = div()
        .id("cooking-mastery-tab-strip")
        .flex()
        .items_center()
        .px_3()
        .pt_2()
        .child(tabs);
    let pane = match tab {
        Tab::Exp => exp_pane(state, &mastery, cx),
        Tab::Yellow => yellow::pane(state, cx),
    };
    div()
        .id("cooking-mastery-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(strip)
        .child(
            ScrollArea::new("cooking-mastery-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(pane),
        )
        .into_any_element()
}

/// The Mastery EXP tab: the summary, the recommendation, the comparison, the
/// daily distribution and the EXP rate breakdown.
fn exp_pane(state: &AppState, mastery: &Mastery, cx: &mut Context<AppState>) -> AnyElement {
    let tree = state.mastery_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(mastery.breakdown(), cx));
    div()
        .id("cooking-mastery-pane")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(summary(mastery, cx))
        .child(recommendation(mastery, cx))
        .child(comparison(mastery, cx))
        .child(distribution::card(state, mastery, cx))
        .child(formula_card(tree))
        .into_any_element()
}

/// The figures the page leads with: rank, points, rate, requirement and hours.
fn summary(mastery: &Mastery, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let rate = mastery.exp_rate();
    let (rate_value, rate_suffix) = format::scale(rate);
    let label = format!(
        "Mastery level {} · {} points available · {} EXP per hour · {} EXP to the next level",
        whole(mastery.level),
        whole(mastery.points_available()),
        format::rate(rate),
        format::number(mastery.exp_required()),
    );
    let (exp_value, exp_suffix) = format::scale(mastery.exp_required());
    div()
        .id("cooking-mastery-summary")
        .role(Role::Label)
        .aria_label(label)
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new("cooking-mastery-kpi-level", "Mastery Level", mastery.level)
                    .decimals(0)
                    .size(TextSize::Xl),
            )
            .caption(format!(
                "rank {} · {} ribbons",
                whole(mastery.level),
                format::count(mastery.ribbons as u64)
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "cooking-mastery-kpi-points",
                    "Points Available",
                    mastery.points_available(),
                )
                .decimals(0)
                .size(TextSize::Xl),
            )
            .caption(format!(
                "{} spent of the {} earned",
                whole(mastery.points_spent()),
                whole(mastery.points_earned())
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new("cooking-mastery-kpi-rate", "EXP Rate", rate_value)
                    .decimals(2)
                    .suffix(SharedString::from(format!("{rate_suffix}/hr")))
                    .size(TextSize::Xl),
            )
            .caption(format!(
                "{}× external",
                format::value(mastery.external.total())
            )),
        )
        .child(
            KpiCard::new(
                Statistic::new("cooking-mastery-kpi-exp", "EXP to Next Lv", exp_value)
                    .decimals(2)
                    .suffix(SharedString::from(exp_suffix))
                    .size(TextSize::Xl),
            )
            .caption("100 × 2.5^lv × 5^max(0, lv − 40)"),
        )
        .child(hours_to_next_level(mastery, muted))
        .into_any_element()
}

/// The EXP requirement over the rate, in the research dashboard's time style.
fn hours_to_next_level(mastery: &Mastery, muted: gpui::Hsla) -> AnyElement {
    let hours = mastery.hours_to_next_level();
    let reading = if hours.is_finite() {
        format::hours(hours)
    } else {
        "∞".into()
    };
    div()
        .id("cooking-mastery-kpi-hours")
        .role(Role::Label)
        .aria_label(format!("Hours to the next level: {reading}"))
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(190.0))
        .flex_1()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(muted)
                .child("Hrs to Next Lv"),
        )
        .child(div().text_size(px(20.0)).child(SharedString::from(reading)))
        .child(Caption::new(format!("at {}", format::rate(mastery.exp_rate()))).text_color(muted))
        .into_any_element()
}

/// The page's recommendation box: where the next point should go.
///
/// A save with nothing left to spend gets the page's "No points available"
/// notice instead, under the same node so the harness reads one place either
/// way.
fn recommendation(mastery: &Mastery, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let available = mastery.points_available();
    let best = mastery
        .ranking()
        .into_iter()
        .next()
        .filter(|_| available > 0.0);
    let (severity, title, detail, label) = match best {
        Some(best) => {
            let next_level = best.level + 1.0;
            (
                Severity::Success,
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
            Severity::Info,
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
        .child(
            Callout::new(severity)
                .title(title)
                .child(Caption::new(detail).text_color(muted).into_any_element()),
        )
        .into_any_element()
}

/// The upgrade comparison: every EXP upgrade by the gain one point brings.
fn comparison(mastery: &Mastery, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new([
        "#",
        "Upgrade",
        "Level",
        "Per Lv",
        "Scaling",
        "Current Bonus%",
        "+1 Gain%",
    ]);
    for (position, row) in mastery.ranking().iter().enumerate() {
        table = table.row(comparison_row(position, row));
    }
    Card::new()
        .header(
            CardHeader::new("Upgrade Comparison")
                .description("Every upgrade that feeds the EXP rate, ranked by one more point."),
        )
        .child(div().id("cooking-mastery-upgrades").p_3().child(table))
        .child(
            div().px_3().pb_3().child(
                Caption::new(format!(
                    "{} is excluded: it raises ribbon rank chance, not the EXP rate.",
                    UPGRADE_NAMES[RIBBON_CHANCE]
                ))
                .text_color(muted),
            ),
        )
        .into_any_element()
}

/// One comparison row; the page marks the top gain and flags the locked rows.
fn comparison_row(position: usize, row: &Upgrade) -> [AnyElement; 7] {
    let mut place = div()
        .id(("cooking-mastery-place", row.index))
        .flex()
        .items_center()
        .gap_1()
        .child(SharedString::from((position + 1).to_string()));
    if position == 0 && !row.locked {
        place = place.child(Badge::new("BEST").tone(Tone::Accent));
    }
    let mut name = div()
        .id(("cooking-mastery-upgrade", row.index))
        .flex()
        .flex_col()
        .gap_0p5()
        .child(SharedString::from(row.name));
    if row.locked {
        name = name.child(div().text_size(px(11.0)).child(SharedString::from(format!(
            "needs mastery rank {}",
            row.required_level
        ))));
    }
    [
        place.into_any_element(),
        name.into_any_element(),
        cell(whole(row.level)),
        cell(whole(row.per_lv)),
        cell(format!("{:.2}", row.scaling)),
        cell(format!("{:.2}%", row.bonus)),
        cell(format!("+{:.4}%", row.gain_pct)),
    ]
}

/// A level or a point count: the page prints these as whole numbers.
fn whole(value: f64) -> String {
    format!("{value:.0}")
}

/// A plain table cell.
fn cell(text: String) -> AnyElement {
    div().child(SharedString::from(text)).into_any_element()
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
    div()
        .id("cooking-mastery-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new(
                "cooking-mastery-empty",
                IconName::Map,
                "No Cooking Mastery data",
            )
            .body(
                "Cooking Mastery needs the Rift's mastery ranks: import a save that has opened \
                 them to read the rank, the upgrades and the EXP rate.",
            )
            .action(
                Button::new("cooking-mastery-import", "Import save")
                    .icon(IconName::Upload)
                    .on_click(move |_event, window, cx| {
                        import.update(cx, |state, cx| state.open_import(window, cx));
                    }),
            ),
        )
        .into_any_element()
}
