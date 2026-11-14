//! The Damage Breakdown section: the two upstream context selectors, the four
//! summary cards, the damage layers and the mastery breakdown.
//!
//! Ported from `damage-calc.html`: the character and map selects, the
//! "Character Sheet Range" card, the minimum and maximum cards with their exact
//! values, the mastery card with its bar, the `Damage Layers` block whose three
//! cards carry a value, the descriptor's note and its pre-softcap value, and the
//! `Mastery Breakdown` tree. The three layers come from the upstream shared stat
//! pipeline, which this build does not decode, so the layer cards show a dash
//! and the page's reason instead of a number it cannot justify. The context, the
//! selector rows, the layers, the mastery terms and the upstream formatting live
//! in [`idleon_damage`] and are ready for that provider.

mod actions;

use crate::state::AppState;
use crate::ui::layout::figure;
use ely_gpui_component::buttons::SegmentedControl;
use ely_gpui_component::feedback::{Callout, EmptyState};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::motion::ProgressBar;
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, Hsla, IntoElement, ParentElement, Role, Styled, Window, div, px,
};
use idleon_common::ValueFormat;
use idleon_common::formula::FormulaNode;
use idleon_damage::{self as damage, Context as DamageContext, LayerId, MapRow, Summary};
use idleon_save::Save;
use idleon_stats::{self as stats, Snapshot};
use idleon_ui::format;
use idleon_ui::formula::FormulaTree;

/// The section's own state: the context, the save slice it reads and the layer
/// tab the page has open.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) context: DamageContext,
    pub(crate) snapshot: Option<Snapshot>,
    /// The last computed summary; `None` before a save loads.
    pub(crate) summary: Option<Summary>,
    /// The damage layer the tabs show.
    pub(crate) layer: LayerId,
    save: Option<Save>,
}

impl State {
    /// The mastery breakdown's root, or nothing while the layers are unresolved.
    pub(crate) fn mastery_tree(&self) -> Option<FormulaNode> {
        self.summary.as_ref().and_then(Summary::mastery_tree)
    }

    /// Open one of the three layer tabs.
    pub(crate) fn select_layer(&mut self, layer: LayerId) {
        self.layer = layer;
    }

    /// Take a freshly imported save, opening on character one and its current
    /// map exactly like upstream `onLoad`.
    pub(crate) fn load(&mut self, save: &Save) {
        let snapshot = Snapshot::from_save(save);
        self.context = DamageContext::from_snapshot(&snapshot);
        self.snapshot = Some(snapshot);
        self.save = Some(save.clone());
        self.clamp(&mut Vec::new());
        self.refresh();
    }

    /// The map selector's rows for the current character, as upstream
    /// `populateMapSel` builds them: every named map, not the accessible subset.
    pub(crate) fn map_rows(&self) -> Vec<MapRow> {
        self.snapshot
            .as_ref()
            .map(|snapshot| self.context.map_rows(snapshot))
            .unwrap_or_default()
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
    }

    /// Recompute the summary for the current context.
    pub(crate) fn refresh(&mut self) {
        self.summary = self
            .save
            .as_ref()
            .map(|save| damage::evaluate(save, self.context));
    }

    /// Pick a character; the map follows its current map, as upstream `switchChar`.
    pub(crate) fn select_character(&mut self, character: usize) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if character >= snapshot.characters() {
            return;
        }
        self.context.select_character(character, snapshot);
        self.refresh();
    }

    /// Pick a map from the list the save allows.
    pub(crate) fn select_map(&mut self, map: usize) {
        if !self.map_rows().iter().any(|row| row.map == map) {
            return;
        }
        self.context.map = map;
        self.refresh();
    }
}

