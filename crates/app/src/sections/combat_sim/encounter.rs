//! The encounter workspace (CS-03) and its results (CS-05).
//!
//! The Simulator options and the Run/Cancel pair run through the shared
//! [`idleon_ui::runner::TaskRunner`], so the progress card, the cancel action
//! and the stale marker are the ones every other section uses. The run itself
//! is [`idleon_combat::encounter`]; the results tabs read the one stored run:
//! the summary, the Killroy practice, the obol fragments, the kills by monster,
//! the cards and the kills timeline. Tabs with nothing to show say why, and the
//! mechanics the run cannot model are listed beside the options instead of
//! being dropped.

use super::ResultsTab;
use super::controls::{duration_choices, player_choices};
use crate::state::AppState;
use crate::ui::layout;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, Hsla, IntoElement, ParentElement, Role, SharedString, Styled,
    Window, div, px,
};
use gpui::{App, RenderOnce};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::chart::LineChart;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_combat::encounter::{self, Outcome};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, PANEL_GAP, PANEL_PADDING, TEXT_CAPTION, TEXT_KPI,
    TEXT_MONO, Tone, show_number,
};

/// The encounter pane: options, the run in flight and its result.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let task = state.combat_encounter(cx);
    let results = state.combat.results;
    let mut body = div()
        .id("combat-encounter-panel")
        .flex()
        .flex_col()
        .gap_3()
        .p(PANEL_PADDING)
        .child(options_card(state, &task, window, cx))
        .child(
            Caption::new(
                "The run replays the game's 10 ms tick over the map's spawn roster: monsters \
                 respawn on their saved timers and the stand-in kills one live monster per \
                 interval. Its card and obol rolls draw one seeded stream, so the map, the \
                 player, the duration and the seed fully describe the run.",
            )
            .text_color(muted),
        );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "combat-encounter-run",
            "Running the encounter",
            "Runs off the UI thread; cancelling keeps the previous result.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_combat_encounter(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "combat-encounter-cancelled",
            task.result.is_some(),
        ));
    }
    match task.result.as_ref() {
        Some(result) => {
            body = body.child(results_card(result, task.stale, results, state, window, cx));
        }
        None if !task.running && !task.cancelled => {
            body = body.child(
                Alert::new(
                    "combat-encounter-empty",
                    "No encounter has run yet. Pick a map, a player and a duration, then Run.",
                )
                .with_variant(Tone::Info.into())
                .banner(),
            );
        }
        None => {}
    }
    body = body.child(unsupported_card(cx));
    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("combat-encounter-scroll")
        .into_any_element()
}

