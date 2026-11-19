//! What the section draws from but never saves: the kit controls and the
//! engine results its tabs read.
//!
//! The persisted values stay in [`super::State`]. The controls are built once
//! here and edit it only through `AppState`'s setters (`docs/ui-kit/forms.md`).
//! [`AppState::sync_monument`] recomputes a result only when what it reads
//! moved, so a frame formats cached snapshots instead of calling the engine.

use super::clean_text;
use super::justice::{Effect, Field};
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_monument::justice::{self, Delayed, Report};
use idleon_monument::{self as monument, JusticeSnapshot, Odds, Snapshot, Summary, WisdomSnapshot};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};

/// The save revision and Bravery run state (enemy, boosts, rethrows, HP
/// bits) the odds were computed for.
type OddsKey = (u64, u32, u32, Option<u32>, Option<u64>);

/// The section's live controls and render caches.
pub(crate) struct Controls {
    pub(crate) character: ChoiceSelect<usize>,
    /// The Justice case the player received.
    pub(crate) case: ChoiceSelect<usize>,
    /// The Bravery run state: enemy number, boost stacks, rethrows and HP.
    pub(crate) wave: Entity<InputState>,
    pub(crate) boosts: Entity<InputState>,
    pub(crate) rerolls: Entity<InputState>,
    pub(crate) hp: Entity<InputState>,
    /// One Justice worksheet field per [`Field::ALL`] entry.
    pub(crate) fields: [Entity<InputState>; Field::ALL.len()],
    /// One value field per [`Effect::ALL`] entry, drawn while it is active.
    pub(crate) effects: [Entity<InputState>; Effect::ALL.len()],
    /// The engine results the tabs read.
    pub(crate) derived: Derived,
    _subscriptions: Vec<Subscription>,
}

/// The engine results, each with the inputs it was computed from.
#[derive(Default)]
pub(crate) struct Derived {
    /// The save revision and character the snapshots read.
    save_key: Option<(u64, usize)>,
    pub(crate) snapshot: Option<Snapshot>,
    pub(crate) summary: Option<Summary>,
    pub(crate) wisdom: Option<WisdomSnapshot>,
    pub(crate) justice: Option<JusticeSnapshot>,
    pub(crate) characters: Vec<Choice<usize>>,
    odds_key: Option<OddsKey>,
    pub(crate) odds: Option<Result<Odds, monument::Error>>,
    advisor_key: Option<(String, justice::State, usize)>,
    /// The case list the search keeps, and how many cases it names.
    pub(crate) cases: Vec<Choice<usize>>,
    pub(crate) case_count: String,
    pub(crate) report: Option<Result<Report, justice::Error>>,
    pub(crate) delayed: Vec<Delayed>,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let character = ChoiceSelect::new(Vec::new(), None, window, cx);
        let case = ChoiceSelect::new(Vec::new(), None, window, cx);
        let wave = number_state(1.0, 1001.0, 1.0, window, cx);
        let boosts = number_state(0.0, 100_000.0, 1.0, window, cx);
        let rerolls = number_state(0.0, 1.0e9, 1.0, window, cx);
        let hp = cx.new(|cx| {
            InputState::new(window, cx)
                .min(0.000001)
                .max(1.0e300)
                .step(1.0)
                .placeholder("Unknown")
        });
        let fields = std::array::from_fn(|index| {
            let min = match Field::ALL[index] {
                Field::Popularity => -1.0e9,
                Field::Round => 1.0,
                _ => 0.0,
            };
            number_state(min, 1.0e9, 1.0, window, cx)
        });
        let effects = std::array::from_fn(|_| number_state(0.0, 1.0e9, 1.0, window, cx));

        // Edits flow control -> section value, through the section's setters.
        let mut subscriptions = vec![
            pick(cx, &character, AppState::set_monument_character),
            pick(cx, &case, AppState::select_monument_justice_case),
            live(cx, &wave, AppState::set_monument_wave),
            live(cx, &boosts, AppState::set_monument_boosts),
            live(cx, &rerolls, AppState::set_monument_rerolls),
            live(cx, &hp, AppState::set_monument_hp),
        ];
        // Each worksheet edit is one undoable step, so a typed value lands on
        // Enter or when the field loses focus, never per keystroke.
        for (field, input) in Field::ALL.into_iter().zip(&fields) {
            subscriptions.push(commit(cx, input, move |state, value, cx| {
                state.set_monument_justice_field(field, value, cx);
            }));
        }
        for (effect, input) in Effect::ALL.into_iter().zip(&effects) {
            subscriptions.push(commit(cx, input, move |state, value, cx| {
                state.set_monument_justice_effect(effect, value, cx);
            }));
        }
        Self {
            character,
            case,
            wave,
            boosts,
            rerolls,
            hp,
            fields,
            effects,
            derived: Derived::default(),
            _subscriptions: subscriptions,
        }
    }
}

