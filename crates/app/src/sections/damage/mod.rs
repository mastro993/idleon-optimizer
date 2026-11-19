//! The Damage Breakdown section: the two context selectors, the summary strip,
//! the damage layers and the mastery breakdown.
//!
//! The page holds the character and map selects, the summary strip (the
//! "Character Sheet Range", the minimum and maximum with their exact values,
//! and the mastery ratio with its bar), the `Damage Layers` block whose three cards carry a
//! value, the descriptor's note and its pre-softcap value, and the `Mastery
//! Breakdown` tree. The three layers come from the shared stat pipeline, which
//! this build does not decode, so the layer cards show a dash and the page's
//! reason instead of a number it cannot justify. The context, the selector
//! rows, the layers, the mastery terms and their formatting live in
//! [`idleon_damage`] and are ready for that provider.

mod actions;

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, Hsla, IntoElement, ParentElement, Role, Styled, Subscription,
    Task, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Toggle, ToggleGroup, ToggleVariants as _};
use gpui_kit::component::empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, v_flex};
use idleon_common::ValueFormat;
use idleon_common::formula::FormulaNode;
use idleon_damage::{self as damage, Context as DamageContext, LayerId, MapRow, Summary};
use idleon_research::model::Simulation;
use idleon_save::Save;
use idleon_stats::{self as stats, Snapshot};
use idleon_ui::format;
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, Choice, ChoiceSelect, KpiStrip, PANEL_GAP, Statistic,
    TEXT_CAPTION, TEXT_KPI, Tone,
};
use std::sync::Arc;

/// The section's own state: the context, the save slice it reads and the layer
/// tab the page has open.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) context: DamageContext,
    pub(crate) snapshot: Option<Snapshot>,
    /// The summary for `context`; `None` before a save loads and while it
    /// resolves.
    pub(crate) summary: Option<Summary>,
    /// The damage layer the tabs show.
    pub(crate) layer: LayerId,
    /// The context moved under `summary`. Resolving is the expensive half of
    /// an import, so it waits for the page and then runs off the UI thread
    /// ([`AppState::resolve_damage`]).
    dirty: bool,
    /// The resolve in flight; replacing it drops a stale one.
    pending: Option<Task<()>>,
    save: Option<Save>,
    /// The imported research grid, which the stat pipeline reads.
    simulation: Arc<Simulation>,
    /// The map selector's rows for the selected character: every named map,
    /// not the accessible subset.
    map_rows: Vec<MapRow>,
    characters: Vec<Choice<usize>>,
}

/// The kit selects that show the context, built once (`docs/ui-kit/forms.md`).
pub(crate) struct Controls {
    character_select: ChoiceSelect<usize>,
    map_select: ChoiceSelect<usize>,
    _subscriptions: [Subscription; 2],
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let character_select = ChoiceSelect::new(Vec::new(), None, window, cx);
        let map_select = ChoiceSelect::new(Vec::new(), None, window, cx);
        let subscriptions = [
            cx.subscribe(
                character_select.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    if let SelectEvent::Confirm(Some(character)) = event {
                        state.set_damage_character(*character, cx);
                    }
                },
            ),
            cx.subscribe(
                map_select.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    if let SelectEvent::Confirm(Some(map)) = event {
                        state.set_damage_map(*map, cx);
                    }
                },
            ),
        ];
        Self {
            character_select,
            map_select,
            _subscriptions: subscriptions,
        }
    }

    /// Bring both selects in line with the context.
    fn show(&self, damage: &State, window: &mut Window, cx: &mut App) {
        self.character_select.show(
            damage.characters.clone(),
            Some(&damage.context.character),
            window,
            cx,
        );
        let maps = damage
            .map_rows
            .iter()
            .map(|row| Choice::new(row.map, row.label.clone()))
            .collect();
        self.map_select
            .show(maps, Some(&damage.context.map), window, cx);
    }
}

impl State {
    /// The mastery breakdown's root, or nothing while the layers are unresolved.
    pub(crate) fn mastery_tree(&self) -> Option<FormulaNode> {
        self.summary.as_ref().and_then(Summary::mastery_tree)
    }

