//! The kit controls the section owns (`docs/ui-kit/forms.md`) and what render
//! reads from the calculator.
//!
//! Every select and number field is built once here, except the party, belt
//! and memory slots and the maximum-stamina fields, whose count follows the
//! save: those are added the first time a render needs them and kept. An edit
//! comes back through a subscription that calls the section's setter; render
//! pushes the section's values back into the controls.
//!
//! [`Derived`] is everything render would otherwise ask the calculator for on
//! every frame (the save's inputs, the configuration check, the search space,
//! the depth profile and every option list). It is rebuilt only when the save
//! or the section state changes.

use super::chart::Series;
use super::events::Events;
use super::state::{OptimizerMode, State, records};
use super::{OptimizerInputs, RunInputs};
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::table::TableState;
use idleon_spelunking::{
    AmberPolicy, Entry, Inputs, Objective, Policy, PresetKind, Profile, ReferenceTool, RunResult,
    Save, SearchResult, StaminaMode, ToolChoice, data, profile, search_space,
};
use idleon_ui::format;
use idleon_ui::runner::{Ending, TaskEvent, TaskRunner};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};
use std::rc::Rc;

/// The per-rock-type chart colours, the game's own rock palette.
const DEPTH_COLORS: [&str; 16] = [
    "#4dd0e1", "#ffb74d", "#a5d66f", "#df91f0", "#fb8299", "#82aaff", "#ffe082", "#80cbc4",
    "#dca978", "#adadff", "#e4ceff", "#91c4a0", "#f6956a", "#b6d6ee", "#ed95bb", "#ffffff",
];

type Setter = fn(&mut AppState, f64, &mut Context<AppState>);

