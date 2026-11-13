//! The Overview tab: the priority signals, the account multipliers, the GMO
//! thresholds, the Crop Scientist bonuses and the growth breakpoints.
//!
//! Ported from `farming-calc.html`'s `renderOverview`, which leads with six
//! cards, four callouts and three tables. The bean trade card and its callout
//! belong to the markets tab and are not built here; every plot's overgrowth
//! curve belongs to the Evolution & OG tab.

use crate::sections::farming::field::family_name;
use crate::state::AppState;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use idleon_farming::{CropScientistUnit, Diagnostics, Farm, GrowthBreakpoint, Snapshot, data};
use idleon_ui::format;

/// The overview tab's scrolling body.
pub(super) fn overview(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let snapshot = state.farm.snapshot();
    let diagnostics = state.farm.diagnostics(&snapshot);
    let muted = cx.theme().colors.fg_muted;
    let body = div()
        .id("farming-overview")
        .role(gpui::Role::Group)
        .aria_label("Farming overview")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(cards(&state.farm, &snapshot, &diagnostics, cx))
        .child(signals(&diagnostics, cx))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(account_multipliers(&snapshot, cx))
                .child(gmo_thresholds(&diagnostics, cx)),
        )
        .child(crop_scientist(&state.farm, cx))
        .child(growth_breakpoints(&state.farm, &snapshot))
        .child(
            Caption::new(
                "Chances and values use the multipliers this save carries; account systems outside it are not applied.",
            )
            .text_color(muted),
        );
    ScrollArea::new("farming-overview-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// A card whose headline is a formatted figure: the upstream cards show scaled
/// numbers, which the numeric statistic cannot.
pub(super) fn stat_card(
    id: &'static str,
    title: &'static str,
    value: String,
    caption: String,
    cx: &Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    div()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(format!("{title}: {value} · {caption}"))
        .flex_1()
        .min_w(px(150.0))
        .child(
            Card::new().header(CardHeader::new(title)).child(
                div()
                    .px_4()
                    .pb_3()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(value),
                    )
                    .child(Caption::new(SharedString::from(caption)).text_color(muted)),
            ),
        )
        .into_any_element()
}

/// The five figures the upstream overview leads with.
fn cards(
    farm: &Farm,
    snapshot: &Snapshot,
    diagnostics: &Diagnostics,
    cx: &Context<AppState>,
) -> AnyElement {
    let highest = farm
        .crop_quantities
        .keys()
        .next_back()
        .map_or(0, |crop| crop + 1);
    let max = data().seeds.last().map_or(0, |seed| seed.end as u32 + 1);
    div()
        .id("farming-cards")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(stat_card(
            "farming-card-crops",
            "Crops discovered",
            format!(
                "{} / {}",
                format::number(f64::from(diagnostics.crop_count)),
                format::number(f64::from(max))
            ),
            format!("Highest crop #{highest}"),
            cx,
        ))
        .child(stat_card(
            "farming-card-growth",
            "Growth rate",
            format!("{}x", format::number(snapshot.growth_rate)),
            if diagnostics.ordinary_growth_at_active_cap {
                "All ordinary seeds at the active floor".into()
            } else {
                "Some ordinary seeds still benefit".into()
            },
            cx,
        ))
        .child(stat_card(
            "farming-card-cap",
            "Crop value cap",
            format::number(diagnostics.crop_value_cap),
            format!(
                "{} / {} plots capped",
                diagnostics.capped_plots,
                idleon_farming::PLOTS
            ),
            cx,
        ))
        .child(stat_card(
            "farming-card-evolution",
            "Crop evolution",
            format!("{}x", format::number(snapshot.next_crop_common)),
            "Global multiplier before plot rank".into(),
            cx,
        ))
        .child(stat_card(
            "farming-card-points",
            "Unspent rank points",
            format::number(f64::from(diagnostics.unspent_rank_points)),
            format!("5th-column max level {}", diagnostics.fifth_column_max),
            cx,
        ))
        .into_any_element()
}

