//! The one import path every section shares: drop a file, browse for one or
//! paste the JSON, preview what it detected, then apply it.
//!
//! Ported from the upstream `js/ui/save-loader.js`, which loads a file or a paste
//! into `js/state.js` and reports the load in the bar it injects. The dialog adds
//! the preview the upstream lacks: the detected format, the research level, the
//! character count and the warnings are shown before anything is applied, and a
//! failed parse only replaces the message, never the loaded run.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::forms::{DropZone, Input, TextInput};
use ely_gpui_component::overlays::Dialog;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Role, SharedString, Styled, Window,
    div, px,
};
use idleon_core::save::{Format, Imported};
use std::path::Path;

/// How many warnings the preview lists before it counts the rest.
const SHOWN_WARNINGS: usize = 3;

/// What the dialog is holding while it is open.
pub struct Import {
    pub open: bool,
    /// The pasted JSON.
    pub input: Entity<TextInput>,
    /// Where the previewed text came from: the paste box, or a dropped file.
    pub source: String,
    /// The line under the preview: what the last parse said.
    pub message: String,
    /// Whether the last parse succeeded.
    pub ok: bool,
    /// The parsed save waiting for Apply. A failed parse leaves it empty, so a
    /// failure can never replace the loaded run.
    pub pending: Option<Imported>,
}

impl Import {
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line(4, 10)
                .placeholder("Paste it.json or save.json contents…")
                .label("Save JSON")
        });
        Self {
            open: false,
            input,
            source: "pasted text".into(),
            message: "Paste a decoded save, or drop a JSON file.".into(),
            ok: true,
            pending: None,
        }
    }
}

/// The line under the preview: what applying would load.
pub fn summary(import: &Imported) -> String {
    format!(
        "Ready to apply: {} · research level {}{} · {} character(s) · {} warning(s).",
        import.format.label(),
        import.simulation.research_level,
        farming_suffix(import),
        import.characters,
        import.warnings.len()
    )
}

/// The farm plots the import carries, when it carries any.
fn farming_suffix(import: &Imported) -> String {
    if import.farming.has_plots() {
        format!(" · {} plots", import.farming.plots.len())
    } else {
        String::new()
    }
}

/// The line the dialog keeps once the save is applied.
pub fn applied(import: &Imported) -> String {
    format!(
        "Applied {} · research level {}{} · {} character(s).",
        import.format.label(),
        import.simulation.research_level,
        farming_suffix(import),
        import.characters
    )
}

/// The import dialog, rendered on top of the shell while it is open.
pub fn dialog(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let close = entity.clone();
    let cancel = entity.clone();
    let apply = entity.clone();
    let pending = state.import.pending.is_some();
    let input_focus = state.import.input.read(cx).focus().clone();

    let dropped = entity.clone();
    let drop_zone = DropZone::new("import-drop")
        .kinds(&["json"])
        .hint("it.json, save.json or an app state export")
        .on_drop(move |paths, _window, cx| {
            let Some(path) = paths.first() else {
                return;
            };
            let source = file_name(path);
            match std::fs::read_to_string(path) {
                Ok(text) => dropped.update(cx, |state, cx| state.check_import(&text, &source, cx)),
                Err(error) => dropped.update(cx, |state, cx| {
                    state.fail_import(&source, &error.to_string(), cx);
                }),
            }
        });

    Dialog::new("import-dialog", "Import save", move |window, cx| {
        close.update(cx, |state, cx| state.close_import(window, cx));
    })
    .detail("Drop a file, browse for one, or paste the decoded JSON.")
    .child(drop_zone)
    .child(
        div()
            .id("import-paste-row")
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().h(px(1.0)).bg(cx.theme().colors.border))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().colors.fg_muted)
                    .child("or paste it"),
            )
            .child(div().flex_1().h(px(1.0)).bg(cx.theme().colors.border)),
    )
    .child(
        div()
            .id("import-input")
            .aria_label("Save JSON")
            .on_click(move |_event, window, cx| window.focus(&input_focus, cx))
            .child(Input::new(&state.import.input).size(ControlSize::Sm)),
    )
    .child(preview(state, cx))
    // While a save is pending the preview says everything; the line below it is
    // for the prompt and for a parse that failed.
    .when(state.import.pending.is_none(), |dialog| {
        dialog.child(
            div()
                .id("import-status")
                .role(Role::Label)
                .aria_label(state.import.message.clone())
                .text_size(px(12.0))
                .text_color(if state.import.ok {
                    cx.theme().colors.fg_muted
                } else {
                    cx.theme().colors.danger
                })
                .child(state.import.message.clone()),
        )
    })
    .action(move |close| {
        Button::new("import-cancel", "Cancel")
            .variant(ButtonVariant::Subtle)
            .on_click(move |_event, window, cx| {
                cancel.update(cx, |state, cx| state.close_import(window, cx));
                close(window, cx);
            })
    })
    .action(move |close| {
        Button::new("import-apply", "Apply")
            .primary()
            .disabled(!pending)
            .on_click(move |_event, window, cx| {
                apply.update(cx, |state, cx| state.apply_import(window, cx));
                close(window, cx);
            })
    })
    .into_any_element()
}

