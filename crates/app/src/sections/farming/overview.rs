//! The Overview tab: the priority signals, the account multipliers, the GMO
//! thresholds, the Crop Scientist bonuses and the growth breakpoints.
//!
//! The bean trade card and its callout belong to the markets tab and are not
//! built here; every plot's overgrowth curve belongs to the Evolution & OG tab.

use super::controls::Field;
use super::field::family_name;
use super::table::{Col, figure, note, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use idleon_farming::{CropScientistUnit, Diagnostics, Farm, GrowthBreakpoint, data};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, KpiStrip, PANEL_GAP, TEXT_BODY, Tone,
};

/// The overview tab's scrolling body.
pub(super) fn overview(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let field = state.farming_field();
    let body = v_flex()
        .id("farming-overview")
        .role(gpui::Role::Group)
        .aria_label("Farming overview")
        .gap(PANEL_GAP)
        .child(cards(&state.farm, &field))
        .child(signals(&field.diagnostics, cx))
        .child(
            h_flex()
                .items_start()
                .gap(PANEL_GAP)
                .child(account_multipliers(&field, cx))
                .child(gmo_thresholds(&field.diagnostics, cx)),
        )
        .child(crop_scientist(&field, cx))
        .child(growth_breakpoints(&field.breakpoints, cx))
        .child(note(
            "Chances and values use the multipliers this save carries; account systems outside it are not applied.",
            cx,
        ));
    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("farming-overview-scroll")
        .into_any_element()
}

/// The five figures the overview leads with.
fn cards(farm: &Farm, field: &Field) -> AnyElement {
    let (snapshot, diagnostics) = (&field.snapshot, &field.diagnostics);
    let highest = farm
        .crop_quantities
        .keys()
        .next_back()
        .map_or(0, |crop| crop + 1);
    let max = data().seeds.last().map_or(0, |seed| seed.end as u32 + 1);
    div()
        .id("farming-cards")
        .child(
            KpiStrip::new()
                .child(figure(
                    "farming-card-crops",
                    "Crops discovered",
                    format!(
                        "{} / {}",
                        format::number(f64::from(diagnostics.crop_count)),
                        format::number(f64::from(max))
                    ),
                    format!("Highest crop #{highest}"),
                ))
                .child(figure(
                    "farming-card-growth",
                    "Growth rate",
                    format!("{}x", format::number(snapshot.growth_rate)),
                    if diagnostics.ordinary_growth_at_active_cap {
                        "All ordinary seeds at the active floor"
                    } else {
                        "Some ordinary seeds still benefit"
                    },
                ))
                .child(figure(
                    "farming-card-cap",
                    "Crop value cap",
                    format::number(diagnostics.crop_value_cap),
                    format!(
                        "{} / {} plots capped",
                        diagnostics.capped_plots,
                        idleon_farming::PLOTS
                    ),
                ))
                .child(figure(
                    "farming-card-evolution",
                    "Crop evolution",
                    format!("{}x", format::number(snapshot.next_crop_common)),
                    "Global multiplier before plot rank",
                ))
                .child(figure(
                    "farming-card-points",
                    "Unspent rank points",
                    format::number(f64::from(diagnostics.unspent_rank_points)),
                    format!("5th-column max level {}", diagnostics.fifth_column_max),
                )),
        )
        .into_any_element()
}

/// The four callouts, each shown only while it fires.
fn signals(diagnostics: &Diagnostics, cx: &App) -> AnyElement {
    let signal = |id: &'static str, tone: Tone, title: String, body: &'static str| {
        div()
            .id(id)
            .child(Callout::new(tone, title).child(Caption::new(body)))
    };
    let mut list = v_flex().id("farming-signals").gap_2();
    let mut fired = 0;
    if diagnostics.all_plots_capped {
        fired += 1;
        list = list.child(signal(
            "farming-signal-capped",
            Tone::Warning,
            "Every plot is at the crop-value cap".into(),
            "Production Rank points and value upgrades have no current output gain until the cap rises.",
        ));
    }
    if diagnostics.ordinary_growth_at_active_cap {
        fired += 1;
        list = list.child(signal(
            "farming-signal-floor",
            Tone::Info,
            "Ordinary seeds are at the one-second active floor".into(),
            "More speed adds offline overflow, but not more active evolution checks.",
        ));
    }
    if diagnostics.medal_plot_count > 0 {
        fired += 1;
        list = list.child(signal(
            "farming-signal-medal",
            Tone::Info,
            format!(
                "{} Medal plot(s) use a fixed seven-hour cycle",
                diagnostics.medal_plot_count
            ),
            "Growth Rate scales the Medal requirement, so it cancels out of Medal growth time.",
        ));
    }
    if diagnostics.unspent_rank_points > 0 {
        fired += 1;
        list = list.child(signal(
            "farming-signal-points",
            Tone::Success,
            format!(
                "{} Land Rank point(s) are available",
                diagnostics.unspent_rank_points
            ),
            "The Land Rank upgrades spend them; the 5th column is capped by the table below.",
        ));
    }
    if fired == 0 {
        list = list.child(note("No priority signal fires with this save.", cx));
    }
    Card::new()
        .header(
            CardHeader::new("Priority signals")
                .description("What the loaded field is telling the player right now"),
        )
        .child(list)
        .into_any_element()
}