/// The four upstream callouts, each shown only while it fires.
fn signals(diagnostics: &Diagnostics, cx: &mut Context<AppState>) -> AnyElement {
    let mut list = div().id("farming-signals").flex().flex_col().gap_2();
    let mut fired = 0;
    if diagnostics.all_plots_capped {
        fired += 1;
        list = list.child(
            div().id("farming-signal-capped").child(
                Callout::new(Severity::Warning).title("Every plot is at the crop-value cap").child(
                    Caption::new(
                        "Production Rank points and value upgrades have no current output gain until the cap rises.",
                    ),
                ),
            ),
        );
    }
    if diagnostics.ordinary_growth_at_active_cap {
        fired += 1;
        list = list.child(
            div().id("farming-signal-floor").child(
                Callout::new(Severity::Info)
                    .title("Ordinary seeds are at the one-second active floor")
                    .child(Caption::new(
                        "More speed adds offline overflow, but not more active evolution checks.",
                    )),
            ),
        );
    }
    if diagnostics.medal_plot_count > 0 {
        fired += 1;
        list = list.child(
            div().id("farming-signal-medal").child(
                Callout::new(Severity::Info)
                    .title(format!(
                        "{} Medal plot(s) use a fixed seven-hour cycle",
                        diagnostics.medal_plot_count
                    ))
                    .child(Caption::new(
                        "Growth Rate scales the Medal requirement, so it cancels out of Medal growth time.",
                    )),
            ),
        );
    }
    if diagnostics.unspent_rank_points > 0 {
        fired += 1;
        list = list.child(
            div().id("farming-signal-points").child(
                Callout::new(Severity::Success)
                    .title(format!(
                        "{} Land Rank point(s) are available",
                        diagnostics.unspent_rank_points
                    ))
                    .child(Caption::new(
                        "The Land Rank upgrades spend them; the 5th column is capped by the table below.",
                    )),
            ),
        );
    }
    if fired == 0 {
        list = list.child(
            Caption::new("No priority signal fires with this save.")
                .text_color(cx.theme().colors.fg_muted),
        );
    }
    Card::new()
        .header(
            CardHeader::new("Priority signals")
                .description("What the loaded field is telling the player right now"),
        )
        .child(div().px_4().pb_3().child(list))
        .into_any_element()
}

/// `farmingExp`, `beanMultiplier`, `plotRankDisplay`, `overgrowthCommon` and
/// `expectedVines`, as the upstream stat lines.
fn account_multipliers(snapshot: &Snapshot, cx: &Context<AppState>) -> AnyElement {
    let rows = [
        (
            "farming-account-farming-exp",
            "Farming EXP multiplier",
            format!("{}x", format::number(snapshot.farming_exp)),
        ),
        (
            "farming-account-beans",
            "Magic Bean multiplier",
            format!("{}x", format::number(snapshot.bean_multiplier)),
        ),
        (
            "farming-account-rank-exp",
            "Land Rank EXP multiplier",
            format!("{}x", format::number(snapshot.plot_rank_display)),
        ),
        (
            "farming-account-overgrowth",
            "Overgrowth common multiplier",
            format!("{}x", format::number(snapshot.overgrowth_common)),
        ),
        (
            "farming-account-vines",
            "Expected crops on vine",
            format::number(snapshot.expected_vines),
        ),
    ];
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let mut list = div().flex().flex_col();
    for (id, label, value) in rows {
        list = list.child(
            div()
                .id(id)
                .role(gpui::Role::Label)
                .aria_label(format!("{label}: {value}"))
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .py_1p5()
                .border_b_1()
                .border_color(muted.opacity(0.15))
                .child(Caption::new(label).text_color(muted))
                .child(div().text_size(px(13.0)).text_color(accent).child(value)),
        );
    }
    div()
        .flex_1()
        .min_w(px(280.0))
        .child(
            Card::new()
                .header(CardHeader::new("Account multipliers"))
                .child(div().id("farming-account").px_4().pb_3().child(list)),
        )
        .into_any_element()
}

/// The five GMO thresholds and how many crops sit at each one.
fn gmo_thresholds(diagnostics: &Diagnostics, cx: &Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let accent = cx.theme().colors.accent;
    let mut list = div().flex().flex_col();
    for (index, threshold) in data().gmo_thresholds.iter().enumerate() {
        let count = diagnostics.gmo_counts[index];
        let label = format!("{} crops", format::number(*threshold));
        list = list.child(
            div()
                .id(("farming-gmo", index))
                .role(gpui::Role::Label)
                .aria_label(format!("GMO {label}: {count}"))
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .py_1p5()
                .border_b_1()
                .border_color(muted.opacity(0.15))
                .child(Caption::new(label).text_color(muted))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(accent)
                        .child(format::number(count)),
                ),
        );
    }
    div()
        .flex_1()
        .min_w(px(220.0))
        .child(
            Card::new()
                .header(
                    CardHeader::new("GMO thresholds")
                        .description("Crop ids at or above each threshold"),
                )
                .child(div().id("farming-gmo").px_4().pb_3().child(list)),
        )
        .into_any_element()
}