    /// The selected layer's formula tree, or nothing while it has no terms.
    pub(crate) fn layer_tree(&self) -> Option<FormulaNode> {
        self.summary
            .as_ref()
            .and_then(|summary| summary.layer_tree(self.layer))
    }

    /// Open one of the three layer tabs.
    pub(crate) fn select_layer(&mut self, layer: LayerId) {
        self.layer = layer;
    }

    /// Take a freshly imported save, opening on character one and its current
    /// map.
    pub(crate) fn load(&mut self, save: &Save, simulation: &Simulation) {
        let snapshot = Snapshot::from_save(save);
        self.context = DamageContext::from_snapshot(&snapshot);
        self.characters = character_choices(&snapshot);
        self.snapshot = Some(snapshot);
        self.save = Some(save.clone());
        self.simulation = Arc::new(simulation.clone());
        self.clamp(&mut Vec::new());
        self.invalidate();
    }

    /// Keep the context inside the save and the map list; names a dropped map
    /// in `warnings`, for a project file restoring an unavailable one.
    pub(crate) fn clamp(&mut self, warnings: &mut Vec<String>) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if self.context.character >= snapshot.characters() {
            warnings.push(format!(
                "damage: character {} is outside the {} the save carries; character 1 is used",
                self.context.character + 1,
                snapshot.characters()
            ));
            self.context.character = 0;
        }
        if let Some(dropped) = self.context.clamp_map(snapshot) {
            warnings.push(format!(
                "damage: map {dropped} is not in the map selector; map {} is used",
                self.context.map
            ));
        }
        self.map_rows = self.context.map_rows(snapshot);
    }

    /// The context moved: the summary describes another one, so it goes, and
    /// the page resolves the new one.
    fn invalidate(&mut self) {
        self.summary = None;
        self.pending = None;
        self.dirty = true;
    }

    /// The evaluation the context needs, ready to run off the UI thread, or
    /// `None` when nothing moved or no save is loaded.
    fn job(&mut self) -> Option<impl FnOnce() -> Summary + Send + 'static> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        let save = self.save.clone()?;
        let simulation = self.simulation.clone();
        let context = self.context;
        Some(move || damage::evaluate(&save, &simulation, context))
    }

    /// Pick a character; the map follows its current map.
    pub(crate) fn select_character(&mut self, character: usize) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if character >= snapshot.characters() {
            return;
        }
        self.context.select_character(character, snapshot);
        self.map_rows = self.context.map_rows(snapshot);
        self.invalidate();
    }

    /// Pick a map from the list the save allows.
    pub(crate) fn select_map(&mut self, map: usize) {
        if !self.map_rows.iter().any(|row| row.map == map) {
            return;
        }
        self.context.map = map;
        self.invalidate();
    }
}

/// The character selector's rows: every character with its class.
fn character_choices(snapshot: &Snapshot) -> Vec<Choice<usize>> {
    (0..snapshot.characters())
        .map(|character| {
            let class = stats::class_label(snapshot.class_id(character));
            Choice::new(character, format!("{} ({class})", snapshot.name(character)))
        })
        .collect()
}