/// The detected format, research level, character count and warnings, or the
/// prompt to import something.
fn preview(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let colors = &cx.theme().colors;
    let (muted, subtle, danger) = (colors.fg_muted, colors.fg_subtle, colors.danger);
    let Some(import) = &state.import.pending else {
        return div()
            .id("import-preview")
            .role(Role::Label)
            .aria_label("Nothing to preview yet")
            .text_size(px(12.0))
            .text_color(muted)
            .child(if state.import.source == "pasted text" {
                "Nothing to preview yet.".to_string()
            } else {
                format!("Nothing to preview from {}.", state.import.source)
            })
            .into_any_element();
    };
    let format: Format = import.format;
    let level = import.simulation.research_level;
    let characters = import.characters;
    let shown = import.warnings.len().min(SHOWN_WARNINGS);
    let mut warnings = div().flex().flex_col().gap_1();
    for (index, warning) in import.warnings.iter().take(shown).enumerate() {
        warnings = warnings.child(
            div()
                .id(("import-warning", index))
                .role(Role::Label)
                .aria_label(warning.clone())
                .text_size(px(11.0))
                .text_color(muted)
                .child(format!("· {warning}")),
        );
    }
    let hidden = import.warnings.len() - shown;
    div()
        .id("import-preview")
        .role(Role::Group)
        .aria_label(format!(
            "{} · research level {level} · {characters} characters · {} warnings",
            format.label(),
            import.warnings.len()
        ))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_4()
                .child(stat("import-format", "Format", format.label(), subtle))
                .child(stat(
                    "import-level",
                    "Research level",
                    &level.to_string(),
                    subtle,
                ))
                .child(stat(
                    "import-characters",
                    "Characters",
                    &characters.to_string(),
                    subtle,
                ))
                .child(stat(
                    "import-warnings",
                    "Warnings",
                    &import.warnings.len().to_string(),
                    if import.warnings.is_empty() {
                        subtle
                    } else {
                        danger
                    },
                )),
        )
        .child(
            div()
                .id("import-source")
                .role(Role::Label)
                .aria_label(format!("From {}", state.import.source))
                .text_size(px(11.0))
                .text_color(muted)
                .child(format!("From {}", state.import.source)),
        )
        .when(!import.warnings.is_empty(), |preview| {
            preview.child(warnings).when(hidden > 0, |preview| {
                preview.child(
                    div()
                        .id("import-warnings-more")
                        .role(Role::Label)
                        .aria_label(format!("and {hidden} more warnings"))
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(format!("…and {hidden} more")),
                )
            })
        })
        .into_any_element()
}

/// One preview figure: its name above its value.
fn stat(id: &'static str, label: &'static str, value: &str, tone: gpui::Hsla) -> AnyElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{label}: {value}"))
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(tone)
                .child(SharedString::from(label)),
        )
        .child(
            div()
                .text_size(px(13.0))
                .child(SharedString::from(value.to_string())),
        )
        .into_any_element()
}

/// The dropped file's name, or its whole path for a root.
fn file_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_core::model::Simulation;

    fn imported(format: Format, characters: usize, warnings: &[&str]) -> Imported {
        Imported {
            simulation: Simulation {
                research_level: 122,
                ..Simulation::default()
            },
            context: Default::default(),
            farming: Default::default(),
            clamworks: Default::default(),
            save: serde_json::Value::Null,
            format,
            characters,
            named_characters: characters,
            warnings: warnings.iter().map(|warning| warning.to_string()).collect(),
            shadow: Default::default(),
        }
    }

    #[test]
    fn the_summary_names_the_format_level_characters_and_warnings() {
        let line = summary(&imported(Format::SaveJson, 11, &["a", "b"]));
        assert!(line.contains("save.json"), "{line}");
        assert!(line.contains("122"), "{line}");
        assert!(line.contains("11 character"), "{line}");
        assert!(line.contains("2 warning"), "{line}");

        let clean = summary(&imported(Format::ItJson, 3, &[]));
        assert!(clean.contains("it.json"), "{clean}");
        assert!(clean.contains("0 warning"), "{clean}");
    }

    #[test]
    fn the_summary_names_the_farm_plots_when_the_save_carries_any() {
        let mut import = imported(Format::SaveJson, 1, &[]);
        import.farming.plots[0].seed_family = 0;
        let line = summary(&import);
        assert!(line.contains("36 plots"), "{line}");

        let farmless = imported(Format::SaveJson, 1, &[]);
        assert!(!summary(&farmless).contains("plots"), "{farmless:?}");
    }

    #[test]
    fn the_applied_line_leaves_the_warnings_to_the_dialog() {
        let line = applied(&imported(Format::ResearchState, 100, &["a"]));
        assert!(line.contains("research state"), "{line}");
        assert!(line.contains("100"), "{line}");
        assert!(!line.contains("warning"), "{line}");
    }

    #[test]
    fn a_dropped_file_reports_its_name_and_a_root_its_path() {
        assert_eq!(file_name(Path::new("/tmp/save.json")), "save.json");
        assert_eq!(file_name(Path::new("/")), "/");
    }
}
