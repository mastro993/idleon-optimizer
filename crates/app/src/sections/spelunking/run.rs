//! The expedition run: its controls, the run in flight and the results.
//!
//! Upstream runs the expedition in a module worker. This section runs the same
//! simulation through the shared [`crate::task::TaskRunner`], so the progress
//! card, the cancel action and the stale marker are the ones every other
//! section uses. A run is deterministic: the seed, the cave, the party, the belt
//! and the memorized elixirs fully describe it, so the result says which seed
//! produced it.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::lists::{List, ListItem};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_core::spelunking::{
    Account, Entry, EventKind, Inputs, Policy, Requested, RunResult, Status, ToolChoice, data,
};

/// What a run was produced from; a change marks the result stale.
#[derive(Clone, Debug, PartialEq)]
pub struct RunInputs {
    pub requested: Requested,
    /// The account values the run reads, which the request does not carry.
    pub account: Account,
    pub seed: u32,
    /// The save this run read, so a new import is stale.
    pub revision: u64,
}

/// How many log lines the card lists before it stops.
const VISIBLE_EVENTS: usize = 200;

/// The run card: controls, the run in flight and the stored result.
pub fn card(state: &AppState, inputs: &Inputs, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.spelunk_simulation(cx);
    let amber = state.spelunking.policy == Policy::AmberFarm;
    let mut body = div()
        .id("spelunk-run-panel")
        .flex()
        .flex_col()
        .gap_3()
        .child(controls(state, inputs, &task, cx));
    if amber {
        body = body.child(
            ely_gpui_component::feedback::Banner::new(
                "spelunk-run-amber",
                ely_gpui_component::primitives::Severity::Warning,
                "Phased Amber farming — its dump depth, finishers, reserve, harvest and rope \
                 reserve phases — arrives with SP-03. Pick another policy to run an expedition.",
            )
            .into_any_element(),
        );
    }
    let mut body = body.child(
        Caption::new(
            "The run replays the game's 10 ms tick with a seeded generator: the same seed, \
             cave, party, belt and memorized elixirs reproduce it exactly. Progress and the \
             events log are the accessible record; the canvas replay is not ported yet.",
        )
        .text_color(muted),
    );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(crate::ui::task::progress_card(
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
        body = body.child(crate::ui::task::cancelled_banner(
            "spelunk-run-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(result) = task.result.clone() {
        body = body.child(results_card(
            &result,
            task.stale,
            state.spelunking.account.regen,
            cx,
        ));
    }
    body.into_any_element()
}

/// The seed, reaction time, tool and entry, and the run actions.
fn controls(
    state: &AppState,
    inputs: &Inputs,
    task: &crate::task::TaskState<RunResult>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let seed_entity = entity.clone();
    let reaction_entity = entity.clone();
    let tool_entity = entity.clone();
    let entry_entity = entity.clone();
    let run_entity = entity.clone();
    let cancel_entity = entity;
    let spelunking = &state.spelunking;

    let seed = NumberInput::new("spelunk-run-seed", f64::from(spelunking.sim_seed))
        .range(0.0, f64::from(u32::MAX))
        .step(1.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Seed")
        .on_change(move |value, _window, cx| {
            seed_entity.update(cx, |state, cx| {
                state.set_spelunk_sim_seed(value.max(0.0) as u32, cx);
            });
        });

    let reaction = NumberInput::new("spelunk-run-reaction", spelunking.sim_reaction_ms)
        .range(100.0, 5_000.0)
        .step(10.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Manual reaction (ms)")
        .on_change(move |value, _window, cx| {
            reaction_entity.update(cx, |state, cx| {
                state.set_spelunk_sim_reaction(value, cx);
            });
        });

    let mut tools = vec![Choice::new("adaptive", "Adaptive")];
    tools.extend(
        inputs
            .tools
            .iter()
            .map(|tool| Choice::new(tool.id.to_string(), tool.name.clone())),
    );
    let tool = Select::new("spelunk-run-tool", tools)
        .label("Manual tool")
        .selected(match spelunking.sim_tool {
            ToolChoice::Adaptive => "adaptive".to_string(),
            ToolChoice::Tool(id) => id.to_string(),
        })
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(choice) = ToolChoice::from_id(value.as_ref()) {
                tool_entity.update(cx, |state, cx| state.set_spelunk_sim_tool(choice, cx));
            }
        });

    let entry = Select::new(
        "spelunk-run-entry",
        [
            Choice::new(Entry::Surface.id(), "Surface"),
            Choice::new(Entry::Shortcut.id(), "Saved shortcut"),
        ],
    )
    .label("Entry")
    .selected(spelunking.sim_entry.id())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        if let Some(entry) = Entry::from_id(value.as_ref()) {
            entry_entity.update(cx, |state, cx| state.set_spelunk_sim_entry(entry, cx));
        }
    });

    let run = Button::new(
        "spelunk-run-start",
        if task.running {
            "Running…"
        } else {
            "Run expedition"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(task.running || spelunking.policy == Policy::AmberFarm)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_spelunk_simulation(cx));
    });
    let cancel = Button::new("spelunk-run-stop", "Cancel")
        .variant(ButtonVariant::Subtle)
        .icon(IconName::X)
        .disabled(!task.running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_spelunk_simulation(cx));
        });

    let mut header = CardHeader::new("Expedition run").description(
        "Cave, mode, policy, party and belt come from the controls above; the entry and the \
         manual tool are the run's own choices.",
    );
    if spelunking.policy == Policy::AmberFarm {
        header = header.action(Badge::new("Amber farming is SP-03").tone(Tone::Warning));
    }
    Card::new()
        .header(header)
        .child(
            div()
                .id("spelunk-run-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_4()
                .p_3()
                .child(super::field("Seed", seed))
                .child(super::field("Manual reaction", reaction.into_any_element()))
                .child(super::field("Manual tool", tool.into_any_element()))
                .child(super::field("Entry", entry.into_any_element()))
                .child(div().flex().items_center().gap_2().child(run).child(cancel))
                .child(
                    div().flex().items_center().gap_2().child(
                        Caption::new(format!(
                            "{} · {}",
                            spelunking.policy.label(),
                            if spelunking.manic { "Manic" } else { "Normal" }
                        ))
                        .text_color(muted),
                    ),
                ),
        )
        .into_any_element()
}

