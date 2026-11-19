//! The expedition run: its controls, the run in flight and the results.
//!
//! The expedition simulation runs through the shared
//! [`idleon_ui::runner::TaskRunner`], so the progress card, the cancel action
//! and the stale marker are the ones every other section uses. A run is
//! deterministic: the seed, the cave, the party, the belt and the memorized
//! elixirs fully describe it, so the result says which seed produced it.

use super::controls::{Derived, entry_choices};
use super::{boxed, field, number, select, tag};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Div, IntoElement, ParentElement, Role, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::table::{
    DataTable, Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
};
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_spelunking::{
    Account, ExitKind, Policy, Requested, RunResult, Status, ToolChoice, data,
};
use idleon_ui::format;
use idleon_ui::runner::TaskState;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, TEXT_BODY, Tone};
use std::rc::Rc;

/// What a run was produced from; a change marks the result stale.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) requested: Requested,
    /// The account values the run reads, which the request does not carry.
    pub(crate) account: Account,
    pub(crate) seed: u32,
    /// The save this run read, so a new import is stale.
    pub(crate) revision: u64,
}

/// The run card: controls, the run in flight and the stored result.
pub(crate) fn card(
    state: &AppState,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let task = state.spelunk_simulation(cx);
    let mut body = div()
        .id("spelunk-run-panel")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls(state, derived, &task, window, cx));
    if state.spelunking.policy == Policy::AmberFarm {
        let amber = state.spelunking.amber;
        let finishers = (amber.finishers as usize).min(state.spelunking.party_ids().len());
        let harvest = if amber.harvest_depths == 0 {
            "After the oranges, seek the next visible rope immediately.".to_string()
        } else {
            format!(
                "After the oranges, target +{} floors before seeking a rope.",
                amber.harvest_depths
            )
        };
        body = body.child(Caption::new(format!(
            "Planned: look for stamina burn from D{} · preserve the last {finishers} \
             character(s), first at {}% stamina. {harvest} Seek a rope sooner below {}% \
             finishing stamina; this is a target, not floors actually gained.",
            amber.dump_depth, amber.reserve_pct, amber.rope_reserve_pct
        )));
    }
    let mut body = body.child(Caption::new(
        "The run replays the game's 10 ms tick with a seeded generator: the same seed, cave, \
         party, belt and memorized elixirs reproduce it exactly. Progress and the events log are \
         the accessible record; the canvas replay is not built yet.",
    ));
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "spelunk-run",
            "Running the expedition",
            "Runs off the UI thread; cancelling keeps the previous result.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_spelunk_simulation(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "spelunk-run-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(result) = &task.result {
        body = body.child(results_card(
            state,
            result,
            task.stale,
            state.spelunking.account.regen,
        ));
    }
    body.into_any_element()
}

/// The seed, reaction time, tool and entry, and the run actions.
fn controls(
    state: &AppState,
    derived: &Derived,
    task: &TaskState<Rc<RunResult>>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let spelunking = &state.spelunking;
    let controls = &state.spelunking_controls;
    let run_entity = cx.entity();
    let cancel_entity = cx.entity();
    let seed = number(
        "spelunk-run-seed",
        &controls.seed,
        "Seed",
        f64::from(spelunking.sim_seed),
        0,
        window,
        cx,
    );
    let reaction = number(
        "spelunk-run-reaction",
        &controls.reaction,
        "Manual reaction (ms)",
        spelunking.sim_reaction_ms,
        0,
        window,
        cx,
    );
    let tool = select(
        &controls.run_tool,
        "spelunk-run-tool",
        "Manual tool",
        &derived.run_tools,
        &spelunking.sim_tool,
        window,
        cx,
    );
    let entry = select(
        &controls.run_entry,
        "spelunk-run-entry",
        "Entry",
        &entry_choices(),
        &spelunking.sim_entry,
        window,
        cx,
    );
    let run = Button::new("spelunk-run-start")
        .label(if task.running {
            "Running…"
        } else {
            "Run expedition"
        })
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(task.running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_spelunk_simulation(cx));
        });
    let cancel = Button::new("spelunk-run-stop")
        .label("Cancel")
        .secondary()
        .small()
        .icon(IconName::X)
        .disabled(!task.running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_spelunk_simulation(cx));
        });

    let header = CardHeader::new("Expedition run").description(
        "Cave, mode, policy, party and belt come from the controls above; the entry and the \
         manual tool are the run's own choices.",
    );
    Card::new()
        .header(header)
        .child(
            div()
                .id("spelunk-run-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .child(field("Seed", seed))
                .child(field("Manual reaction", reaction))
                .child(field(
                    "Manual tool",
                    boxed(&controls.run_tool, tool, window, cx),
                ))
                .child(field(
                    "Entry",
                    boxed(&controls.run_entry, entry, window, cx),
                ))
                .child(div().flex().items_center().gap_2().child(run).child(cancel))
                .child(Caption::new(format!(
                    "{} · {}",
                    spelunking.policy.label(),
                    if spelunking.manic { "Manic" } else { "Normal" }
                ))),
        )
        .into_any_element()
}