pub(crate) struct Controls {
    pub(crate) character: ChoiceSelect<usize>,
    pub(crate) cave: ChoiceSelect<usize>,
    pub(crate) mode: ChoiceSelect<bool>,
    pub(crate) policy: ChoiceSelect<Policy>,
    pub(crate) speed: ChoiceSelect<u32>,
    pub(crate) starting: ChoiceSelect<StaminaMode>,
    pub(crate) finishers: ChoiceSelect<u32>,
    pub(crate) depth_tool: ChoiceSelect<ReferenceTool>,
    pub(crate) run_tool: ChoiceSelect<ToolChoice>,
    pub(crate) run_entry: ChoiceSelect<Entry>,
    pub(crate) optimizer_mode: ChoiceSelect<OptimizerMode>,
    pub(crate) objective: ChoiceSelect<Objective>,
    /// One per party, belt and memory slot, added as the save needs them.
    pub(crate) party: Vec<ChoiceSelect<i32>>,
    pub(crate) elixirs: Vec<ChoiceSelect<i32>>,
    pub(crate) memorized: Vec<ChoiceSelect<i32>>,
    pub(crate) power: Entity<InputState>,
    pub(crate) exits: Entity<InputState>,
    pub(crate) amber: Entity<InputState>,
    pub(crate) regen: Entity<InputState>,
    pub(crate) grand: Entity<InputState>,
    pub(crate) dump: Entity<InputState>,
    pub(crate) reserve: Entity<InputState>,
    pub(crate) harvest: Entity<InputState>,
    pub(crate) rope: Entity<InputState>,
    pub(crate) depth_from: Entity<InputState>,
    pub(crate) depth_to: Entity<InputState>,
    pub(crate) inspect: Entity<InputState>,
    pub(crate) seed: Entity<InputState>,
    pub(crate) reaction: Entity<InputState>,
    pub(crate) screening: Entity<InputState>,
    pub(crate) trials: Entity<InputState>,
    pub(crate) optimizer_seed: Entity<InputState>,
    pub(crate) optimizer_from: Entity<InputState>,
    pub(crate) optimizer_to: Entity<InputState>,
    pub(crate) min_rope: Entity<InputState>,
    pub(crate) max_trials: Entity<InputState>,
    /// One per character, added as the save needs them.
    pub(crate) max_stamina: Vec<Entity<InputState>>,
    /// The run's event log, filled when a run is stored.
    pub(crate) events: Entity<TableState<Events>>,
    /// The stored run and search, copied once when each run stores them.
    pub(crate) run: Option<Rc<RunResult>>,
    pub(crate) search: Option<Rc<SearchResult>>,
    derived: Option<((u64, State), Rc<Derived>)>,
    depth: Option<(DepthKey, Rc<Depth>)>,
    subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(
        sim: &Entity<TaskRunner<RunResult, RunInputs>>,
        optimizer: &Entity<TaskRunner<SearchResult, OptimizerInputs>>,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) -> Self {
        let mut subscriptions = Vec::new();
        let character = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&character, cx, |state, id, cx| {
            state.set_spelunk_character(id, cx)
        }));
        let cave = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&cave, cx, |state, id, cx| {
            state.set_spelunk_cave(id, cx)
        }));
        let mode = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&mode, cx, |state, manic, cx| {
            state.set_spelunk_manic(manic, cx)
        }));
        let policy = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&policy, cx, |state, policy, cx| {
            state.set_spelunk_policy(policy, cx)
        }));
        let speed = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&speed, cx, |state, speed, cx| {
            state.set_spelunk_speed(speed, cx)
        }));
        let starting = ChoiceSelect::new(starting_choices(), None, window, cx);
        subscriptions.push(on_pick(&starting, cx, |state, mode, cx| {
            state.set_spelunk_starting(mode, cx)
        }));
        let finishers = ChoiceSelect::new(finisher_choices(), None, window, cx);
        subscriptions.push(on_pick(&finishers, cx, |state, finishers, cx| {
            let amber = AmberPolicy {
                finishers,
                ..state.spelunking.amber
            };
            state.set_spelunk_amber_policy(amber, cx)
        }));
        let depth_tool = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&depth_tool, cx, |state, tool, cx| {
            state.set_spelunk_depth_tool(tool, cx)
        }));
        let run_tool = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&run_tool, cx, |state, tool, cx| {
            state.set_spelunk_sim_tool(tool, cx)
        }));
        let run_entry = ChoiceSelect::new(entry_choices(), None, window, cx);
        subscriptions.push(on_pick(&run_entry, cx, |state, entry, cx| {
            state.set_spelunk_sim_entry(entry, cx)
        }));
        let optimizer_mode = ChoiceSelect::new(optimizer_mode_choices(), None, window, cx);
        subscriptions.push(on_pick(&optimizer_mode, cx, |state, mode, cx| {
            state.set_spelunk_optimizer_mode(mode, cx)
        }));
        let objective = ChoiceSelect::new(Vec::new(), None, window, cx);
        subscriptions.push(on_pick(&objective, cx, |state, objective, cx| {
            state.set_spelunk_optimizer_objective(objective, cx)
        }));

        let mut number = |min: f64, max: f64, step: f64, set: Setter| {
            let field = number_state(min, max, step, window, cx);
            subscriptions.push(on_number(&field, cx, set));
            field
        };
        let power = number(0.0, 1e30, 1.0, |state, value, cx| {
            state.set_spelunk_power(value, cx)
        });
        let exits = number(0.0, 100_000.0, 1.0, |state, value, cx| {
            state.set_spelunk_holes_bonus(value, cx)
        });
        let amber = number(0.0, 1e30, 0.1, |state, value, cx| {
            state.set_spelunk_amber(value, cx)
        });
        let regen = number(0.0, 1e9, 1.0, |state, value, cx| {
            state.set_spelunk_regen(value, cx)
        });
        let grand = number(0.0, 100.0, 0.001, |state, value, cx| {
            state.set_spelunk_grand_pct(value, cx)
        });
        let dump = number(1.0, 10_000.0, 1.0, |state, value, cx| {
            let amber = AmberPolicy {
                dump_depth: value.max(0.0) as u32,
                ..state.spelunking.amber
            };
            state.set_spelunk_amber_policy(amber, cx)
        });
        let reserve = number(25.0, 100.0, 1.0, |state, value, cx| {
            let amber = AmberPolicy {
                reserve_pct: value.max(0.0) as u32,
                ..state.spelunking.amber
            };
            state.set_spelunk_amber_policy(amber, cx)
        });
        let harvest = number(0.0, 100.0, 1.0, |state, value, cx| {
            let amber = AmberPolicy {
                harvest_depths: value.max(0.0) as u32,
                ..state.spelunking.amber
            };
            state.set_spelunk_amber_policy(amber, cx)
        });
        let rope = number(1.0, 100.0, 1.0, |state, value, cx| {
            let amber = AmberPolicy {
                rope_reserve_pct: value.max(0.0) as u32,
                ..state.spelunking.amber
            };
            state.set_spelunk_amber_policy(amber, cx)
        });
        let depth_from = number(1.0, 10_000.0, 1.0, |state, value, cx| {
            state.set_spelunk_depth_from(value.max(0.0) as usize, cx)
        });
        let depth_to = number(1.0, 10_000.0, 1.0, |state, value, cx| {
            state.set_spelunk_depth_to(value.max(0.0) as usize, cx)
        });
        let inspect = number(1.0, 10_000.0, 1.0, |state, value, cx| {
            state.set_spelunk_inspect(value.max(0.0) as usize, cx)
        });
        let seed = number(0.0, f64::from(u32::MAX), 1.0, |state, value, cx| {
            state.set_spelunk_sim_seed(value.max(0.0) as u32, cx)
        });
        let reaction = number(100.0, 5_000.0, 10.0, |state, value, cx| {
            state.set_spelunk_sim_reaction(value, cx)
        });
        let screening = number(1.0, 8.0, 1.0, |state, value, cx| {
            state.set_spelunk_optimizer_screening(value.max(1.0) as usize, cx)
        });
        let trials = number(2.0, 64.0, 1.0, |state, value, cx| {
            state.set_spelunk_optimizer_trials(value.max(2.0) as usize, cx)
        });
        let optimizer_seed = number(0.0, f64::from(u32::MAX), 1.0, |state, value, cx| {
            state.set_spelunk_optimizer_seed(value.max(0.0) as u32, cx)
        });
        let optimizer_from = number(1.0, 10_000.0, 1.0, |state, value, cx| {
            state.set_spelunk_optimizer_from(value.max(1.0) as usize, cx)
        });
        let optimizer_to = number(1.0, 10_000.0, 1.0, |state, value, cx| {
            state.set_spelunk_optimizer_to(value.max(1.0) as usize, cx)
        });
        let min_rope = number(0.0, 100.0, 1.0, |state, value, cx| {
            state.set_spelunk_optimizer_min_rope(value.max(0.0) as usize, cx)
        });
        let max_trials = number(2.0, 256.0, 1.0, |state, value, cx| {
            state.set_spelunk_optimizer_max_trials(value.max(2.0) as usize, cx)
        });

        let events = cx.new(|cx| TableState::new(Events::default(), window, cx));
        // Keep each run's result once, when it is stored, and build the event
        // log's rows then.
        subscriptions.push(cx.subscribe(sim, |state, runner, event: &TaskEvent, cx| {
            if matches!(event, TaskEvent::Finished(Ending::Stored)) {
                let run = runner.read(cx).result().cloned().map(Rc::new);
                let controls = &mut state.spelunking_controls;
                controls.events.update(cx, |table, cx| {
                    table.delegate_mut().show(run.as_deref());
                    cx.notify();
                });
                controls.run = run;
            }
        }));
        subscriptions.push(
            cx.subscribe(optimizer, |state, runner, event: &TaskEvent, cx| {
                if matches!(event, TaskEvent::Finished(Ending::Stored)) {
                    state.spelunking_controls.search =
                        runner.read(cx).result().cloned().map(Rc::new);
                }
            }),
        );

        Self {
            character,
            cave,
            mode,
            policy,
            speed,
            starting,
            finishers,
            depth_tool,
            run_tool,
            run_entry,
            optimizer_mode,
            objective,
            party: Vec::new(),
            elixirs: Vec::new(),
            memorized: Vec::new(),
            power,
            exits,
            amber,
            regen,
            grand,
            dump,
            reserve,
            harvest,
            rope,
            depth_from,
            depth_to,
            inspect,
            seed,
            reaction,
            screening,
            trials,
            optimizer_seed,
            optimizer_from,
            optimizer_to,
            min_rope,
            max_trials,
            max_stamina: Vec::new(),
            events,
            run: None,
            search: None,
            derived: None,
            depth: None,
            subscriptions,
        }
    }

    /// What render reads for `state` and the save at `revision`.
    pub(crate) fn derived(&mut self, revision: u64, state: &State, save: &Save) -> Rc<Derived> {
        if let Some(((known, shown), derived)) = &self.derived
            && *known == revision
            && shown == state
        {
            return derived.clone();
        }
        let inputs = save.inputs(&state.account);
        let depth_key = (
            revision,
            state.requested(),
            state.range(),
            state.account.clone(),
        );
        let depth = match &self.depth {
            Some((known, depth)) if *known == depth_key => depth.clone(),
            _ => {
                let depth = Rc::new(Depth::of(&inputs, state));
                self.depth = Some((depth_key, depth.clone()));
                depth
            }
        };
        let derived = Rc::new(Derived::of(inputs, state, depth));
        self.derived = Some(((revision, state.clone()), derived.clone()));
        derived
    }

    /// Make sure there is a slot select for each of `count` party, belt or
    /// memory slots.
    pub(crate) fn ensure_slots(
        &mut self,
        kind: Slots,
        count: usize,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) {
        let slots = match kind {
            Slots::Party => &mut self.party,
            Slots::Belt => &mut self.elixirs,
            Slots::Memory => &mut self.memorized,
        };
        while slots.len() < count {
            let slot = slots.len();
            let select = ChoiceSelect::new(Vec::new(), None, window, cx);
            self.subscriptions
                .push(on_pick(&select, cx, move |state, id, cx| match kind {
                    Slots::Party => state.set_spelunk_party(slot, id, cx),
                    Slots::Belt => state.set_spelunk_elixir(slot, id, cx),
                    Slots::Memory => state.set_spelunk_memorized(slot, id, cx),
                }));
            slots.push(select);
        }
    }

    /// Make sure there is a maximum-stamina field for each of `count` characters.
    pub(crate) fn ensure_max_stamina(
        &mut self,
        count: usize,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) {
        while self.max_stamina.len() < count {
            let character = self.max_stamina.len();
            let field = number_state(0.0, 1e12, 10.0, window, cx);
            self.subscriptions
                .push(cx.subscribe(&field, move |state, field, event, cx| {
                    if matches!(event, InputEvent::Change)
                        && let Some(value) = number_value(field.read(cx))
                    {
                        state.set_spelunk_max_stamina(character, value, cx);
                    }
                }));
            self.max_stamina.push(field);
        }
    }
}