/// The stored run: its outcome, loot, stamina, members and event log.
fn results_card(
    result: &RunResult,
    stale: bool,
    regen: f64,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
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
        header = header.action(crate::ui::task::stale_badge("spelunk-run"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(crate::ui::task::stale_banner("spelunk-run"));
    }
    if let Some(error) = &result.error {
        card = card.child(
            ely_gpui_component::feedback::Banner::new(
                "spelunk-run-error",
                ely_gpui_component::primitives::Severity::Danger,
                format!("The run stopped: {error}"),
            )
            .into_any_element(),
        );
    }
    let mut badges = div()
        .id("spelunk-run-badges")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(Badge::new(format!("Depth {}", result.depth)).tone(Tone::Accent))
        .child(Badge::new(format!("{} Amber", format::value(amber))))
        .child(Badge::new(format!(
            "{} actions · {} broken",
            result.actions, result.broken
        )))
        .child(Badge::new(format!(
            "{} hits / {} misses",
            result.hits, result.misses
        )));
    if result.grands > 0.0 {
        badges = badges.child(
            Badge::new(format!(
                "{} Grand Discoveries",
                format::value(result.grands)
            ))
            .tone(Tone::Success),
        );
    }
    if result.status == Status::Blocked {
        badges = badges.child(Badge::new("Blocked: no way down").tone(Tone::Warning));
    }
    if !result.settled {
        badges = badges.child(Badge::new("Loot not collected").tone(Tone::Warning));
    }
    let summary = format!(
        "{} · depth {} from {} · {} Amber · seed {}",
        result.status.label(),
        result.depth,
        result.start_depth,
        format::value(amber),
        result.seed
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
                    .role(gpui::Role::Label)
                    .aria_label(summary)
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(badges),
            )
            .child(
                div()
                    .id("spelunk-run-kpis")
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(kpi(
                        "spelunk-run-depth",
                        "Reached depth",
                        &format!("{}", result.depth),
                        &format!("started on {}", result.start_depth),
                        muted,
                    ))
                    .child(kpi(
                        "spelunk-run-amber",
                        if result.settled {
                            "Collected Amber"
                        } else {
                            "Uncollected Amber"
                        },
                        &format::value(amber),
                        &format!("{} found", format::value(result.gross_rewards[0])),
                        muted,
                    ))
                    .child(kpi(
                        "spelunk-run-time",
                        "Expedition time",
                        &format!("{:.1}s", result.seconds),
                        &format!("{} actions", result.actions),
                        muted,
                    ))
                    .child(kpi(
                        "spelunk-run-recovery",
                        "Recovery to start",
                        &format!("{:.2}h", recovered),
                        "At the account's current regeneration",
                        muted,
                    ))
                    .child(kpi(
                        "spelunk-run-stamina",
                        "Stamina spent",
                        &format::value(result.spent),
                        &format!(
                            "{} resisted",
                            format::value(result.diagnostics.resisted_stamina)
                        ),
                        muted,
                    )),
            )
            .child(rewards_table(result))
            .child(stamina_table(result))
            .child(members_table(result, muted))
            .child(events_list(result, muted))
            .child(
                Caption::new(
                    "A run banks its loot only when the whole party runs out of stamina and the \
                     floor transition ends: the Delve toll keeps the retained share and rolls each \
                     reward's fractional part for one extra item. A blocked run keeps nothing.",
                )
                .text_color(muted),
            ),
    )
    .into_any_element()
}