/// The simulator options: encounter, map, player, kill interval and duration.
fn options_card(
    state: &AppState,
    task: &idleon_ui::runner::TaskState<super::RunResult>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let options = state.combat_encounter_options();
    let running = task.running;
    let invalid = encounter::validate(&options).err();
    let valid = invalid.is_none();
    let run = entity.clone();
    let cancel = entity;
    let stand_in = matches!(options.player, encounter::Player::StandIn { .. });
    let kill_every = match options.player {
        encounter::Player::StandIn { kill_every_ms } => kill_every_ms as f64,
        encounter::Player::Idle => 1500.0,
    };

    // The section's values flow into the controls; edits come back through
    // the subscriptions in `Controls::new`.
    let controls = &state.combat.controls;
    controls
        .map
        .show(controls.maps.clone(), Some(&options.map), window, cx);
    controls
        .player
        .show(player_choices(), Some(&!stand_in), window, cx);
    controls.duration.show(
        duration_choices(),
        Some(&(options.duration_ms / 1000)),
        window,
        cx,
    );
    show_number(&controls.kill_every, kill_every, 0, window, cx);
    show_number(&controls.seed, options.seed as f64, 0, window, cx);
    let map_select = Select::new(controls.map.state())
        .id("combat-encounter-map")
        .accessibility_label("Map")
        .small()
        .w(px(220.0));
    let player_select = Select::new(controls.player.state())
        .id("combat-encounter-player")
        .accessibility_label("Player")
        .small()
        .w(px(240.0));
    let duration_select = Select::new(controls.duration.state())
        .id("combat-encounter-duration")
        .accessibility_label("Duration")
        .small()
        .w(px(140.0));
    let kill_input = labelled(
        "Kill every (ms)",
        crate::ui::number_field(
            "combat-encounter-kill",
            &controls.kill_every,
            "Kill every (ms)",
        )
        .w(px(140.0)),
    );
    let seed_input = labelled(
        "Seed",
        crate::ui::number_field("combat-encounter-seed", &controls.seed, "Seed").w(px(140.0)),
    );

    Card::new()
        .header(
            CardHeader::new("Encounter")
                .description(
                    "Normal combat on one map. Every results tab below reads this run; the \
                     auto-mode player AI is not modelled yet, so a run cannot measure character DPS.",
                )
                .action(
                    Tag::new()
                        .with_variant(Tone::Accent.into())
                        .outline()
                        .small()
                        .child("Normal combat"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(PANEL_GAP)
                .p(PANEL_PADDING)
                .child(
                    div()
                        .id("combat-encounter-controls")
                        .flex()
                        .flex_wrap()
                        .items_end()
                        .gap(PANEL_GAP)
                        .child(labelled("Map", map_select))
                        .child(labelled("Player", player_select))
                        .when(stand_in, |row| row.child(kill_input))
                        .child(labelled("Duration", duration_select))
                        .child(seed_input)
                        .child(div().flex_1())
                        .child(
                            Button::new("combat-encounter-run")
                                .label("Run")
                                .primary()
                                .small()
                                .icon(IconName::Play)
                                .disabled(running || !valid)
                                .on_click(move |_event, _window, cx| {
                                    run.update(cx, |state, cx| {
                                        state.run_combat_encounter(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("combat-encounter-cancel")
                                .label("Cancel")
                                .small()
                                .icon(IconName::X)
                                .disabled(!running)
                                .on_click(move |_event, _window, cx| {
                                    cancel.update(cx, |state, cx| {
                                        state.cancel_combat_encounter(cx);
                                    });
                                }),
                        ),
                )
                .child(Caption::new(match options.player {
                    encounter::Player::StandIn { .. } => {
                        "Stand-in player: stands on the spawn platform and, every interval, kills \
                         a live monster for exactly its remaining HP. Spawns and respawns follow \
                         the source; movement, attacks and skills do not."
                    }
                    encounter::Player::Idle => {
                        "Idle player: never attacks. Useful for checking the spawn and respawn \
                         roster without the stand-in's kills."
                    }
                }))
                .when_some(invalid, |card, error| {
                    card.child(Caption::new(error).text_color(muted))
                }),
        )
        .into_any_element()
}

/// A control with its caption above it.
fn labelled(label: &'static str, control: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label))
        .child(control)
        .into_any_element()
}

/// The stored run: its totals, the kills by monster and how far it got.
///
/// One [`RunResult`] feeds every results tab: the summary, Killroy practice,
/// the obol fragments, the kills by monster, the cards and the kills timeline.
/// A tab with nothing to show says why instead of rendering blank.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunResult {
    pub(crate) run: Outcome,
    pub(crate) killroy: Option<idleon_killroy::Record>,
    /// The Den attempt projected at this run's kill rate.
    pub(crate) den: Option<idleon_combat::den::Outcome>,
    /// The config terms the save decoder could not read.
    pub(crate) progress_warnings: Vec<String>,
    /// The captured Killroy replay this run answered, when it was opened from one.
    pub(crate) source: Option<idleon_combat::replay::KillroyReplay>,
}

/// The results: a tab bar over the one stored run and the picked tab's panel.
fn results_card(
    result: &RunResult,
    stale: bool,
    current: ResultsTab,
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let run = &result.run;
    let Some(map) = encounter::map(run.map) else {
        return div().into_any_element();
    };
    let mut header = CardHeader::new(format!("Run result · {}", encounter::map_name(map)))
        .description(format!(
            "{} · {} · {:.0}s of {:.0}s simulated · {} steps · {} frames · seed {}",
            run.scene,
            player_label(run.player),
            run.simulated_ms as f64 / 1000.0,
            run.duration_ms as f64 / 1000.0,
            run.steps,
            run.frames,
            run.seed,
        ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("combat-encounter"));
    }
    let select = cx.entity();
    let selected = ResultsTab::ALL
        .iter()
        .position(|tab| *tab == current)
        .unwrap_or_default();
    let tabs = layout::tab_bar(
        "combat-result-tabs",
        ResultsTab::ALL.map(|tab| (tab.id().into(), tab.label().into())),
        selected,
        move |index, _window, cx| {
            select.update(cx, |state, cx| {
                state.set_combat_results_tab(ResultsTab::ALL[index], cx);
            });
        },
    );
    let mut card = Card::new().header(header).child(
        div()
            .id("combat-result-tabs-row")
            .p(PANEL_PADDING)
            .child(tabs),
    );
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("combat-encounter"));
    }
    let panel = match current {
        ResultsTab::Summary => summary_panel(run, muted),
        ResultsTab::Killroy => killroy_panel(result, muted),
        ResultsTab::Obols => obols_panel(run, muted),
        ResultsTab::Kills => kills_panel(run, muted),
        ResultsTab::Cards => cards_panel(run, muted),
        ResultsTab::Timeline => timeline_panel(run, muted, cx),
        ResultsTab::Den => den_panel(result, muted),
        ResultsTab::Progress => progress_panel(result, muted),
        ResultsTab::Smelting => smelting_panel(run, muted),
        ResultsTab::Replay => super::replay::pane(state, result, window, cx),
    };
    card.child(panel).into_any_element()
}

/// The run's totals and how far it got.
fn summary_panel(run: &Outcome, muted: Hsla) -> AnyElement {
    let kpis = KpiStrip::new()
        .flex_wrap()
        .child(kpi(
            "combat-encounter-kills",
            "Kills",
            &format::count(run.kills),
            &format!(
                "{} kill ticks, {} found nothing",
                format::count(run.kill_ticks()),
                format::count(run.idle_ticks)
            ),
            muted,
        ))
        .child(kpi(
            "combat-encounter-damage",
            "Damage dealt",
            &format::value(run.damage),
            &format!("{}/s over the run", format::value(run.damage_per_second())),
            muted,
        ))
        .child(kpi(
            "combat-encounter-rate",
            "Kills / hour",
            &format::value(run.kills_per_hour()),
            "Kills × 3,600 ÷ simulated seconds",
            muted,
        ))
        .child(kpi(
            "combat-encounter-interval",
            "Mean kill interval",
            &run.kill_interval_ms().map_or_else(
                || "—".to_string(),
                |interval| format!("{:.2}s", interval / 1000.0),
            ),
            "Simulated time ÷ kills",
            muted,
        ));
    div()
        .id("combat-summary-panel")
        .p(PANEL_PADDING)
        .child(div().id("combat-encounter-kpis").child(kpis))
        .into_any_element()
}

/// The Killroy practice: the same run's kill rate through the room's timer.
fn killroy_panel(result: &RunResult, muted: Hsla) -> AnyElement {
    let run = &result.run;
    let Some(record) = result.killroy else {
        return empty_panel(
            "combat-killroy-empty",
            "Killroy needs kills to project from: this run killed nothing. Run the stand-in              player on a map with monsters.",
            muted,
        );
    };
    let seconds = (run.simulated_ms as f64 / 1000.0).max(1.0);
    let kpis = KpiStrip::new()
        .flex_wrap()
        .child(kpi(
            "combat-killroy-kills",
            "Kills",
            &format::count(record.kills),
            "The timed attempt's final score",
            muted,
        ))
        .child(kpi(
            "combat-killroy-skulls",
            "Skulls collected",
            &format::count(record.skulls_picked),
            &format!("{} dropped", format::count(record.skulls_dropped)),
            muted,
        ))
        .child(kpi(
            "combat-killroy-run",
            "Run length",
            &format::duration(record.seconds),
            "100 s + the saved Time upgrade",
            muted,
        ))
        .child(kpi(
            "combat-killroy-spawned",
            "Extra monsters spawned",
            &format::count(record.spawned),
            "One queued arrival every 100 ms while the queue holds one",
            muted,
        ))
        .child(kpi(
            "combat-killroy-talents",
            "Talent points",
            &format::count(record.talent_points),
            "Talent-point drops the skull rolls leave behind",
            muted,
        ))
        .child(kpi(
            "combat-killroy-credits",
            "Dungeon credits",
            &format::count(record.dungeon_credits),
            "Credit drops that fit the bag",
            muted,
        ))
        .child(kpi(
            "combat-killroy-pearls",
            "Pearls",
            &format::count(record.pearls),
            "Pearl drops that fit the bag",
            muted,
        ));
    div()
        .id("combat-killroy-panel")
        .p(PANEL_PADDING)
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(div().id("combat-killroy-kpis").child(kpis))
        .child(
            Caption::new(format!(
                "Killroy practice projects this run's {:.2} kills/s onto the room's native timer, \
             seeded with {}: the weekly rooms and reward choices are not applied, and the skull \
             count is a per-attempt total, not an hourly yield.",
                run.kills as f64 / seconds,
                run.seed,
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// The obol fragments the run's obol drops rolled up.
fn obols_panel(run: &Outcome, muted: Hsla) -> AnyElement {
    let obols = &run.drops;
    let hours = (run.simulated_ms as f64 / 3_600_000.0).max(1.0 / 3_600.0);
    let kpis = KpiStrip::new()
        .flex_wrap()
        .child(kpi(
            "combat-obols-fragments",
            "Fragments earned",
            &format::number(obols.fragments_earned),
            &format!("{}/hour", format::number(obols.fragments_earned / hours)),
            muted,
        ))
        .child(kpi(
            "combat-obols-trashed",
            "Obols auto-trashed",
            &format::count(obols.fragments_auto_trashed),
            "Full obol pages only; Hyper obols are excluded",
            muted,
        ))
        .child(kpi(
            "combat-obols-balance",
            "Ending fragment balance",
            &if obols.fragments_known {
                format::count(obols.fragments_balance as u64)
            } else {
                "Unavailable".to_string()
            },
            "Capped at 2 billion",
            muted,
        ));
    let note = if obols.auto_trash {
        "Silver Jellyfish auto-trash is unlocked. Empty obol slots fill before the auto-trash; \
         each eligible overflow pickup rolls for up to 3 fragments."
    } else {
        "Silver Jellyfish auto-trash is not unlocked; an obol drop only fills a blank page slot."
    };
    let mut panel = div()
        .id("combat-obols-panel")
        .p(PANEL_PADDING)
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(div().id("combat-obols-kpis").child(kpis))
        .child(Caption::new(note).text_color(muted));
    if !obols.fragments_known {
        panel = panel.child(
            Caption::new("The save carried no fragment balance; the gains are still reported.")
                .text_color(muted),
        );
    }
    if obols.obols_unpicked > 0 {
        panel = panel.child(
            Caption::new(format!(
                "{} obol drop(s) needed a bag slot and were not collected; the inventory is not \
                 modelled.",
                obols.obols_unpicked
            ))
            .text_color(muted),
        );
    }
    panel.into_any_element()
}

/// The Den attempt: the hound waves replayed at this run's kill rate.
fn den_panel(result: &RunResult, muted: Hsla) -> AnyElement {
    let Some(den) = result.den.as_ref() else {
        return empty_panel(
            "combat-den-empty",
            "The Den attempt is projected from a kill rate: pick the fixed kill interval player and \
             run again.",
            muted,
        );
    };
    let snapshot = den.snapshot;
    let totals = den.totals;
    let kpis = KpiStrip::new()
        .flex_wrap()
        .child(kpi(
            "combat-den-score",
            "Den score",
            &format::value(snapshot.score),
            den.status(),
            muted,
        ))
        .child(kpi(
            "combat-den-kills",
            "Hounds killed",
            &format::count(snapshot.kills as u64),
            &format!(
                "{} across {} attempt(s)",
                format::count(totals.hounds_killed as u64),
                format::count(totals.runs as u64)
            ),
            muted,
        ))
        .child(kpi(
            "combat-den-spawned",
            "Hounds spawned",
            &format::count(totals.hounds_spawned as u64),
            &format!("Wave {}", format::count(snapshot.wave as u64)),
            muted,
        ))
        .child(kpi(
            "combat-den-fear",
            "Fear",
            &format!(
                "{} / {}",
                format::value(snapshot.fear),
                format::value(snapshot.fear_max)
            ),
            &format!("{} incoming fear tokens", format::value(snapshot.tokens)),
            muted,
        ))
        .child(kpi(
            "combat-den-chain",
            "Skull chain",
            &format::count(snapshot.chain as u64),
            "Skull hounds killed this attempt",
            muted,
        ))
        .child(kpi(
            "combat-den-opals",
            "Opals earned",
            &format::count(totals.opals as u64),
            "Awarded after the attempt ends",
            muted,
        ));
    div()
        .id("combat-den-panel")
        .p(PANEL_PADDING)
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(div().id("combat-den-kpis").child(kpis))
        .child(
            Caption::new(format!(
                "Amplifier multiplier ×{} · the projection replays the cavern's hound waves at this \
                 run's kill rate; the map's other monsters and the player's movement are not part of \
                 the attempt.",
                format::value(snapshot.amp_multi),
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// The combat-driven account progress the run banked.
fn progress_panel(result: &RunResult, muted: Hsla) -> AnyElement {
    let progress = &result.run.progress;
    let kpis = KpiStrip::new()
        .flex_wrap()
        .child(kpi(
            "combat-progress-forge-time",
            "Forge time from kills",
            &format!("{} s", format::value(progress.forge_seconds)),
            "Extra smelting time; not hours",
            muted,
        ))
        .child(kpi(
            "combat-progress-pet-time",
            "Pet time from kills",
            &format!("{} s", format::value(progress.pet_seconds)),
            &format!(
                "{} s awaiting the greater-than-10-second batch",
                format::value(progress.pet_pending_seconds)
            ),
            muted,
        ))
        .child(kpi(
            "combat-progress-alchemy-time",
            "Alchemy time from kills",
            &format!("{} s", format::value(progress.alchemy_seconds)),
            "Credited time only; production is not simulated",
            muted,
        ))
        .child(kpi(
            "combat-progress-scurge",
            "Scurge EXP",
            &format::value(progress.scurge_exp),
            if progress.scurge.is_some() {
                "Banked; the Scurge menu processes levels"
            } else {
                "Scurge not unlocked"
            },
            muted,
        ));
    let mut panel = div()
        .id("combat-progress-panel")
        .p(PANEL_PADDING)
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(div().id("combat-progress-kpis").child(kpis));
    if let Some(scurge) = progress.scurge {
        panel = panel.child(
            Caption::new(format!(
                "Scurge level {}: {} / {} EXP. Menu-gated levels are not auto-claimed.",
                format::value(scurge.level),
                format::value(scurge.exp),
                format::value(scurge.requirement),
            ))
            .text_color(muted),
        );
    }
    if let Some(coral) = &progress.coral {
        panel = panel.child(
            Caption::new(format!(
                "Coral Kid Singularity: {} kills banked this run ({} total).",
                format::value(progress.coral_kills),
                format::value(coral.kills),
            ))
            .text_color(muted),
        );
    }
    if progress.omega_balls > 0.0 {
        panel = panel.child(
            Caption::new(format!(
                "Omega Arcade balls earned: {}.",
                format::count(progress.omega_balls as u64)
            ))
            .text_color(muted),
        );
    }
    if !progress.pets.is_empty() {
        let mut table = RunTable::new(["Pet", "Shiny days gained", "Level", "World"]);
        for pet in &progress.pets {
            table = table.row([
                div()
                    .id(SharedString::from(format!(
                        "combat-progress-pet-{}-{}",
                        pet.world, pet.index
                    )))
                    .role(Role::Label)
                    .aria_label(format!("{}: {} shiny days", pet.name, pet.gained))
                    .child(SharedString::from(pet.name.clone()))
                    .into_any_element(),
                div().child(format::value(pet.gained)).into_any_element(),
                div()
                    .child(format!(
                        "{} → {}",
                        format::value(pet.start_level),
                        format::value(pet.level)
                    ))
                    .into_any_element(),
                div().child(format!("{}", pet.world + 1)).into_any_element(),
            ]);
        }
        panel = panel.child(Caption::new("Shiny-pet progress from kills").text_color(muted));
        panel = panel.child(table);
    }
    for notice in &progress.notices {
        panel = panel.child(Caption::new(notice.text.clone()).text_color(muted));
    }
    for warning in &result.progress_warnings {
        panel = panel.child(Caption::new(warning.clone()).text_color(muted));
    }
    panel.into_any_element()
}

/// The bars the saved furnace produced during the run.
fn smelting_panel(run: &Outcome, muted: Hsla) -> AnyElement {
    let progress = &run.progress;
    let hours = (run.simulated_ms as f64 / 3_600_000.0).max(1.0 / 3_600.0);
    if !progress.forge_available {
        return div()
            .id("combat-smelting-panel")
            .p(PANEL_PADDING)
            .flex()
            .flex_col()
            .gap(PANEL_GAP)
            .child(
                Alert::new(
                    "combat-smelting-unavailable",
                    "Forge contents are unavailable; only earned smelting time is reported.",
                )
                .with_variant(Tone::Warning.into())
                .banner(),
            )
            .child(
                Caption::new(format!(
                    "{} s of forge time from kills. The saved furnace contents are needed to \
                     produce bars.",
                    format::value(progress.forge_seconds),
                ))
                .text_color(muted),
            )
            .into_any_element();
    }
    let mut rows: Vec<(&String, &f64)> = progress.bars.iter().collect();
    rows.sort_by(|left, right| right.1.total_cmp(left.1).then_with(|| left.0.cmp(right.0)));
    let mut table = RunTable::new(["Bar", "Produced", "Per hour"]);
    for (bar, qty) in &rows {
        let name = idleon_combat::progress::item_name(bar);
        table = table.row([
            div()
                .id(SharedString::from(format!("combat-smelting-{bar}")))
                .role(Role::Label)
                .aria_label(format!("{name}: {qty} produced"))
                .text_size(TEXT_MONO)
                .child(SharedString::from(name))
                .into_any_element(),
            div().child(format::value(**qty)).into_any_element(),
            div().child(format::value(**qty / hours)).into_any_element(),
        ]);
    }
    let mut panel = div()
        .id("combat-smelting-panel")
        .p(PANEL_PADDING)
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(
            Caption::new(
                "Ordinary online smelting plus the forge time the kills granted. Ore, oil, storage \
                 refills, gem slots and fractional-output rolls advance during the run; bars remain \
                 in the furnace, not the drop inventory.",
            )
            .text_color(muted),
        );
    if rows.is_empty() {
        panel = panel.child(empty_panel(
            "combat-smelting-empty",
            "No completed bars: the saved furnace had no ore it could smelt during this run.",
            muted,
        ));
    } else {
        panel = panel.child(table);
    }
    panel.into_any_element()
}

/// The kills by monster, most kills first and ties by monster id.
fn kills_panel(run: &Outcome, muted: Hsla) -> AnyElement {
    if run.kills_by_type.is_empty() {
        return empty_panel(
            "combat-kills-empty",
            "The run killed nothing: the idle player never attacks.",
            muted,
        );
    }
    let hours = (run.simulated_ms as f64 / 3_600_000.0).max(1.0 / 3_600.0);
    let mut rows: Vec<(&String, &u64)> = run.kills_by_type.iter().collect();
    rows.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
    let mut table = RunTable::new(["Monster", "Kills", "Kills / hr", "Share"]);
    let total = run.kills.max(1);
    for (kind, kills) in rows {
        let name = idleon_combat::drops::monster_name(kind);
        table = table.row([
            div()
                .id(SharedString::from(format!("combat-kill-type-{kind}")))
                .role(Role::Label)
                .aria_label(format!("{name}: {kills} kills"))
                .text_size(TEXT_MONO)
                .child(SharedString::from(name))
                .into_any_element(),
            div().child(format::count(*kills)).into_any_element(),
            div()
                .child(format::value(*kills as f64 / hours))
                .into_any_element(),
            div()
                .child(format!("{:.1}%", *kills as f64 * 100.0 / total as f64))
                .into_any_element(),
        ]);
    }
    div()
        .id("combat-kills-panel")
        .p(PANEL_PADDING)
        .child(table)
        .into_any_element()
}

/// The cards the run's kills dropped.
fn cards_panel(run: &Outcome, muted: Hsla) -> AnyElement {
    let cards = &run.drops.cards;
    if cards.is_empty() {
        return empty_panel(
            "combat-cards-empty",
            "No cards dropped in this run. Cards roll on a monster's drop rows; a longer run or \
             a higher Drop rate in the Mechanics tab finds more.",
            muted,
        );
    }
    let hours = (run.simulated_ms as f64 / 3_600_000.0).max(1.0 / 3_600.0);
    let mut rows: Vec<(&String, &f64)> = cards.iter().collect();
    rows.sort_by(|left, right| right.1.total_cmp(left.1).then_with(|| left.0.cmp(right.0)));
    let mut table = RunTable::new(["Card", "Found", "Per hour"]);
    for (kind, found) in rows {
        let name = format!("{} card", idleon_combat::drops::monster_name(kind));
        table = table.row([
            div()
                .id(SharedString::from(format!("combat-card-{kind}")))
                .role(Role::Label)
                .aria_label(format!("{name}: {found} found"))
                .text_size(TEXT_MONO)
                .child(SharedString::from(name))
                .into_any_element(),
            div().child(format::value(*found)).into_any_element(),
            div().child(format::value(found / hours)).into_any_element(),
        ]);
    }
    div()
        .id("combat-cards-panel")
        .p(PANEL_PADDING)
        .child(table)
        .into_any_element()
}

/// The kills over time, one point per ten simulated seconds plus the end.
fn timeline_panel(run: &Outcome, muted: Hsla, cx: &Context<AppState>) -> AnyElement {
    if run.kills == 0 || run.timeline.len() < 2 {
        return empty_panel(
            "combat-timeline-empty",
            "The chart needs a kill and at least two ten-second samples: a run of ten seconds or more \
             fills it, and the idle player never attacks.",
            muted,
        );
    }
    let chart = div().h(px(240.0)).child(
        LineChart::new(run.timeline.clone())
            .id("combat-kills-timeline")
            .name("Kills")
            .x(|sample: &encounter::Sample| format::duration(sample.t_ms as f64 / 1000.0))
            .y(|sample: &encounter::Sample| sample.kills as f64)
            .y_tick_format(|value| format::count(value.max(0.0) as u64))
            .stroke(cx.theme().chart_1),
    );
    div()
        .id("combat-timeline-panel")
        .p(PANEL_PADDING)
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(chart)
        .child(Caption::new(format!(
            "Cumulative kills at every ten simulated seconds, ending on the run's own {} kills at \
             {}. Every point is the run's timeline, not an average.",
            format::count(run.kills),
            format::duration(run.simulated_ms as f64 / 1000.0)
        ))
        .text_color(muted))
        .into_any_element()
}

/// A results tab with nothing to show yet, and why.
pub(crate) fn empty_panel(id: &'static str, message: &str, muted: Hsla) -> AnyElement {
    div()
        .id(id)
        .p(PANEL_PADDING)
        .text_size(TEXT_MONO)
        .text_color(muted)
        .child(SharedString::from(message.to_string()))
        .into_any_element()
}

/// The mechanics this build does not model, named rather than dropped.
fn unsupported_card(cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(
            CardHeader::new("Not simulated")
                .description(
                    "A run reports only what this build models. These parts of the game's \
                     combat are left out on purpose and listed here, not silently ignored.",
                )
                .action(
                    Tag::new()
                        .with_variant(Tone::Warning.into())
                        .outline()
                        .small()
                        .child(format!("{} items", encounter::unsupported().len())),
                ),
        )
        .child(
            div()
                .id("combat-encounter-unsupported")
                .role(Role::Label)
                .aria_label(encounter::unsupported().join(" · "))
                .flex()
                .flex_col()
                .gap_1()
                .p(PANEL_PADDING)
                .children(encounter::unsupported().iter().map(|item| {
                    div()
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(SharedString::from(format!("• {item}")))
                })),
        )
        .into_any_element()
}

/// One headline figure in a strip. Its label reads "<label> <value>", which
/// the harness parses.
fn kpi(
    id: &'static str,
    label: &'static str,
    value: &str,
    caption: &str,
    muted: Hsla,
) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{label} {value}"))
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(140.0))
        .child(Caption::new(label).text_color(muted))
        .child(
            div()
                .text_size(TEXT_KPI)
                .font_weight(FontWeight::MEDIUM)
                .child(SharedString::from(value.to_string())),
        )
        .child(Caption::new(caption.to_string()).text_color(muted))
        .into_any_element()
}

/// A run's small table: the map roster, the furnace's bars or the pets bound
/// its rows, so the kit `Table` draws them all. Every column after the first
/// is a number, right-aligned.
#[derive(IntoElement)]
struct RunTable {
    headers: Vec<&'static str>,
    rows: Vec<Vec<AnyElement>>,
}

impl RunTable {
    fn new<const N: usize>(headers: [&'static str; N]) -> Self {
        Self {
            headers: headers.to_vec(),
            rows: Vec::new(),
        }
    }

    fn row<const N: usize>(mut self, cells: [AnyElement; N]) -> Self {
        self.rows.push(cells.into());
        self
    }
}

impl RenderOnce for RunTable {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // Short row ids: the kit's own ids carry type paths that outgrow the
        // bridge's 256-byte limit, and a row over it drops out of the tree.
        let head =
            TableRow::new()
                .id("combat-run-head")
                .children(self.headers.iter().enumerate().map(|(ix, name)| {
                    TableHead::new()
                        .when(ix > 0, |head| head.text_right())
                        .child(*name)
                }));
        Table::new()
            .small()
            .child(TableHeader::new().child(head))
            .child(
                TableBody::new().children(self.rows.into_iter().enumerate().map(|(row, cells)| {
                    TableRow::new().id(("combat-run-row", row)).children(
                        cells.into_iter().enumerate().map(|(ix, cell)| {
                            TableCell::new()
                                .when(ix > 0, |cell| cell.text_right().text_size(TEXT_MONO))
                                .child(cell)
                        }),
                    )
                })),
            )
    }
}

fn player_label(player: &str) -> &str {
    match player {
        "idle" => "Idle player",
        _ => "Fixed kill interval",
    }
}