/// Which slot selects [`Controls::ensure_slots`] fills.
#[derive(Clone, Copy)]
pub(crate) enum Slots {
    Party,
    Belt,
    Memory,
}

fn on_pick<V: Clone + PartialEq + 'static>(
    select: &ChoiceSelect<V>,
    cx: &mut Context<AppState>,
    set: impl Fn(&mut AppState, V, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            if let SelectEvent::Confirm(Some(value)) = event {
                set(state, value.clone(), cx);
            }
        },
    )
}

fn on_number(field: &Entity<InputState>, cx: &mut Context<AppState>, set: Setter) -> Subscription {
    cx.subscribe(field, move |state, field, event, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            set(state, value, cx);
        }
    })
}

/// What render reads from the calculator for one save and section state.
pub(crate) struct Derived {
    pub(crate) inputs: Inputs,
    /// Why the controls cannot run, when they cannot.
    pub(crate) config_error: Option<String>,
    /// How many configurations the search scope allows.
    pub(crate) space: Option<u128>,
    pub(crate) depth: Rc<Depth>,
    /// Why each built-in preset cannot be applied, in `data()` order.
    pub(crate) preset_issues: Vec<Vec<String>>,
    pub(crate) characters: Vec<Choice<usize>>,
    pub(crate) caves: Vec<Choice<usize>>,
    pub(crate) modes: Vec<Choice<bool>>,
    pub(crate) policies: Vec<Choice<Policy>>,
    pub(crate) speeds: Vec<Choice<u32>>,
    pub(crate) depth_tools: Vec<Choice<ReferenceTool>>,
    pub(crate) run_tools: Vec<Choice<ToolChoice>>,
    pub(crate) objectives: Vec<Choice<Objective>>,
    /// One option list per party, belt and memory slot.
    pub(crate) party: Vec<Vec<Choice<i32>>>,
    pub(crate) elixirs: Vec<Vec<Choice<i32>>>,
    pub(crate) memorized: Vec<Vec<Choice<i32>>>,
}

