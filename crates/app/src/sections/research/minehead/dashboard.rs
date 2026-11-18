//! The Minehead dashboard: the floor's stat cards, the upgrade table and the
//! floor rewards.

use super::super::controls::stepper;
use super::boss_name;
use crate::sections::research::sushi::columns::{self, Column};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableRow};
use idleon_minehead::{Inputs, game, upgrade_desc};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, show_number};

/// How many figures one strip holds before the next strip starts.
const FIGURES_PER_STRIP: usize = 5;

/// One headline figure, formatted.
struct Figure {
    id: &'static str,
    label: &'static str,
    value: SharedString,
    caption: SharedString,
}

/// A whole-number figure, grouped by thousands.
fn figure(
    id: &'static str,
    label: &'static str,
    value: f64,
    caption: impl Into<SharedString>,
) -> Figure {
    Figure {
        id,
        label,
        value: format::exact(value).into(),
        caption: caption.into(),
    }
}

/// One upgrade's row, formatted.
struct UpgradeRow {
    /// Past the next unlock: drawn as `???` unless the spoilers show.
    hidden: bool,
    name: SharedString,
    level: SharedString,
    desc: SharedString,
    cost: SharedString,
    req: SharedString,
    locked: bool,
    maxed: bool,
}

/// One floor's reward card.
struct Reward {
    cleared: bool,
    /// Past the current floor: drawn as `???` unless the spoilers show.
    hidden: bool,
    boss: SharedString,
    desc: SharedString,
}

/// The dashboard's figures, upgrade rows and rewards, derived from `inputs`.
pub(super) struct Dashboard {
    figures: Vec<Figure>,
    rows: Vec<UpgradeRow>,
    rewards: Vec<Reward>,
    research_level: u32,
}

impl Dashboard {
    pub(super) fn new(inputs: &Inputs) -> Self {
        let data = game();
        let floor = inputs.floor;
        let (cols, rows) = inputs.grid_dims();
        let figures = vec![
            figure("mh-boss", "Floor", floor as f64, boss_name(floor)),
            figure(
                "mh-currency",
                "Mine Currency",
                inputs.currency,
                "saved balance",
            ),
            figure(
                "mh-grid",
                "Grid",
                (cols * rows) as f64,
                format!("{cols}×{rows} tiles"),
            ),
            figure("mh-lives", "Lives", inputs.lives().round(), "MaxHP_You"),
            figure(
                "mh-damage",
                "Base Damage",
                inputs.base_dmg(),
                "per tile number",
            ),
            figure("mh-hp", "Boss HP", inputs.boss_hp().round(), "floor health"),
            figure("mh-mines", "Depth Charges", inputs.mines(), "per grid"),
            figure(
                "mh-tries",
                "Daily Tries",
                inputs.daily_tries(),
                "from grid 147",
            ),
            figure(
                "mh-cph",
                "Currency/hr",
                inputs.currency_per_hour(),
                "every account source",
            ),
        ];
        let next_upgrade_req = (0..data.upgrades.len())
            .map(upg_req)
            .filter(|req| *req > inputs.research_level)
            .min()
            .unwrap_or(u32::MAX);
        let rows = data
            .upgrades
            .iter()
            .enumerate()
            .map(|(index, upgrade)| {
                let level = inputs.levels.get(index).copied().unwrap_or(0.0);
                let req = upg_req(index);
                let locked = inputs.research_level < req;
                let maxed = upgrade.max_lv <= 998.0 && level >= upgrade.max_lv;
                let level_text = if maxed {
                    format!("{level:.0}/{:.0} MAX", upgrade.max_lv)
                } else if upgrade.max_lv > 998.0 {
                    format!("{level:.0}")
                } else {
                    format!("{level:.0}/{:.0}", upgrade.max_lv)
                };
                let cost = if level < upgrade.max_lv || upgrade.max_lv > 998.0 {
                    format::value(inputs.upgrade_cost(index))
                } else {
                    "--".into()
                };
                UpgradeRow {
                    hidden: locked && req > next_upgrade_req,
                    name: upgrade.name.replace('_', " ").into(),
                    level: level_text.into(),
                    desc: upgrade_desc(inputs, index).into(),
                    cost: cost.into(),
                    req: req.to_string().into(),
                    locked,
                    maxed,
                }
            })
            .collect();
        let rewards = (0..data.floor_rewards.qty.len().min(23))
            .map(|floor_index| Reward {
                cleared: floor > floor_index,
                hidden: floor_index > floor,
                boss: boss_name(floor_index).into(),
                desc: data.floor_reward_desc(floor_index).into(),
            })
            .collect();
        Self {
            figures,
            rows,
            rewards,
            research_level: inputs.research_level,
        }
    }
}

/// The upgrade table's columns.
const COLUMNS: [Column; 5] = [
    ("Name", Some(150.0), false),
    ("Lv", Some(96.0), true),
    ("Description", None, false),
    ("Cost", Some(80.0), true),
    ("Req", Some(48.0), true),
];

/// A cell of the upgrade table's column `index`.
fn cell(index: usize) -> TableCell {
    columns::cell(&COLUMNS, index)
}