/// The section page: the context card, the summary cards, the damage layers and
/// the mastery breakdown.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    state.resolve_damage(cx);
    let entity = cx.entity();
    let muted = cx.theme().muted_foreground;
    let damage = &state.damage;
    let controls = &state.damage_controls;
    controls.show(damage, window, cx);
    let context = damage.context;
    let snapshot = damage.snapshot.as_ref();
    let summary = damage.summary.as_ref();
    let note = match (snapshot, summary) {
        (None, _) => "Load a save to calculate damage.".to_string(),
        (Some(snapshot), Some(summary)) if !summary.reason.is_empty() => format!(
            "{} · {} · {}",
            snapshot.name(context.character),
            stats::classify(context.map).name,
            summary.reason
        ),
        (Some(snapshot), _) => format!(
            "{} · {}",
            snapshot.name(context.character),
            stats::classify(context.map).name
        ),
    };
    let has_save = snapshot.is_some();

    let controls = div()
        .id("damage-context-controls")
        .flex()
        .items_end()
        .gap_3()
        .child(field("Character", muted).child(select(
            &controls.character_select,
            "damage-character",
            "Character",
            has_save,
        )))
        .child(field("Map", muted).child(select(
            &controls.map_select,
            "damage-map",
            "Map",
            has_save,
        )));

    let body = match (snapshot, summary) {
        (_, Some(summary)) => div()
            .flex()
            .flex_col()
            .gap(PANEL_GAP)
            .child(summary_card(summary, muted))
            .child(layers_card(summary, damage.layer, muted, entity, cx))
            .child(formula_breakdown(
                summary,
                damage.layer,
                state.damage_formula.clone(),
                muted,
            ))
            .child(mastery_breakdown(
                summary,
                state.damage_mastery.clone(),
                muted,
            ))
            .into_any_element(),
        (Some(_), None) => resolving(),
        (None, None) => empty_summary(muted),
    };

    div()
        .id("damage-page")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(PANEL_GAP)
                        .child(
                            Card::new()
                                .header(CardHeader::new("Calculation Context").description(
                                    "The damage summary resolves against this character and map.",
                                ))
                                .child(controls)
                                .child(
                                    div()
                                        .id("damage-context-note")
                                        .role(Role::Label)
                                        .aria_label(note.clone())
                                        .text_size(TEXT_CAPTION)
                                        .text_color(muted)
                                        .child(note),
                                ),
                        )
                        .child(body),
                )
                .overflow_y_scrollbar()
                .id("damage-scroll"),
        )
        .into_any_element()
}

/// One context selector under its harness id, disabled until a save loads.
fn select(
    select: &ChoiceSelect<usize>,
    id: &'static str,
    label: &'static str,
    has_save: bool,
) -> impl IntoElement {
    div().flex_none().w(px(260.0)).child(
        Select::new(select.state())
            .id(id)
            .accessibility_label(label)
            .placeholder("Load a save first")
            .disabled(!has_save)
            .small(),
    )
}

/// What the page shows while the summary resolves off the UI thread.
fn resolving() -> AnyElement {
    div()
        .id("damage-resolving")
        .role(Role::Label)
        .aria_label("Resolving the damage summary")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(
            div()
                .flex()
                .gap(PANEL_GAP)
                .children((0..4).map(|_| Skeleton::new().flex_1().h(px(96.0)))),
        )
        .child(Skeleton::new().h(px(160.0)))
        .child(Skeleton::new().h(px(320.0)))
        .into_any_element()
}

/// The `Damage Layers` block: one tab per layer, with the descriptor's own title,
/// value, note and pre-softcap value. A layer the shared stat pipeline cannot
/// resolve shows a dash and the page's reason instead of a made-up zero.
fn layers_card(
    summary: &Summary,
    selected: LayerId,
    muted: Hsla,
    entity: Entity<AppState>,
    cx: &App,
) -> AnyElement {
    let tabs = ToggleGroup::new("damage-layer-tabs")
        .segmented()
        .single()
        .outline()
        .small()
        .children(LayerId::ALL.map(|id| {
            Toggle::new(gpui::SharedString::from(format!(
                "damage-layer-{}",
                id.id()
            )))
            .label(id.id().to_uppercase())
            .checked(id == selected)
        }))
        .on_click(move |checked, _window, cx| {
            // A click on the open tab clears every flag: keep it open.
            if let Some(ix) = checked.iter().position(|checked| *checked) {
                let layer = LayerId::ALL[ix];
                entity.update(cx, |state, cx| state.set_damage_layer(layer, cx));
            }
        });
    let layer = summary.layers().map(|layers| layers.get(selected));
    let value = match layer {
        Some(layer) => match selected.format() {
            ValueFormat::Raw => format::value(layer.value),
            _ => format!("×{}", format::value(layer.value)),
        },
        None => "—".to_string(),
    };
    let note = match layer {
        Some(layer) if selected.note().is_empty() => {
            format!("before softcaps: {}", format::value(layer.pre_softcap))
        }
        Some(layer) => format!(
            "{} · before softcaps: {}",
            selected.note(),
            format::value(layer.pre_softcap)
        ),
        None => summary.reason.clone(),
    };
    Card::new()
        .header(
            CardHeader::new("Damage Layers")
                .description("The three layers the maximum damage multiplies"),
        )
        .child(div().id("damage-layer-controls").flex().child(tabs))
        .child(
            div()
                .id("damage-layer")
                .role(Role::Label)
                .aria_label(format!("{}: {value}", selected.title()))
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new(selected.title()).text_color(muted))
                .child(
                    div()
                        .id("damage-layer-value")
                        .role(Role::Label)
                        .aria_label(value.clone())
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(TEXT_KPI)
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(value),
                )
                .child(
                    div()
                        .id("damage-layer-note")
                        .role(Role::Label)
                        .aria_label(note.clone())
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(note),
                ),
        )
        .into_any_element()
}