impl Derived {
    fn of(inputs: Inputs, state: &State, depth: Rc<Depth>) -> Self {
        let config_error = state.config(&inputs).err().map(|error| error.to_string());
        let space = config_error
            .is_none()
            .then(|| {
                search_space(
                    &inputs,
                    &state.run_requested(),
                    &state.optimizer_requested(),
                )
                .ok()
            })
            .flatten()
            .map(|(count, _)| count);
        let preset_issues = data()
            .elixir_presets
            .iter()
            .map(|preset| state.preset_issues(&inputs, preset))
            .collect();
        let characters = inputs
            .characters
            .iter()
            .map(|character| {
                Choice::new(
                    character.id,
                    format!(
                        "{} · {} / {} stamina",
                        character.name,
                        format::value(character.stamina),
                        format::value(character.max_stamina)
                    ),
                )
            })
            .collect();
        let records = records(&inputs, state.manic);
        let caves = inputs
            .caves
            .iter()
            .map(|cave| {
                let record = records.get(cave.id).copied().unwrap_or(0.0);
                Choice::new(
                    cave.id,
                    format!(
                        "{} · record {} · {} members",
                        cave.name,
                        format::value(record),
                        cave.capacity
                    ),
                )
            })
            .collect();
        let modes = vec![
            Choice::new(false, "Normal"),
            Choice::new(true, "Manic").disabled(!inputs.manic),
        ];
        let policies = Policy::ALL
            .into_iter()
            .map(|policy| {
                Choice::new(policy, policy.label()).disabled(policy == Policy::Auto && !inputs.auto)
            })
            .collect();
        let speeds = (0..=inputs.speed_max)
            .map(|speed| {
                Choice::new(
                    speed,
                    if speed == 0 {
                        "Base".to_string()
                    } else {
                        format!("Speed {speed}")
                    },
                )
            })
            .collect();
        let depth_tools = std::iter::once(Choice::new(
            ReferenceTool::Best,
            "Lowest isolated cost (optimistic)",
        ))
        .chain(
            inputs
                .tools
                .iter()
                .map(|tool| Choice::new(ReferenceTool::Tool(tool.id), tool.name.clone())),
        )
        .collect();
        let run_tools = std::iter::once(Choice::new(ToolChoice::Adaptive, "Adaptive"))
            .chain(
                inputs
                    .tools
                    .iter()
                    .map(|tool| Choice::new(ToolChoice::Tool(tool.id), tool.name.clone())),
            )
            .collect();
        let amber_mode = state.optimizer_mode == OptimizerMode::Amber;
        let objectives = Objective::ALL
            .into_iter()
            .filter(|objective| !amber_mode || objective.amber())
            .map(|objective| Choice::new(objective, objective.label()))
            .collect();
        let capacity = inputs.caves.get(state.cave).map_or(0, |cave| cave.capacity);
        let full_stamina = state.starting == StaminaMode::Full;
        let party = (0..capacity)
            .map(|slot| {
                std::iter::once(Choice::new(-1, "Empty"))
                    .chain(inputs.characters.iter().map(|character| {
                        let eligible = full_stamina || character.has_half_stamina();
                        Choice::new(
                            character.id as i32,
                            format!(
                                "{} · {} / {}",
                                character.name,
                                format::value(character.stamina),
                                format::value(character.max_stamina)
                            ),
                        )
                        .disabled(state.in_party(slot, character.id) || !eligible)
                    }))
                    .collect()
            })
            .collect();
        let elixirs = slot_choices(&inputs, PresetKind::Belt, &state.elixirs, inputs.slots);
        let memorized = slot_choices(
            &inputs,
            PresetKind::Memory,
            &state.memorized,
            inputs.memory_slots,
        );
        Self {
            config_error,
            space,
            depth,
            preset_issues,
            characters,
            caves,
            modes,
            policies,
            speeds,
            depth_tools,
            run_tools,
            objectives,
            party,
            elixirs,
            memorized,
            inputs,
        }
    }
}