pub(super) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let minehead = &mut state.research.minehead;
    let Some(inputs) = minehead.inputs.as_ref() else {
        return crate::ui::section::no_save(cx);
    };
    let server_var_cost = inputs.server_var_cost;
    let dashboard = minehead
        .dashboard
        .get_or_insert_with(|| Dashboard::new(inputs));
    let show_spoiler = minehead.show_spoiler;
    show_number(&minehead.server_cost, server_var_cost, 2, window, cx);
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let entity = cx.entity();
    let cost_entity = entity.clone();

    let cards =
        div()
            .flex()
            .flex_col()
            .gap_3()
            .children(dashboard.figures.chunks(FIGURES_PER_STRIP).map(|figures| {
                KpiStrip::new().children(figures.iter().map(|figure| {
                    Statistic::new(figure.id, figure.label, figure.value.clone())
                        .caption(figure.caption.clone())
                }))
            }));

    let server_var = Card::new()
        .header(CardHeader::new("Server var").description(
            "A_MineCost scales every upgrade's price; the save's own value loads first.",
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new("A_MineCost").text_color(muted))
                .child(
                    div().id("mh-svar-cost").w(px(120.0)).child(
                        stepper(
                            "mh-svar-cost",
                            &minehead.server_cost,
                            (server_var_cost, 0.01, 2),
                            move |cost, _window, cx| {
                                cost_entity.update(cx, |state, cx| {
                                    state.research.minehead.set_server_var_cost(cost, cx);
                                });
                            },
                        )
                        .accessibility_label("A_MineCost")
                        .small(),
                    ),
                ),
        );

    let spoiler = entity.clone();
    let spoiler_toggle = Checkbox::new("mh-spoiler")
        .label("Show all floors (spoilers)")
        .checked(show_spoiler)
        .on_click(move |on, _window, cx| {
            let on = *on;
            spoiler.update(cx, |state, cx| {
                state.research.minehead.show_spoiler = on;
                cx.notify();
            });
        });

    // The upgrade table.
    let body = TableBody::new().children(dashboard.rows.iter().enumerate().map(|(index, row)| {
        if row.hidden && !show_spoiler {
            return TableRow::new()
                .child(cell(0).child(div().id(("mh-upg-name", index)).child("???")))
                .child(cell(1).child("--"))
                .child(cell(2).child("???"))
                .child(cell(3).child("--"))
                .child(cell(4).child(row.req.clone()));
        }
        let tone = if row.maxed {
            theme.success
        } else if row.locked {
            muted
        } else {
            theme.foreground
        };
        TableRow::new()
            .child(cell(0).child(div().id(("mh-upg-name", index)).child(row.name.clone())))
            .child(cell(1).text_color(tone).child(row.level.clone()))
            .child(
                cell(2).child(
                    div()
                        .id(("mh-upg-desc", index))
                        .text_color(muted)
                        .child(row.desc.clone()),
                ),
            )
            .child(cell(3).text_color(muted).child(row.cost.clone()))
            .child(
                cell(4)
                    .text_color(if row.locked { theme.danger } else { muted })
                    .child(row.req.clone()),
            )
    }));
    let table = div().h(px(430.0)).child(
        div()
            .overflow_y_scrollbar()
            .id("mh-upgrades-scroll")
            .size_full()
            .child(
                Table::new()
                    .small()
                    .child(columns::header(&COLUMNS))
                    .child(body),
            ),
    );
    let upgrades = Card::new()
        .header(
            CardHeader::new("Upgrades")
                .description(format!("Research level {}", dashboard.research_level)),
        )
        .child(table);

    // The floor rewards.
    let rewards =
        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(
                dashboard
                    .rewards
                    .iter()
                    .enumerate()
                    .map(|(floor_index, reward)| {
                        let card = div()
                            .id(("mh-reward", floor_index))
                            .flex_none()
                            .w(px(170.0))
                            .p_2()
                            .rounded(theme.radius)
                            .bg(theme.muted);
                        if reward.hidden && !show_spoiler {
                            return card
                                .opacity(0.35)
                                .child(Caption::new("???").text_color(muted));
                        }
                        card.flex()
                            .flex_col()
                            .gap_1()
                            .when(!reward.cleared, |card| card.opacity(0.5))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(Caption::new(reward.boss.clone()).text_color(
                                        if reward.cleared { theme.success } else { muted },
                                    ))
                                    .child(
                                        Caption::new(if reward.cleared { "Yes" } else { "--" })
                                            .text_color(muted),
                                    ),
                            )
                            .child(
                                Caption::new(reward.desc.clone())
                                    .text_color(muted)
                                    .text_size(px(10.0)),
                            )
                    }),
            );
    let rewards = Card::new()
        .header(CardHeader::new("Boss rewards").description("Each cleared floor's bonus"))
        .child(rewards);

    div()
        .id("minehead-dashboard")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(cards)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(server_var)
                .child(div().flex_1())
                .child(spoiler_toggle),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .min_h_0()
                .child(div().flex_1().min_w_0().child(upgrades))
                .child(div().w(px(380.0)).flex_none().child(rewards)),
        )
        .into_any_element()
}

/// The research level an upgrade unlocks at.
fn upg_req(index: usize) -> u32 {
    idleon_minehead::math::upg_lv_req(index)
}