/// The `Formula Breakdown` card: the selected layer's expanded tree, with each
/// term's input, multiplier and source; the shared tree's search
/// highlights the matching nodes.
fn formula_breakdown(
    summary: &Summary,
    layer: LayerId,
    tree: Entity<FormulaTree>,
    muted: Hsla,
) -> AnyElement {
    let card = Card::new().header(
        CardHeader::new("Formula Breakdown")
            .description("The selected layer, term by term, with each term's source"),
    );
    match summary.layer_tree(layer) {
        Some(_) => card
            .child(
                div()
                    .id("damage-formula-breakdown")
                    .h(px(320.0))
                    .min_h_0()
                    .child(tree),
            )
            .into_any_element(),
        None => card
            .child(
                div()
                    .id("damage-formula-gap")
                    .role(Role::Label)
                    .aria_label(summary.reason.clone())
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(summary.reason.clone()),
            )
            .into_any_element(),
    }
}

/// The `Mastery Breakdown` card: the shared formula tree over the mastery terms,
/// or the reason the ratio has none.
fn mastery_breakdown(summary: &Summary, tree: Entity<FormulaTree>, muted: Hsla) -> AnyElement {
    let card = Card::new().header(
        CardHeader::new("Mastery Breakdown").description("The minimum/maximum ratio, term by term"),
    );
    match summary.layers() {
        Some(_) => card
            .child(
                div()
                    .id("damage-mastery-breakdown")
                    .h(px(240.0))
                    .min_h_0()
                    .child(tree),
            )
            .into_any_element(),
        None => card
            .child(
                div()
                    .id("damage-mastery-gap")
                    .role(Role::Label)
                    .aria_label(summary.reason.clone())
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(summary.reason.clone()),
            )
            .into_any_element(),
    }
}

/// The page before a save loads.
fn empty_summary(muted: Hsla) -> AnyElement {
    div()
        .id("damage-summary-empty")
        .child(
            Empty::new().header(
                EmptyHeader::new()
                    .media(EmptyMedia::new().child(Icon::new(IconName::SearchX).text_color(muted)))
                    .title(EmptyTitle::new().child("No damage summary yet"))
                    .description(EmptyDescription::new().child(
                        "Import a save to open the character and map the summary resolves against.",
                    )),
            ),
        )
        .into_any_element()
}