/// Each belt or memory slot's elixirs; one already used up elsewhere (past the
/// duplicate limit on the belt, at all in memory) is shown but not pickable.
fn slot_choices(
    inputs: &Inputs,
    kind: PresetKind,
    values: &[i32],
    count: usize,
) -> Vec<Vec<Choice<i32>>> {
    (0..count)
        .map(|slot| {
            let others = || {
                values
                    .iter()
                    .enumerate()
                    .filter(move |(index, _)| *index != slot)
                    .map(|(_, value)| *value)
            };
            std::iter::once(Choice::new(-1, "Empty"))
                .chain(
                    inputs
                        .elixirs
                        .iter()
                        .take(inputs.elixir_owned)
                        .map(|elixir| {
                            let id = elixir.id as i32;
                            let used = match kind {
                                PresetKind::Belt => {
                                    others().filter(|value| *value == id).count() >= inputs.dupes
                                }
                                PresetKind::Memory => others().any(|value| value == id),
                            };
                            Choice::new(id, elixir.name.clone()).disabled(used)
                        }),
                )
                .collect()
        })
        .collect()
}

/// The depth profile behind the two charts and the inspector.
type DepthKey = (
    u64,
    idleon_spelunking::Requested,
    idleon_spelunking::DepthRange,
    idleon_spelunking::Account,
);

