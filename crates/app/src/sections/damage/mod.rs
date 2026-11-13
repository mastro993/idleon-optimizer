//! The Damage Breakdown section: the two upstream context selectors and the
//! four summary cards.
//!
//! Ported from the summary block in `damage-calc.html`: the character and map
//! selects, the "Character Sheet Range" card, the minimum and maximum cards
//! with their exact values, and the mastery card with its bar. The three damage
//! layers come from the upstream shared stat pipeline, which this build does
//! not decode, so the page renders the upstream dungeon and missing-pipeline
//! states instead of a number it cannot justify. The context, the selector rows,
//! the layer arithmetic and the upstream formatting live in [`idleon_damage`]
//! and are ready for that provider.

mod actions;

use crate::state::AppState;
use crate::ui::layout::figure;
use ely_gpui_component::feedback::{Callout, EmptyState};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::motion::ProgressBar;
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use idleon_damage::{self as damage, Context as DamageContext, MapRow, Summary};
use idleon_save::Save;
use idleon_stats::{self as stats, Snapshot};
use idleon_ui::format;

/// The section's own state: the context and the save slice it reads.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) context: DamageContext,
    pub(crate) snapshot: Option<Snapshot>,
    /// The last computed summary; `None` before a save loads.
    pub(crate) summary: Option<Summary>,
    save: Option<Save>,
}

impl State {
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

/// The section page: the context card and the summary cards.
pub(crate) fn view(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
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
                        .child(summary_cards),
                ),
        )
        .into_any_element()
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
                    .child(ProgressBar::new("damage-mastery-bar", fill)),
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
