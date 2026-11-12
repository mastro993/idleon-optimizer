//! The 9×4 plot field: family colours, tiles and the crop legend.

use crate::state::AppState;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Styled, div, px, rgb};
use idleon_farming::{self as farming, Assignment, CropRow, PlotMetrics};
use idleon_ui::format;

/// The 9×4 family colours from `farming-calc.html`.
pub(super) fn family_color(family: i64) -> Hsla {
    match family {
        0 => rgb(0x72_c6_5d).into(),
        1 => rgb(0xbc_8f_58).into(),
        2 => rgb(0xd7_c6_4e).into(),
        3 => rgb(0xef_6f_93).into(),
        4 => rgb(0x9b_78_cc).into(),
        5 => rgb(0x57_b6_c7).into(),
        6 => rgb(0xd7_a9_41).into(),
        _ => rgb(0x77_77_77).into(),
    }
}

/// The family's display name, or `Empty`.
pub(super) fn family_name(family: i64) -> String {
    if family < 0 {
        return "Empty".into();
    }
    farming::data()
        .seed(family as usize)
        .map(|seed| {
            let mut name = seed.name.to_lowercase();
            if let Some(first) = name.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            name
        })
        .unwrap_or_else(|| "Empty".into())
}

/// The full 9×4 field with its family legend and the assignment's targets.
pub(super) fn field_card(
    plots: &[PlotMetrics],
    selected: usize,
    has_plots: bool,
    assignment: &Assignment,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let palette = TilePalette {
        border: colors.border,
        accent: colors.accent,
        surface: colors.surface,
        sunken: colors.sunken,
        fg: colors.fg,
        muted: colors.fg_muted,
    };
    let mut rows = div().flex().flex_col().gap_2();
    for row in 0..4 {
        let mut line = div().id(("farming-row", row)).flex().gap_2();
        for column in 0..9 {
            let index = row * 9 + column;
            line = line.child(plot_tile(
                &plots[index],
                index == selected,
                assignment.rows.get(index),
                palette,
                cx,
            ));
        }
        rows = rows.child(line);
    }

    let mut legend = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .px_4()
        .pb_3();
    for seed in &farming::data().seeds {
        legend = legend.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .size(px(8.0))
                        .rounded(px(2.0))
                        .bg(family_color(seed.id as i64)),
                )
                .child(Caption::new(family_name(seed.id as i64)).text_color(colors.fg_muted)),
        );
    }

    let description = if has_plots {
        "Every plot from the loaded save; click one to inspect it."
    } else {
        "The 9 × 4 field is empty; import a save or plant a crop."
    };
    Card::new()
        .header(CardHeader::new("Full 9 × 4 field").description(description))
        .child(div().id("farming-field").px_4().pt_3().child(rows))
        .child(legend)
        .child(
            div()
                .px_4()
                .pb_3()
                .text_size(px(11.0))
                .text_color(colors.fg_muted)
                .child(
                    "Chances and values use the multipliers this save carries; account systems outside it are not applied.",
                ),
        )
        .into_any_element()
}

/// The colours one field tile draws with, copied out of the theme borrow.
#[derive(Clone, Copy)]
struct TilePalette {
    border: Hsla,
    accent: Hsla,
    surface: Hsla,
    sunken: Hsla,
    fg: Hsla,
    muted: Hsla,
}

/// One plot: family stripe, crop id, overgrowth, rank, the next roll and the
/// crop the objective wants there.
fn plot_tile(
    metric: &PlotMetrics,
    selected: bool,
    target: Option<&CropRow>,
    palette: TilePalette,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let TilePalette {
        border,
        accent,
        surface,
        sunken,
        fg,
        muted,
    } = palette;
    let index = metric.plot;
    let tint = family_color(metric.seed_family);
    let family = family_name(metric.seed_family);
    let empty = metric.seed_family < 0;
    let title = if empty {
        format!("Plot {} · empty", index + 1)
    } else {
        format!(
            "Plot {} · {} · crop #{} · overgrowth {} · rank {}",
            index + 1,
            family,
            metric.crop_id,
            metric.overgrowth,
            metric.rank
        )
    };
    let crop = if empty {
        "Empty".to_string()
    } else {
        format!("#{}", metric.crop_id)
    };
    let detail = if empty {
        "—".to_string()
    } else {
        format!(
            "OG {} · Rank {}",
            metric.overgrowth,
            format::number(f64::from(metric.rank))
        )
    };
    let roll = if empty {
        String::new()
    } else {
        format!(
            "Evo {} · {}",
            format::chance(metric.next_crop_chance),
            format::duration(metric.growth_seconds)
        )
    };
    let target = target
        .filter(|row| row.change_needed())
        .map(|row| format!("Target #{}", row.target_crop))
        .unwrap_or_default();
    let title = if target.is_empty() {
        title
    } else {
        format!("{title} · {target}")
    };
    div()
        .id(format!("plot-{index}"))
        .role(gpui::Role::Button)
        .aria_label(title.clone())
        .aria_selected(selected)
        .flex_1()
        .min_w_0()
        .h(px(96.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(6.0))
        .bg(if selected { sunken } else { surface })
        .border_1()
        .border_color(if selected { accent } else { border })
        .child(div().h(px(3.0)).w_full().bg(tint))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .p_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(muted)
                                .child(format!("Plot {}", index + 1)),
                        )
                        .child(div().text_size(px(10.0)).text_color(tint).child(family)),
                )
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(if empty { muted } else { fg })
                        .child(crop),
                )
                .child(div().text_size(px(10.0)).text_color(muted).child(detail))
                .child(div().text_size(px(10.0)).text_color(muted).child(roll))
                .child(div().text_size(px(10.0)).text_color(tint).child(target)),
        )
        .tooltip(ely_gpui_component::primitives::Tooltip::text(title))
        .on_click(cx.listener(move |state, _event, window, cx| {
            state.select_plot(index, window, cx);
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_use_the_upstream_colours_and_names() {
        assert_eq!(family_name(0), "Basic");
        assert_eq!(family_name(6), "Medal");
        assert_eq!(family_name(-1), "Empty");
        assert_eq!(family_color(3), rgb(0xef_6f_93).into());
    }
}