pub(crate) struct Depth {
    pub(crate) profile: Result<Profile, idleon_spelunking::ConfigError>,
    pub(crate) risk: Rc<Vec<Series>>,
    pub(crate) stamina: Rc<Vec<Series>>,
}

impl Depth {
    fn of(inputs: &Inputs, state: &State) -> Self {
        let profile = profile(inputs, &state.requested(), &state.range());
        let (risk, stamina) = match &profile {
            Ok(profile) => (risk_series(profile), stamina_series(profile)),
            Err(_) => (Vec::new(), Vec::new()),
        };
        Self {
            profile,
            risk: Rc::new(risk),
            stamina: Rc::new(stamina),
        }
    }
}

fn risk_series(profile: &Profile) -> Vec<Series> {
    let blocked = profile
        .rows
        .iter()
        .map(|row| Some(row.blocked_exits))
        .collect();
    let hard = profile.rows.iter().map(|row| Some(row.hard)).collect();
    vec![
        Series::new(
            "All descent holes over budget",
            format::hex(DEPTH_COLORS[1]),
            blocked,
        ),
        Series::new(
            "Normal rocks over budget",
            format::hex(DEPTH_COLORS[4]),
            hard,
        )
        .dashed(),
    ]
}

fn stamina_series(profile: &Profile) -> Vec<Series> {
    let mut kinds: Vec<usize> = profile
        .rows
        .iter()
        .flat_map(|row| row.objects.iter().map(|object| object.kind))
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
        .iter()
        .map(|kind| {
            let name = profile
                .rows
                .iter()
                .flat_map(|row| row.objects.iter())
                .find(|object| object.kind == *kind)
                .map_or_else(|| format!("Rock {kind}"), |object| object.name.clone());
            let values = profile
                .rows
                .iter()
                .map(|row| {
                    row.objects
                        .iter()
                        .find(|object| object.kind == *kind)
                        .map(|object| object.expected)
                        .filter(|expected| expected.is_finite())
                })
                .collect();
            Series::new(
                name,
                format::hex(DEPTH_COLORS[kind % DEPTH_COLORS.len()]),
                values,
            )
        })
        .collect()
}