fn kpi(id: &'static str, label: &str, value: &str, detail: &str, muted: gpui::Hsla) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(150.0))
        .flex_1()
        .child(Caption::new(label).text_color(muted))
        .child(div().text_size(px(18.0)).child(value.to_string()))
        .child(Caption::new(detail.to_string()).text_color(muted))
        .into_any_element()
}

fn rewards_table(result: &RunResult) -> AnyElement {
    let mut table = Table::new(["Reward", "Found", "Collected"]);
    let mut any = false;
    for (index, name) in data().rewards.iter().enumerate() {
        let gross = result.gross_rewards.get(index).copied().unwrap_or(0.0);
        let collected = result.rewards.get(index).copied().unwrap_or(0.0);
        if gross == 0.0 && collected == 0.0 {
            continue;
        }
        any = true;
        table = table.row([
            div()
                .id(format!("spelunk-run-reward-{index}"))
                .child(name.clone())
                .into_any_element(),
            div().child(format::value(gross)).into_any_element(),
            div().child(format::value(collected)).into_any_element(),
        ]);
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_size(px(14.0)).child("Rewards"))
        .when(any, |section| section.child(table))
        .when(!any, |section| {
            section.child(Caption::new("This run found no rewards."))
        })
        .into_any_element()
}

fn stamina_table(result: &RunResult) -> AnyElement {
    let mut table = Table::new(["Character", "Start", "End", "Maximum"]);
    for member in &result.stamina {
        table = table.row([
            div()
                .id(format!("spelunk-run-stamina-{}", member.id))
                .child(format!("Character {}", member.id + 1))
                .into_any_element(),
            div().child(format::value(member.start)).into_any_element(),
            div().child(format::value(member.end)).into_any_element(),
            div().child(format::value(member.max)).into_any_element(),
        ]);
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_size(px(14.0)).child("Party stamina"))
        .child(table)
        .into_any_element()
}