/// The stored run: its outcome, loot, stamina, members and event log.
fn results_card(state: &AppState, result: &RunResult, stale: bool, regen: f64) -> AnyElement {
    let amber = result.rewards[0];
    let recovery = result
        .stamina
        .iter()
        .map(|member| (member.start - member.end).max(0.0))
        .fold(0.0f64, f64::max);
    let recovered = recovery / regen.max(f64::MIN_POSITIVE);
    let mut header = CardHeader::new(format!("Run result · {}", result.status.label()))
        .description(format!(
            "seed {} · {} · {} · entry {} · {} manual tool · {:.1}s",
            result.seed,
            result.config.policy.label(),
            if result.config.manic {
                "Manic"
            } else {
                "Normal"
            },
            result.config.entry.label(),
            match result.config.tool {
                ToolChoice::Adaptive => "adaptive".to_string(),
                ToolChoice::Tool(id) => data()
                    .tools
                    .iter()
                    .find(|tool| tool.id == id)
                    .map_or_else(|| format!("tool {id}"), |tool| tool.name.clone()),
            },
            result.seconds
        ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("spelunk-run"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("spelunk-run"));
    }
    if let Some(error) = &result.error {
        card = card.child(
            Alert::new("spelunk-run-error", format!("The run stopped: {error}"))
                .with_variant(Tone::Danger.into())
                .banner(),
        );
    }
    let mut badges = div()
        .id("spelunk-run-badges")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(tag(format!("Depth {}", result.depth), Tone::Accent))
        .child(tag(
            format!("{} Amber", format::value(amber)),
            Tone::Neutral,
        ))
        .child(tag(
            format!("{} actions · {} broken", result.actions, result.broken),
            Tone::Neutral,
        ))
        .child(tag(
            format!("{} hits / {} misses", result.hits, result.misses),
            Tone::Neutral,
        ));
    if result.grands > 0.0 {
        badges = badges.child(tag(
            format!("{} Grand Discoveries", format::value(result.grands)),
            Tone::Success,
        ));
    }
    if result.status == Status::Blocked {
        badges = badges.child(tag("Blocked: no way down", Tone::Warning));
    }
    if !result.settled {
        badges = badges.child(tag("Loot not collected", Tone::Warning));
    }
    let summary = format!(
        "{} · depth {} from {} · {} Amber · seed {}",
        result.status.label(),
        result.depth,
        result.start_depth,
        format::value(amber),
        result.seed
    );
    let kpis = KpiStrip::new()
        .child(
            Statistic::new(
                "spelunk-run-depth",
                "Reached depth",
                result.depth.to_string(),
            )
            .caption(format!("started on {}", result.start_depth)),
        )
        .child(
            Statistic::new(
                "spelunk-run-amber",
                if result.settled {
                    "Collected Amber"
                } else {
                    "Uncollected Amber"
                },
                format::value(amber),
            )
            .caption(format!("{} found", format::value(result.gross_rewards[0]))),
        )
        .child(
            Statistic::new(
                "spelunk-run-time",
                "Expedition time",
                format!("{:.1}s", result.seconds),
            )
            .caption(format!("{} actions", result.actions)),
        )
        .child(
            Statistic::new(
                "spelunk-run-recovery",
                "Recovery to start",
                format!("{recovered:.2}h"),
            )
            .caption("At the account's current regeneration"),
        )
        .child(
            Statistic::new(
                "spelunk-run-stamina",
                "Stamina spent",
                format::value(result.spent),
            )
            .caption(format!(
                "{} resisted",
                format::value(result.diagnostics.resisted_stamina)
            )),
        );
    card.child(
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_3()
            .child(
                div()
                    .id("spelunk-run-summary")
                    .role(Role::Label)
                    .aria_label(summary)
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(badges),
            )
            .child(div().id("spelunk-run-kpis").child(kpis))
            .child(rewards_table(result))
            .child(stamina_table(result))
            .child(members_table(result))
            .children(amber_details(result))
            .child(events_list(state, result))
            .child(exit_note(result)),
    )
    .into_any_element()
}

/// A kit table with `heads`; the columns flagged numeric align right. Every
/// kit table has the same element id, so each sits under an `id` of its own.
fn table(
    id: &'static str,
    heads: &[(&'static str, bool)],
    rows: Vec<TableRow>,
) -> impl IntoElement {
    div().id(id).child(
        Table::new()
            .child(
                TableHeader::new().child(TableRow::new().children(heads.iter().map(
                    |(head, numeric)| {
                        TableHead::new()
                            .when(*numeric, |head| head.text_right())
                            .child(*head)
                    },
                ))),
            )
            .child(TableBody::new().children(rows)),
    )
}

/// A row whose first cell carries `id`, then the right-aligned figures.
fn row(key: (&'static str, usize), id: String, first: String, figures: Vec<String>) -> TableRow {
    TableRow::new()
        .id(key)
        .child(TableCell::new().child(div().id(id).child(first)))
        .children(
            figures
                .into_iter()
                .map(|figure| TableCell::new().text_right().child(figure)),
        )
}

fn section(title: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_size(TEXT_BODY).child(title))
}

/// The Amber policy's phase record: every change, its reason and its oranges.
fn amber_details(result: &RunResult) -> Option<AnyElement> {
    let amber = result.amber.as_ref()?;
    // A handful of phase changes and one activation per orange on the belt.
    let transitions = amber
        .transitions
        .iter()
        .enumerate()
        .map(|(index, transition)| {
            TableRow::new()
                .id(("spelunk-amber-transition", index))
                .child(
                    TableCell::new().child(
                        div()
                            .id(format!("spelunk-run-amber-transition-{index}"))
                            .child(format!("D{}", transition.depth)),
                    ),
                )
                .child(TableCell::new().child(transition.phase.label()))
                .child(TableCell::new().child(transition.reason.clone()))
        })
        .collect();
    let activations = amber
        .activations
        .iter()
        .enumerate()
        .map(|(index, activation)| {
            row(
                ("spelunk-amber-activation", index),
                format!("spelunk-run-amber-activation-{index}"),
                format!("D{}", activation.depth),
                vec![
                    format::value(activation.spent),
                    format!("{}x", format::value(activation.effect)),
                ],
            )
        })
        .collect();
    Some(
        div()
            .id("spelunk-run-amber")
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id("spelunk-run-amber-phase")
                    .role(Role::Label)
                    .aria_label(format!("Amber policy ended in {}", amber.phase.label()))
                    .text_size(TEXT_BODY)
                    .child(format!("Amber policy · ended in {}", amber.phase.label())),
            )
            .child(table(
                "spelunk-run-amber-transitions",
                &[("Depth", false), ("Phase", false), ("Reason", false)],
                transitions,
            ))
            .when(amber.activations.is_empty(), |section| {
                section.child(Caption::new("No oranges consumed."))
            })
            .when(!amber.activations.is_empty(), |details| {
                details.child(section("Orange activations").child(table(
                    "spelunk-run-amber-activations",
                    &[
                        ("Depth", false),
                        ("Stamina spent", true),
                        ("Amber effect", true),
                    ],
                    activations,
                )))
            })
            .child(Caption::new(format!(
                "{} deliberate burn digs · {} native spent-stamina credit.",
                amber.dump_digs,
                format::value(amber.dump_spent)
            )))
            .into_any_element(),
    )
}

/// How the run banked its loot, or why it kept none.
fn exit_note(result: &RunResult) -> AnyElement {
    let text = match result.exit.as_ref().map(|exit| exit.kind) {
        Some(ExitKind::Rope) => format!(
            "Rope exit: the party kept every reward whole and Amber at {}x. The Amber policy left \
             early instead of paying the Delve toll.",
            format::value(
                result
                    .exit
                    .as_ref()
                    .map_or(1.0, |exit| exit.amber_multiplier)
            ),
        ),
        Some(ExitKind::Toll) => "A run banks its loot only when the whole party runs out of \
             stamina and the floor transition ends: the Delve toll keeps the retained share and \
             rolls each reward's fractional part for one extra item. A blocked run keeps nothing."
            .to_string(),
        None => "This run banked no loot: a blocked run keeps nothing.".to_string(),
    };
    Caption::new(text).into_any_element()
}

/// The game's reward kinds the run found.
fn rewards_table(result: &RunResult) -> AnyElement {
    let rows: Vec<TableRow> = data()
        .rewards
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let gross = result.gross_rewards.get(index).copied().unwrap_or(0.0);
            let collected = result.rewards.get(index).copied().unwrap_or(0.0);
            (gross != 0.0 || collected != 0.0).then(|| {
                row(
                    ("spelunk-reward", index),
                    format!("spelunk-run-reward-{index}"),
                    name.clone(),
                    vec![format::value(gross), format::value(collected)],
                )
            })
        })
        .collect();
    let any = !rows.is_empty();
    section("Rewards")
        .when(any, |section| {
            section.child(table(
                "spelunk-run-rewards",
                &[("Reward", false), ("Found", true), ("Collected", true)],
                rows,
            ))
        })
        .when(!any, |section| {
            section.child(Caption::new("This run found no rewards."))
        })
        .into_any_element()
}