pub(crate) fn starting_choices() -> Vec<Choice<StaminaMode>> {
    vec![
        Choice::new(StaminaMode::Saved, "Saved stamina"),
        Choice::new(StaminaMode::Full, "Fully recovered (hypothetical)"),
    ]
}

pub(crate) fn finisher_choices() -> Vec<Choice<u32>> {
    vec![
        Choice::new(1, "Last character"),
        Choice::new(2, "Last two characters"),
    ]
}

pub(crate) fn entry_choices() -> Vec<Choice<Entry>> {
    vec![
        Choice::new(Entry::Surface, "Surface"),
        Choice::new(Entry::Shortcut, "Saved shortcut"),
    ]
}

pub(crate) fn optimizer_mode_choices() -> Vec<Choice<OptimizerMode>> {
    vec![
        Choice::new(OptimizerMode::Configuration, "Configurations"),
        Choice::new(OptimizerMode::Amber, "Amber farming phases"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::searchable_list::SearchableListItem as _;
    use idleon_spelunking::Account;

    fn save() -> Save {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/data.json"
        ))
        .expect("the fixture is committed");
        let imported = idleon_import::import(&text).expect("the fixture imports");
        Save::from_parts(&imported.save).expect("the fixture carries Spelunk rows")
    }

    #[test]
    fn the_derived_figures_read_the_save_and_lock_taken_slots() {
        let save = save();
        let mut state = State::default();
        state.adopt(&save.inputs(&Account::neutral()));
        let inputs = save.inputs(&state.account);
        let depth = Rc::new(Depth::of(&inputs, &state));
        assert_eq!(
            depth.profile,
            profile(&inputs, &state.requested(), &state.range()),
            "the cached profile is the calculator's"
        );
        let derived = Derived::of(inputs, &state, depth);
        assert_eq!(derived.config_error, None);
        assert_eq!(derived.party.len(), state.party.len());
        assert_eq!(derived.elixirs.len(), derived.inputs.slots);
        assert_eq!(derived.memorized.len(), derived.inputs.memory_slots);

        // The first slot's character cannot be picked again in the second.
        let first = state.party[0];
        let taken = derived.party[1]
            .iter()
            .find(|choice| *choice.value() == first)
            .expect("every character is listed");
        assert!(taken.disabled());
        let own = derived.party[0]
            .iter()
            .find(|choice| *choice.value() == first)
            .expect("every character is listed");
        assert!(!own.disabled());
        // A memorized elixir cannot fill the other memory slot.
        let memorized = state.memorized[0];
        assert!(
            derived.memorized[1]
                .iter()
                .find(|choice| *choice.value() == memorized)
                .is_some_and(|choice| choice.disabled())
        );
    }
}