/// The four summary figures in one strip. While the layers cannot be resolved
/// they show a dash and the reason, never a made-up zero.
fn summary_card(summary: &Summary, muted: Hsla) -> AnyElement {
    let game = summary
        .game_range()
        .map(|(min, max)| format::damage_range(min, max))
        .unwrap_or_else(|| "—".into());
    let min = if summary.available {
        format::value(summary.min)
    } else {
        "—".into()
    };
    let max = if summary.available {
        format::value(summary.max)
    } else {
        "—".into()
    };
    let mastery = if summary.available {
        format!("{:.1}%", summary.mastery * 100.0)
    } else {
        "—".into()
    };
    let exact_min = if summary.available {
        format!("Exact: {}", format::exact(summary.min))
    } else {
        "No exact value until the layers resolve".into()
    };
    let exact_max = if summary.available {
        format!("Exact: {}", format::exact(summary.max))
    } else {
        "No exact value until the layers resolve".into()
    };
    let game_sub = if !summary.available {
        "Unavailable"
    } else if summary.partial() {
        "Partial result"
    } else {
        "As shown on the character sheet"
    };
    let mastery_sub = if summary.available {
        "Minimum / maximum ratio; capped at 80%"
    } else {
        "The mastery ratio needs the shared stat pipeline"
    };
    let mastery_fill = summary.mastery_fill() as f32;

    let mut body = div().flex().flex_col().gap(PANEL_GAP).child(
        div().id("damage-summary").child(
            KpiStrip::new()
                .child(
                    Statistic::new("damage-game", "Character Sheet Range", game).caption(game_sub),
                )
                .child(Statistic::new("damage-min", "Minimum Damage", min).caption(exact_min))
                .child(Statistic::new("damage-max", "Maximum Damage", max).caption(exact_max))
                .child(mastery_cell(mastery, mastery_sub, mastery_fill)),
        ),
    );
    if !summary.available {
        body = body.child(
            Callout::new(Tone::Warning, "Damage summary unavailable")
                .child(Caption::new(summary.reason.clone()).text_color(muted)),
        );
    }
    // The import's own gap, reported beside the value: the
    // `it.json` companion roster and the server's active vote.
    if summary.partial() {
        let reason = summary.partial_reason();
        body = body.child(
            div()
                .id("damage-partial")
                .role(Role::Label)
                .aria_label(reason.clone())
                .child(
                    Callout::new(Tone::Warning, "Partial result")
                        .child(Caption::new(reason).text_color(muted)),
                ),
        );
    }
    body.into_any_element()
}

/// The mastery cell: the ratio, its note and the bar, filled to `percent`.
fn mastery_cell(value: String, note: &'static str, percent: f32) -> AnyElement {
    let label = format!("Mastery: {value}");
    let bar = format!("Mastery bar: {value}");
    v_flex()
        .id("damage-mastery-card")
        .role(Role::Label)
        .aria_label(label)
        .gap_2()
        .child(Statistic::new("damage-mastery", "Mastery", value).caption(note))
        // The shared bar draws no label of its own, so the cell names the
        // ratio it fills to.
        .child(
            div()
                .id("damage-mastery-bar")
                .role(Role::ProgressIndicator)
                .aria_label(bar)
                .child(Progress::new("damage-mastery-bar-fill").value(percent)),
        )
        .into_any_element()
}