fn members_table(result: &RunResult, muted: gpui::Hsla) -> AnyElement {
    let mut table = Table::new([
        "Member", "Joined", "Digs", "Spent", "Resisted", "Hits", "Misses", "Damage", "Broken",
        "Descents", "Amber", "Flags",
    ]);
    for member in &result.diagnostics.members {
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
        table = table.row([
            div()
                .id(format!("spelunk-run-member-{}", member.id))
                .child(format!("Character {}", member.id + 1))
                .into_any_element(),
            div()
                .child(
                    member
                        .start_depth
                        .map_or_else(|| "—".to_string(), |depth| format!("D{depth}")),
                )
                .into_any_element(),
            div().child(format::value(member.digs)).into_any_element(),
            div().child(format::value(member.spent)).into_any_element(),
            div()
                .child(format::value(member.resisted_stamina))
                .into_any_element(),
            div().child(format::value(member.hits)).into_any_element(),
            div().child(format::value(member.misses)).into_any_element(),
            div().child(format::value(member.damage)).into_any_element(),
            div().child(format::value(member.broken)).into_any_element(),
            div()
                .child(format::value(member.descents))
                .into_any_element(),
            div()
                .child(format::value(member.rewards[0]))
                .into_any_element(),
            div()
                .child(if flags.is_empty() {
                    "—".to_string()
                } else {
                    flags.join(", ")
                })
                .into_any_element(),
        ]);
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_size(px(14.0)).child("Party contributions"))
        .child(table)
        .child(
            Caption::new(
                "No further floor means the run ended on the floor the member joined. Resist \
                 stall means the member spent stamina only on resisted hits and broke nothing.",
            )
            .text_color(muted),
        )
        .into_any_element()
}

fn events_list(result: &RunResult, muted: gpui::Hsla) -> AnyElement {
    let mut list = List::new().divided();
    for (index, event) in result.events.iter().take(VISIBLE_EVENTS).enumerate() {
        let minutes = (event.time / 60.0).floor();
        let seconds = event.time - minutes * 60.0;
        list = list.child(
            ListItem::new(
                format!("spelunk-run-event-{index}"),
                event_label(&event.kind),
            )
            .description(event_detail(&event.kind))
            .trailing(
                Caption::new(format!("D{} · {minutes:.0}:{seconds:04.1}", event.depth))
                    .text_color(muted),
            ),
        );
    }
    let shown = result.events.len().min(VISIBLE_EVENTS);
    let total = result.events.len() + result.events_dropped;
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_size(px(14.0)).child("Event log"))
        .child(list)
        .child(Caption::new(if shown < total {
            format!("Showing the first {shown} of {total} events.")
        } else {
            format!("{total} events.")
        }))
        .into_any_element()
}

fn event_label(kind: &EventKind) -> String {
    match kind {
        EventKind::Floor => "Entered floor".into(),
        EventKind::Dig { .. } => "Dig".into(),
        EventKind::Elixir { id, .. } => data()
            .elixirs
            .get(*id)
            .map_or_else(|| format!("Elixir {id}"), |elixir| elixir.name.clone()),
        EventKind::Switch { character } => format!("Switch to character {}", character + 1),
        EventKind::Boss => "Boss defeated".into(),
        EventKind::Discovery { name } => format!("Discovered {}", name.replace('_', " ")),
        EventKind::GrandDiscovery => "Grand Discovery".into(),
        EventKind::ExitFound => "Exit revealed".into(),
        EventKind::Descend => "Next floor".into(),
        EventKind::Exhausted => "Stamina exhausted".into(),
        EventKind::Blocked => "No way down".into(),
    }
}

fn event_detail(kind: &EventKind) -> String {
    match kind {
        EventKind::Dig {
            tool,
            character,
            used,
        } => format!(
            "{} · {} stamina · character {}",
            data()
                .tools
                .iter()
                .find(|candidate| candidate.id == *tool)
                .map_or_else(|| format!("tool {tool}"), |tool| tool.name.clone()),
            format::value(*used),
            character + 1
        ),
        EventKind::Elixir { slot, .. } => format!("Belt slot {}", slot + 1),
        _ => String::new(),
    }
}
