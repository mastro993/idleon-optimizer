//! A small line chart for the depth estimates.
//!
//! The upstream page draws two SVG charts with per-depth hover tips; here the
//! same series are painted on a canvas with the axis labels laid out beside it,
//! and the depth inspector below owns the exact per-depth values and tooltips.
//! Lines can start and stop at a rock's first and last depth, which the shared
//! category chart cannot express, so this stays a canvas.

use gpui::prelude::*;
use gpui::{
    AnyElement, Hsla, IntoElement, ParentElement, PathBuilder, Role, Styled, canvas, div, point, px,
};

/// One line, one value per depth offset; `None` leaves a gap.
pub(crate) struct Series {
    pub(crate) name: String,
    pub(crate) color: Hsla,
    pub(crate) values: Vec<Option<f64>>,
    pub(crate) dashed: bool,
}

impl Series {
    pub(crate) fn new(name: impl Into<String>, color: Hsla, values: Vec<Option<f64>>) -> Self {
        Self {
            name: name.into(),
            color,
            values,
            dashed: false,
        }
    }

    /// Dashed, as the upstream page draws its secondary risk series.
    pub(crate) fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }
}

/// A line chart of `series` across `depths`, with an optional dashed reference.
#[allow(clippy::too_many_arguments)]
pub(crate) fn chart(
    id: &'static str,
    aria: String,
    series: Vec<Series>,
    depths: (usize, usize),
    rule: Option<(f64, String)>,
    log: bool,
    format: fn(f64) -> String,
    height: f32,
    grid: Hsla,
) -> AnyElement {
    let maximum = series
        .iter()
        .flat_map(|series| series.values.iter().flatten())
        .chain(rule.iter().map(|(value, _)| value))
        .copied()
        .filter(|value| value.is_finite() && *value > 0.0)
        .fold(0.0f64, f64::max)
        .max(1.0);
    let ticks: Vec<f64> = (0..=4).map(|index| maximum * index as f64 / 4.0).collect();
    let labels = x_labels(depths);
    let legend = series
        .iter()
        .map(|series| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().w(px(10.0)).h(px(3.0)).bg(series.color))
                .child(div().text_size(px(11.0)).child(series.name.clone()))
        })
        .collect::<Vec<_>>();
    let rule_text = rule
        .as_ref()
        .map(|(value, label)| format!("{} {}", format(*value), label,));
    let data = ChartData {
        series,
        maximum,
        log,
        rule: rule.map(|(value, _)| value),
    };
    let label_format = format;
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
        .child(
            div()
                .flex()
                .gap_2()
                .w_full()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .items_end()
                        .w(px(72.0))
                        .h(px(height))
                        .text_size(px(10.0))
                        .text_color(grid)
                        .children(
                            ticks
                                .iter()
                                .rev()
                                .map(|tick| div().child(label_format(*tick)))
                                .collect::<Vec<_>>(),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(height))
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(grid.opacity(0.4))
                        .child(
                            canvas(
                                move |_bounds, _window, _cx| data,
                                move |bounds, data, window, _cx| {
                                    let width = f32::from(bounds.size.width);
                                    let height = f32::from(bounds.size.height);
                                    let x_at = |index: usize, len: usize| {
                                        let fraction = if len <= 1 {
                                            0.5
                                        } else {
                                            index as f32 / (len - 1) as f32
                                        };
                                        bounds.origin.x + px(width * fraction)
                                    };
                                    let y_at = |value: f64| {
                                        let fraction = if data.log {
                                            (1.0 + value.max(0.0)).log10()
                                                / (1.0 + data.maximum).log10()
                                        } else {
                                            value / data.maximum
                                        };
                                        bounds.origin.y
                                            + px(height * (1.0 - fraction.clamp(0.0, 1.0) as f32))
                                    };
                                    for series in &data.series {
                                        let len = series.values.len();
                                        let mut run: Vec<(usize, f64)> = Vec::new();
                                        let mut paint = |run: &mut Vec<(usize, f64)>| {
                                            if run.is_empty() {
                                                return;
                                            }
                                            if run.len() == 1 {
                                                let (index, value) = run[0];
                                                let mut dot = PathBuilder::fill();
                                                let center = point(x_at(index, len), y_at(value));
                                                dot.add_polygon(
                                                    &[
                                                        point(center.x - px(2.5), center.y),
                                                        point(center.x, center.y - px(2.5)),
                                                        point(center.x + px(2.5), center.y),
                                                        point(center.x, center.y + px(2.5)),
                                                    ],
                                                    true,
                                                );
                                                if let Ok(path) = dot.build() {
                                                    window.paint_path(path, series.color);
                                                }
                                                run.clear();
                                                return;
                                            }
                                            let mut path = PathBuilder::stroke(px(2.0));
                                            if series.dashed {
                                                path = path.dash_array(&[px(6.0), px(4.0)]);
                                            }
                                            let mut started = false;
                                            for (index, value) in run.iter() {
                                                let point = point(x_at(*index, len), y_at(*value));
                                                if started {
                                                    path.line_to(point);
                                                } else {
                                                    path.move_to(point);
                                                    started = true;
                                                }
                                            }
                                            if let Ok(path) = path.build() {
                                                window.paint_path(path, series.color);
                                            }
                                            run.clear();
                                        };
                                        for (index, value) in series.values.iter().enumerate() {
                                            match value {
                                                Some(value) => run.push((index, *value)),
                                                None => paint(&mut run),
                                            }
                                        }
                                        paint(&mut run);
                                    }
                                    if let Some(rule) = data.rule {
                                        let mut path = PathBuilder::stroke(px(1.5))
                                            .dash_array(&[px(6.0), px(4.0)]);
                                        path.move_to(point(bounds.origin.x, y_at(rule)));
                                        path.line_to(point(
                                            bounds.origin.x + px(width),
                                            y_at(rule),
                                        ));
                                        if let Ok(path) = path.build() {
                                            window.paint_path(path, grid);
                                        }
                                    }
                                },
                            )
                            .size_full(),
                        )
                        .id(format!("{id}-plot"))
                        .role(Role::Image)
                        .aria_label(aria),
                ),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .pl(px(80.0))
                .pr(px(2.0))
                .text_size(px(10.0))
                .text_color(grid)
                .children(
                    labels
                        .into_iter()
                        .map(|label| div().child(label))
                        .collect::<Vec<_>>(),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .pt(px(2.0))
                .text_color(grid)
                .children(legend),
        )
        .when_some(rule_text, |chart, text| {
            chart.child(div().text_size(px(11.0)).text_color(grid).child(text))
        })
        .into_any_element()
}

struct ChartData {
    series: Vec<Series>,
    maximum: f64,
    log: bool,
    rule: Option<f64>,
}

/// The first, middle and last depth under the plot.
fn x_labels((from, to): (usize, usize)) -> Vec<String> {
    if from >= to {
        return vec![from.to_string()];
    }
    let middle = from + (to - from) / 2;
    vec![from.to_string(), middle.to_string(), to.to_string()]
}