/// One row per party member.
fn stamina_table(result: &RunResult) -> AnyElement {
    let rows = result
        .stamina
        .iter()
        .enumerate()
        .map(|(index, member)| {
            row(
                ("spelunk-stamina", index),
                format!("spelunk-run-stamina-{}", member.id),
                format!("Character {}", member.id + 1),
                vec![
                    format::value(member.start),
                    format::value(member.end),
                    format::value(member.max),
                ],
            )
        })
        .collect();
    section("Party stamina")
        .child(table(
            "spelunk-run-stamina-table",
            &[
                ("Character", false),
                ("Start", true),
                ("End", true),
                ("Maximum", true),
            ],
            rows,
        ))
        .into_any_element()
}

/// One row per party member.
fn members_table(result: &RunResult) -> AnyElement {
    let rows = result
        .diagnostics
        .members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let mut flags = Vec::new();
            if member.unused {
                flags.push("unused");
            }
            if member.exhausted {
                flags.push("exhausted");
            }
            if member.no_advance {
                flags.push("no descent");
            }
            if member.no_further_floor {
                flags.push("no further floor");
            }
            if member.resist_stall {
                flags.push("resist stall");
            }
            row(
                ("spelunk-member", index),
                format!("spelunk-run-member-{}", member.id),
                format!("Character {}", member.id + 1),
                vec![
                    member
                        .start_depth
                        .map_or_else(|| "—".to_string(), |depth| format!("D{depth}")),
                    format::value(member.digs),
                    format::value(member.spent),
                    format::value(member.resisted_stamina),
                    format::value(member.hits),
                    format::value(member.misses),
                    format::value(member.damage),
                    format::value(member.broken),
                    format::value(member.descents),
                    format::value(member.rewards[0]),
                ],
            )
            .child(TableCell::new().child(if flags.is_empty() {
                "—".to_string()
            } else {
                flags.join(", ")
            }))
        })
        .collect();
    section("Party contributions")
        .child(table(
            "spelunk-run-members",
            &[
                ("Member", false),
                ("Joined", true),
                ("Digs", true),
                ("Spent", true),
                ("Resisted", true),
                ("Hits", true),
                ("Misses", true),
                ("Damage", true),
                ("Broken", true),
                ("Descents", true),
                ("Amber", true),
                ("Flags", false),
            ],
            rows,
        ))
        .child(Caption::new(
            "No further floor means the run ended on the floor the member joined. Resist stall \
             means the member spent stamina only on resisted hits and broke nothing.",
        ))
        .into_any_element()
}

/// The event log: a virtualized table whose rows were built when the run was
/// stored.
fn events_list(state: &AppState, result: &RunResult) -> AnyElement {
    let shown = result.events.len();
    let total = shown + result.events_dropped;
    section("Event log")
        .child(
            div()
                .id("spelunk-run-events")
                .h(px(320.0))
                .child(DataTable::new(&state.spelunking_controls.events)),
        )
        .child(Caption::new(if shown < total {
            format!("Showing the first {shown} of {total} events.")
        } else {
            format!("{total} events.")
        }))
        .into_any_element()
}