/// One labelled figure in a stat list, split from the next by a hairline.
fn stat_line(id: impl Into<gpui::ElementId>, label: String, value: String, cx: &App) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(format!("{label}: {value}"))
        .justify_between()
        .gap_3()
        .py_1p5()
        .border_b_1()
        .border_color(theme.border)
        .child(Caption::new(label).text_color(theme.muted_foreground))
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_size(TEXT_BODY)
                .text_color(theme.primary)
                .child(value),
        )
        .into_any_element()
}

/// Farming EXP, the bean multiplier, the plot rank, the common overgrowth and
/// the expected vines, as stat lines.
fn account_multipliers(field: &Field, cx: &App) -> AnyElement {
    let snapshot = &field.snapshot;
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
    div()
        .flex_1()
        .min_w(px(280.0))
        .child(
            Card::new()
                .header(CardHeader::new("Account multipliers"))
                .child(v_flex().id("farming-account").children(
                    rows.map(|(id, label, value)| stat_line(id, label.into(), value, cx)),
                )),
        )
        .into_any_element()
}

/// The five GMO thresholds and how many crops sit at each one.
fn gmo_thresholds(diagnostics: &Diagnostics, cx: &App) -> AnyElement {
    let lines = data()
        .gmo_thresholds
        .iter()
        .enumerate()
        .map(|(index, threshold)| {
            let label = format!("{} crops", format::number(*threshold));
            stat_line(
                ("farming-gmo", index),
                label,
                format::number(diagnostics.gmo_counts[index]),
                cx,
            )
        });
    div()
        .flex_1()
        .min_w(px(220.0))
        .child(
            Card::new()
                .header(
                    CardHeader::new("GMO thresholds")
                        .description("Crop ids at or above each threshold"),
                )
                .child(v_flex().id("farming-gmo").children(lines)),
        )
        .into_any_element()
}

/// The ten permanent Crop Scientist bonuses.
fn crop_scientist(field: &Field, cx: &App) -> AnyElement {
    let bonuses = &field.crop_scientist;
    let multiplier = field.crop_scientist_multiplier;
    let unlocked = bonuses.iter().filter(|bonus| bonus.unlocked).count();
    let rows = bonuses.iter().map(|bonus| {
        let value = match bonus.unit {
            CropScientistUnit::Multiplier => format!("{}x", format::number(bonus.display_value())),
            CropScientistUnit::Percent => format!("+{}%", format::number(bonus.display_value())),
            CropScientistUnit::Flat => format!("+{}", format::number(bonus.display_value())),
        };
        [
            div()
                .id(("farming-crop-scientist", bonus.id))
                .role(gpui::Role::Label)
                .aria_label(format!("{}: {value}", bonus.name))
                .child(SharedString::from(bonus.name))
                .into_any_element(),
            text(value),
            text(bonus.applies_to),
        ]
    });
    let description = if unlocked == bonuses.len() {
        format!(
            "All {} emporium items owned · account multiplier {}x",
            bonuses.len(),
            format::number(multiplier)
        )
    } else {
        format!(
            "{unlocked} of {} emporium items owned · account multiplier {}x",
            bonuses.len(),
            format::number(multiplier)
        )
    };
    Card::new()
        .header(CardHeader::new("Crop Scientist bonuses").description(description))
        .child(table(
            "farming-crop-scientist",
            &[
                Col::Text("Permanent bonus"),
                Col::Number("Value"),
                Col::Text("Applies to"),
            ],
            rows,
            cx,
        ))
        .child(note(
            "Each row is gated behind one emporium item in Ninja[102][9]. The multiplier adds Grimoire 22, exotic 40 and Vault 79; the lab mainframe term is not modelled.",
            cx,
        ))
        .into_any_element()
}

/// The seven seed families' raw and active growth cycles.
fn growth_breakpoints(breakpoints: &[GrowthBreakpoint], cx: &App) -> AnyElement {
    Card::new()
        .header(
            CardHeader::new("Growth breakpoints")
                .description("The one-second active scheduler floor, per seed family"),
        )
        .child(table(
            "farming-growth",
            &[
                Col::Text("Seed"),
                Col::Number("Raw cycle"),
                Col::Number("Active cycle"),
                Col::Number("One-second threshold"),
                Col::Text("State"),
            ],
            breakpoints.iter().map(growth_row),
            cx,
        ))
        .into_any_element()
}

/// One breakpoint row.
fn growth_row(row: &GrowthBreakpoint) -> [AnyElement; 5] {
    let state = if row.medal_fixed {
        tag("Fixed 7h", Tone::Warning)
    } else if row.at_active_cap {
        tag("Active cap", Tone::Success)
    } else {
        tag("Benefits", Tone::Info)
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
        text(format::duration(row.raw_seconds)),
        text(format::duration(row.active_seconds)),
        text(if row.medal_fixed {
            "Not applicable".to_string()
        } else {
            format!("{}x", format::number(row.one_second_rate))
        }),
        state.into_any_element(),
    ]
}