/// The section page: the context card, the summary cards, the damage layers and
/// the mastery breakdown.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
    // The tree's root is set before anything is borrowed: updating the child
    // entity needs the context mutably.
    let tree = state.damage_mastery.clone();
    if let Some(root) = state.damage.mastery_tree() {
        tree.update(cx, |tree, cx| tree.set_root(root, cx));
    }
    let damage = &state.damage;
    let context = damage.context;
    let snapshot = damage.snapshot.as_ref();
    let summary = damage.summary.clone();
    let note = match (snapshot, &summary) {
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

    let snapshot_choices: Vec<Choice> = match snapshot {
        None => Vec::new(),
        Some(snapshot) => (0..snapshot.characters())
            .map(|character| {
                let class = stats::class_name(snapshot.class_id(character)).replace('_', " ");
                Choice::new(
                    character.to_string(),
                    format!("{} ({class})", snapshot.name(character)),
                )
            })
            .collect(),
    };
    let rows = damage.map_rows();
    let map_choices: Vec<Choice> = rows
        .iter()
        .map(|row| Choice::new(row.map.to_string(), row.label.clone()))
        .collect();

    let character = entity.clone();
    let map = entity.clone();
    let has_save = snapshot.is_some();

    let controls = div()
        .id("damage-context-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .p_3()
        .child(
            field("Character", muted).child(
                Select::new("damage-character", snapshot_choices)
                    .label("Character")
                    .placeholder("Load a save first")
                    .selected(context.character.to_string())
                    .disabled(!has_save)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            character.update(cx, |state, cx| state.set_damage_character(index, cx));
                        }
                    }),
            ),
        )
        .child(
            field("Map", muted).child(
                Select::new("damage-map", map_choices)
                    .label("Map")
                    .placeholder("Load a save first")
                    .selected(context.map.to_string())
                    .disabled(!has_save)
                    .on_change(move |value, _window, cx| {
                        if let Ok(index) = value.parse::<usize>() {
                            map.update(cx, |state, cx| state.set_damage_map(index, cx));
                        }
                    }),
            ),
        );

    let summary_cards = summary_card(&summary, muted, cx);
    let layers = summary
        .as_ref()
        .map(|summary| layers_card(summary, damage.layer, muted, entity.clone()));
    let mastery = summary
        .as_ref()
        .map(|summary| mastery_breakdown(summary, tree, muted));

    div()
        .id("damage-page")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            ScrollArea::new("damage-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_3()
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
                                        .px_3()
                                        .pb_3()
                                        .text_size(px(12.0))
                                        .text_color(muted)
                                        .child(note),
                                ),
                        )
                        .child(summary_cards)
                        .children(layers)
                        .children(mastery),
                ),
        )
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
) -> AnyElement {
    let mut tabs = SegmentedControl::new("damage-layer-tabs", selected.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(layer) = LayerId::from_id(value) {
                entity.update(cx, |state, cx| state.set_damage_layer(layer, cx));
            }
        });
    for id in LayerId::ALL {
        tabs = tabs.segment(id.id(), id.id().to_uppercase(), None);
    }
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
        .child(div().id("damage-layer-controls").px_3().child(tabs))
        .child(
            div()
                .id("damage-layer")
                .role(Role::Label)
                .aria_label(format!("{}: {value}", selected.title()))
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .child(Caption::new(selected.title()).text_color(muted))
                .child(
                    div()
                        .id("damage-layer-value")
                        .role(Role::Label)
                        .aria_label(value.clone())
                        .text_size(px(20.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(value),
                )
                .child(
                    div()
                        .id("damage-layer-note")
                        .role(Role::Label)
                        .aria_label(note.clone())
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(note),
                ),
        )
        .into_any_element()
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
                    .px_3()
                    .pb_3()
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(summary.reason.clone()),
            )
            .into_any_element(),
    }
}

/// The four upstream cards. While the layers cannot be resolved they show a
/// dash and the reason, never a made-up zero.
fn summary_card(
    summary: &Option<Summary>,
    muted: gpui::Hsla,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(summary) = summary else {
        return div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                EmptyState::new(
                    "damage-summary-empty",
                    IconName::SearchX,
                    "No damage summary yet",
                )
                .body("Import a save to open the character and map the summary resolves against."),
            )
            .into_any_element();
    };
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
    let mastery_fill = (summary.mastery_fill() / 100.0) as f32;

    let mut body = div().flex().flex_col().gap_3().child(
        div()
            .id("damage-summary")
            .flex()
            .flex_wrap()
            .gap_3()
            .child(figure(
                "damage-game",
                "Character Sheet Range",
                game,
                game_sub,
                cx,
            ))
            .child(figure("damage-min", "Minimum Damage", min, exact_min, cx))
            .child(figure("damage-max", "Maximum Damage", max, exact_max, cx))
            .child(mastery_card(mastery, mastery_sub, mastery_fill, muted)),
    );
    if !summary.available {
        body = body.child(
            Callout::new(Severity::Warning)
                .title("Damage summary unavailable")
                .child(
                    Caption::new(summary.reason.clone())
                        .text_color(muted)
                        .into_any_element(),
                ),
        );
    }
    // The import's own gap, which upstream reports beside the value: the
    // `it.json` companion roster and the server's active vote.
    if summary.partial() {
        let reason = summary.partial_reason();
        body = body.child(
            div()
                .id("damage-partial")
                .role(Role::Label)
                .aria_label(reason.clone())
                .child(
                    Callout::new(Severity::Warning)
                        .title("Partial result")
                        .child(Caption::new(reason).text_color(muted).into_any_element()),
                ),
        );
    }
    body.into_any_element()
}