/// The ten permanent Crop Scientist bonuses.
fn crop_scientist(farm: &Farm, cx: &Context<AppState>) -> AnyElement {
    let table = farm.crop_scientist();
    let muted = cx.theme().colors.fg_muted;
    let mut rows = Table::new(["Permanent bonus", "Value", "Applies to"]);
    for bonus in &table.rows {
        let value = match bonus.unit {
            CropScientistUnit::Multiplier => format!("{}x", format::number(bonus.display_value())),
            CropScientistUnit::Percent => format!("+{}%", format::number(bonus.display_value())),
            CropScientistUnit::Flat => format!("+{}", format::number(bonus.display_value())),
        };
        rows = rows.row([
            div()
                .id(("farming-crop-scientist", bonus.id))
                .role(gpui::Role::Label)
                .aria_label(format!("{}: {value}", bonus.name))
                .child(SharedString::from(bonus.name))
                .into_any_element(),
            div().child(value).into_any_element(),
            div()
                .child(SharedString::from(bonus.applies_to))
                .into_any_element(),
        ]);
    }
    let description = if table.unlocked() == table.rows.len() {
        format!(
            "All {} emporium items owned · account multiplier {}x",
            table.rows.len(),
            format::number(table.multiplier)
        )
    } else {
        format!(
            "{} of {} emporium items owned · account multiplier {}x",
            table.unlocked(),
            table.rows.len(),
            format::number(table.multiplier)
        )
    };
    Card::new()
        .header(CardHeader::new("Crop Scientist bonuses").description(description))
        .child(div().id("farming-crop-scientist-table").p_3().child(rows))
        .child(
            div()
                .px_4()
                .pb_3()
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "Each row is gated behind one emporium item in Ninja[102][9]. The multiplier adds Grimoire 22, exotic 40 and Vault 79; the lab mainframe term is not modelled.",
                ),
        )
        .into_any_element()
}

/// The seven seed families' raw and active growth cycles.
fn growth_breakpoints(farm: &Farm, snapshot: &Snapshot) -> AnyElement {
    let mut table = Table::new([
        "Seed",
        "Raw cycle",
        "Active cycle",
        "One-second threshold",
        "State",
    ]);
    for row in farm.growth_breakpoints(snapshot) {
        table = table.row(growth_row(&row));
    }
    Card::new()
        .header(
            CardHeader::new("Growth breakpoints")
                .description("The one-second active scheduler floor, per seed family"),
        )
        .child(div().id("farming-growth-table").p_3().child(table))
        .into_any_element()
}

/// One breakpoint row.
fn growth_row(row: &GrowthBreakpoint) -> [AnyElement; 5] {
    let state = if row.medal_fixed {
        Badge::new("Fixed 7h")
            .tone(ely_gpui_component::data_display::Tone::Warning)
            .into_any_element()
    } else if row.at_active_cap {
        Badge::new("Active cap")
            .tone(ely_gpui_component::data_display::Tone::Success)
            .into_any_element()
    } else {
        Badge::new("Benefits")
            .tone(ely_gpui_component::data_display::Tone::Info)
            .into_any_element()
    };
    [
        div()
            .id(("farming-growth", row.family))
            .role(gpui::Role::Label)
            .aria_label(format!(
                "{}: raw {} · active {}",
                family_name(row.family as i64),
                format::duration(row.raw_seconds),
                format::duration(row.active_seconds)
            ))
            .child(SharedString::from(family_name(row.family as i64)))
            .into_any_element(),
        div()
            .child(format::duration(row.raw_seconds))
            .into_any_element(),
        div()
            .child(format::duration(row.active_seconds))
            .into_any_element(),
        div()
            .child(if row.medal_fixed {
                "Not applicable".to_string()
            } else {
                format!("{}x", format::number(row.one_second_rate))
            })
            .into_any_element(),
        state,
    ]
}