/// A select whose pick calls `set`.
fn pick(
    cx: &mut Context<AppState>,
    select: &ChoiceSelect<usize>,
    set: impl Fn(&mut AppState, usize, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
            let SelectEvent::Confirm(value) = event;
            if let Some(value) = *value {
                set(state, value, cx);
            }
        },
    )
}

/// A number field whose every edit calls `set`.
fn live(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    set: impl Fn(&mut AppState, f64, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            set(state, value, cx);
        }
    })
}

/// A number field that calls `set` on Enter and on blur.
fn commit(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    set: impl Fn(&mut AppState, f64, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event, cx| {
        if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur)
            && let Some(value) = number_value(field.read(cx))
        {
            set(state, value, cx);
        }
    })
}

impl AppState {
    /// Bring the cached results in line with the save, the character, the
    /// run state, the worksheet and the case search.
    pub(crate) fn sync_monument(&mut self, cx: &mut Context<Self>) {
        let query = self.monument_search.read(cx).value().trim().to_lowercase();
        let derived = &mut self.monument_controls.derived;
        let save_key = (self.save_revision, self.monument.character);
        if derived.save_key != Some(save_key) {
            derived.save_key = Some(save_key);
            let character = self.monument.character;
            let save = self.monument_save.as_ref();
            derived.snapshot = save.map(|save| monument::snapshot(save, character));
            derived.summary = save.map(|save| monument::summary(save, character));
            derived.wisdom = save.map(|save| monument::wisdom_snapshot(save, character));
            derived.justice = save.map(|save| monument::justice_snapshot(save, character));
            derived.characters = self
                .save
                .as_ref()
                .map(|save| {
                    (0..save.character_count())
                        .map(|index| Choice::new(index, save.character_name(index)))
                        .collect()
                })
                .unwrap_or_default();
            derived.odds_key = None;
        }

        let monument = &self.monument;
        let odds_key = (
            self.save_revision,
            monument.wave,
            monument.boosts,
            monument.rerolls,
            monument.remaining_hp.map(f64::to_bits),
        );
        if derived.odds_key != Some(odds_key) {
            derived.odds_key = Some(odds_key);
            derived.odds = self.monument_save.as_ref().map(|save| {
                monument::bravery_odds(
                    save,
                    monument.wave.saturating_sub(1),
                    monument::Runtime {
                        rerolls: monument.rerolls,
                        boosts: monument.boosts,
                    },
                    monument.remaining_hp,
                )
            });
        }

        let worksheet = &monument.worksheet;
        if derived
            .advisor_key
            .as_ref()
            .is_none_or(|(cached, state, case)| {
                *cached != query || *state != worksheet.state || *case != worksheet.case_id
            })
        {
            let (cases, case_count) = case_choices(&query, &worksheet.state, worksheet.case_id);
            derived.cases = cases;
            derived.case_count = case_count;
            derived.report = Some(justice::responses(&worksheet.state, worksheet.case_id));
            derived.delayed = justice::delayed_effects(&worksheet.state);
            derived.advisor_key = Some((query, worksheet.state.clone(), worksheet.case_id));
        }
    }
}

/// The cases the search keeps, and the count line under it. The search
/// matches the visitor and the request wording, keeps the selected case in the
/// list, and de-duplicates rows with the same wording.
fn case_choices(
    query: &str,
    state: &justice::State,
    case_id: usize,
) -> (Vec<Choice<usize>>, String) {
    let mut visible: Vec<usize> = Vec::new();
    for case in justice::cases() {
        let haystack = format!("{} {}", case.name, case.text).to_lowercase();
        if query.is_empty() || haystack.contains(query) {
            let duplicate = visible.iter().any(|id| {
                justice::cases()
                    .get(*id)
                    .is_some_and(|kept| kept.visitor == case.visitor && kept.text == case.text)
            });
            if !duplicate {
                visible.push(case.id);
            }
        }
    }
    if !visible.contains(&case_id) {
        visible.insert(0, case_id);
    }
    let choices = visible
        .iter()
        .map(|id| {
            let row =
                justice::case_text(state, *id).unwrap_or_else(|_| justice::cases()[*id].clone());
            Choice::new(*id, format!("{} - {}", row.name, clean_text(&row.text)))
        })
        .collect();
    let count = if query.is_empty() {
        format!("{} cases", justice::cases().len())
    } else {
        format!("{} matching cases", visible.len())
    };
    (choices, count)
}