/// The mastery card: the ratio, its note and the upstream bar.
fn mastery_card(value: String, note: &'static str, fill: f32, muted: gpui::Hsla) -> AnyElement {
    let ratio = value.clone();
    div()
        .id("damage-mastery-card")
        .role(Role::Label)
        .aria_label(format!("Mastery: {value}"))
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(190.0))
        .child(
            Card::new().header(CardHeader::new("Mastery")).child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id("damage-mastery")
                            .text_size(px(20.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(value),
                    )
                    .child(Caption::new(note).text_color(muted))
                    // The shared bar draws no label of its own, so the card names
                    // the ratio it fills to.
                    .child(
                        div()
                            .id("damage-mastery-bar")
                            .role(Role::ProgressIndicator)
                            .aria_label(format!("Mastery bar: {ratio}"))
                            .child(ProgressBar::new("damage-mastery-bar-fill", fill)),
                    ),
            ),
        )
        .into_any_element()
}

fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(190.0))
        .child(Caption::new(label).text_color(muted))
}

/// Restore a project file's damage context, warning about values the save
/// cannot show.
pub(crate) fn restore(state: &mut AppState, context: DamageContext, warnings: &mut Vec<String>) {
    state.damage.context = context;
    state.damage.clamp(warnings);
    state.damage.refresh();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        state.load(&save());
        assert_eq!(state.context.character, 0);
        assert_eq!(state.context.map, 0);
        assert!(state.snapshot.is_some());
        assert_eq!(state.map_rows().first().map(|row| row.map), Some(0));
        let summary = state
            .summary
            .expect("the missing-pipeline state is computed");
        assert!(!summary.available);
        assert!(
            summary.reason.contains("shared stat pipeline"),
            "{summary:?}"
        );
        // A flat save carries neither the companion roster nor the vote row, so
        // the page names the partial import as the descriptor does.
        assert!(summary.partial());
        assert!(summary.partial_reason().contains("companion ownership"));
    }

    #[test]
    fn the_map_row_labels_carry_the_dungeon_and_current_markers() {
        let mut state = State::default();
        state.load(&save());
        let rows = state.map_rows();
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
        state.load(&save());
        state.select_character(1);
        assert_eq!(state.context.map, 50);
    }

    #[test]
    fn a_dungeon_map_reports_the_upstream_reason() {
        let mut state = State::default();
        state.load(&save());
        state.select_map(39);
        let summary = state.summary.expect("the dungeon state is computed");
        assert!(!summary.available);
        assert!(summary.reason.contains("Dungeon"), "{summary:?}");
    }

    #[test]
    fn the_layer_tabs_open_on_the_first_layer_and_switch() {
        let mut state = State::default();
        state.load(&save());
        assert_eq!(state.layer, LayerId::Base);
        state.select_layer(LayerId::Multiplier);
        assert_eq!(state.layer, LayerId::Multiplier);
        assert_eq!(LayerId::from_id("ddl2"), Some(state.layer));
    }

    #[test]
    fn the_layer_tabs_and_the_mastery_breakdown_read_the_summary() {
        // The provider has not landed, so the page has no layers to tab through
        // and no mastery terms to break down; it names the gap instead.
        let mut state = State::default();
        state.load(&save());
        assert_eq!(state.mastery_tree(), None);
        assert!(state.summary.as_ref().unwrap().layers().is_none());

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
        state.load(&save());
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