fn field(label: &'static str, muted: Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}

/// Restore a project file's damage context, warning about values the save
/// cannot show.
pub(crate) fn restore(state: &mut AppState, context: DamageContext, warnings: &mut Vec<String>) {
    state.damage.context = context;
    state.damage.clamp(warnings);
    // The restored character and map re-resolve on the next render.
    state.damage.invalidate();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Resolve what the context left pending, as the page does off the UI thread.
    fn resolve(state: &mut State) {
        if let Some(job) = state.job() {
            state.summary = Some(job());
        }
    }

    fn save() -> Save {
        Save::new(json!({
            "charNames": ["Ana", "Bo"],
            "Lv0_0": "[1]",
            "Lv0_1": "[1]",
            "CharacterClass_0": 1,
            "CharacterClass_1": 7,
            "CurrentMap_0": 0,
            "CurrentMap_1": 50,
            "KLA_0": "[[0]]",
            "KLA_1": "[[0]]",
        }))
    }

    #[test]
    fn an_import_opens_on_character_one_and_its_current_map() {
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        assert_eq!(state.context.character, 0);
        assert_eq!(state.context.map, 0);
        assert!(state.snapshot.is_some());
        assert_eq!(state.map_rows.first().map(|row| row.map), Some(0));
        let summary = state.summary.expect("the summary is computed");
        // The shared stat engine resolves the three layers, so the page opens
        // on real values instead of the missing-pipeline state.
        assert!(summary.available, "{summary:?}");
        assert!(summary.layers().is_some(), "{summary:?}");
        assert!(summary.max.is_finite() && summary.max >= 0.0);
        assert!(summary.min <= summary.max);
        assert!((0.0..=0.8).contains(&summary.mastery), "{summary:?}");
        // A flat save carries neither the companion roster nor the vote row, so
        // the page names the partial import as the descriptor does.
        assert!(summary.partial());
        assert!(summary.partial_reason().contains("companion ownership"));
    }

    #[test]
    fn the_map_row_labels_carry_the_dungeon_and_current_markers() {
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        let rows = &state.map_rows;
        let current: Vec<&MapRow> = rows.iter().filter(|row| row.current).collect();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].map, 0);
        assert!(current[0].label.ends_with(" (current)"));
        let dungeon = rows
            .iter()
            .find(|row| row.dungeon)
            .expect("the selector lists dungeons");
        assert!(dungeon.label.ends_with(" (dungeon)"), "{}", dungeon.label);
    }

    #[test]
    fn a_character_switch_follows_its_current_map() {
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        state.select_character(1);
        assert_eq!(state.context.map, 50);
    }

    #[test]
    fn a_dungeon_map_reports_the_dungeon_reason() {
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        state.select_map(39);
        assert!(
            state.summary.is_none(),
            "a context change drops the old summary"
        );
        resolve(&mut state);
        let summary = state.summary.expect("the dungeon state is computed");
        assert!(!summary.available);
        assert!(summary.reason.contains("Dungeon"), "{summary:?}");
    }

    #[test]
    fn the_layer_tabs_open_on_the_first_layer_and_switch() {
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        assert_eq!(state.layer, LayerId::Base);
        state.select_layer(LayerId::Multiplier);
        assert_eq!(state.layer, LayerId::Multiplier);
        assert_eq!(LayerId::from_id("ddl2"), Some(state.layer));
    }

    #[test]
    fn the_formula_tree_follows_the_selected_layer() {
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        let expected = |state: &State, layer| {
            state
                .summary
                .as_ref()
                .expect("the summary resolves")
                .layers()
                .expect("the layers resolve")
                .get(layer)
                .value
        };
        let base = state.layer_tree().expect("the base tree resolves");
        assert_eq!(base.value, expected(&state, LayerId::Base));
        assert!(base.node_count() > 8);
        state.select_layer(LayerId::Multiplier);
        let multiplier = state.layer_tree().expect("the multiplier tree resolves");
        assert_eq!(multiplier.value, expected(&state, LayerId::Multiplier));
        assert_ne!(base.value, multiplier.value);
    }

    #[test]
    fn the_layer_tabs_and_the_mastery_breakdown_read_the_summary() {
        // The engine resolves the layers, so the tabs and the mastery breakdown
        // read the summary's own values from the imported save.
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        assert!(state.mastery_tree().is_some());
        assert!(state.summary.as_ref().unwrap().layers().is_some());

        // The same page once the layers resolve: all three tabs, and the mastery
        // tree's root is the ratio the mastery card prints.
        let summary = Summary::from_layers(damage::Layers {
            base: damage::Layer {
                value: 100.0,
                pre_softcap: 500.0,
            },
            percent: damage::Layer {
                value: 2.0,
                pre_softcap: 8.0,
            },
            multiplier: damage::Layer {
                value: 3.0,
                pre_softcap: 12.0,
            },
            mastery: damage::Mastery {
                bubble: 0.2,
                ..damage::Mastery::default()
            },
        });
        let layers = summary.layers().expect("the layers are kept");
        assert_eq!(summary.max, 600.0);
        assert_eq!(layers.get(LayerId::Base).value, 100.0);
        assert_eq!(layers.get(LayerId::Percent).pre_softcap, 8.0);
        assert_eq!(layers.get(LayerId::Multiplier).value, 3.0);
        let tree = summary.mastery_tree().expect("the mastery tree resolves");
        assert_eq!(tree.value, summary.mastery);
        assert!(
            (summary.mastery - 0.55).abs() < 1e-12,
            "{}",
            summary.mastery
        );
        state.summary = Some(summary);
        assert_eq!(state.mastery_tree().map(|tree| tree.value), Some(0.55));
    }

    #[test]
    fn a_restored_context_outside_the_save_is_clamped_and_named() {
        let mut state = State::default();
        state.load(&save(), &Default::default());
        resolve(&mut state);
        let mut warnings = Vec::new();
        state.context = DamageContext {
            character: 12,
            map: 5_000,
        };
        state.clamp(&mut warnings);
        assert_eq!(state.context.character, 0);
        assert_eq!(state.context.map, 0);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("character 13"), "{warnings:?}");
        assert!(warnings[1].contains("map 5000"), "{warnings:?}");
    }
}
