//! The one import path every section shares: drop a file, browse for one or
//! paste the JSON, preview what it detected, then apply it.
//!
//! Ported from the upstream `js/ui/save-loader.js`, which loads a file or a paste
//! into `js/state.js` and reports the load in the bar it injects, and from
//! `js/ui/state-io.js`, whose state box reads the gzip+base64 `ROv2:` string.
//! The dialog adds the preview the upstream lacks: the detected format, the
//! research level, the section count and the warnings are shown before anything
//! is applied, and a failed parse only replaces the message, never the loaded
//! run.
//!
//! Four documents go through it: a raw decoded save, a research state, a whole
//! project file and one section's config.

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
use idleon_core::project;
use idleon_core::save::Imported;
use std::path::Path;

/// How many warnings the preview lists before it counts the rest.
const SHOWN_WARNINGS: usize = 3;

/// What a parsed import text turned out to be, waiting for Apply.
#[derive(Clone, Debug)]
pub enum Pending {
    /// A raw decoded save or a research state, as the save importer reads it.
    Save(Box<Imported>),
    /// A whole project document: the save, the research state and every config.
    Project(Box<project::Project>),
    /// One section's config document.
    Section(project::SectionConfig),
}

impl Pending {
    /// The warnings the preview counts and lists.
    pub fn warnings(&self) -> &[String] {
        match self {
            Pending::Save(imported) => &imported.warnings,
            Pending::Project(project) => &project.warnings,
            Pending::Section(section) => &section.warnings,
        }
    }

    /// The format the preview names.
    pub fn format(&self) -> String {
        match self {
            Pending::Save(imported) => imported.format.label().to_string(),
            Pending::Project(project) => match project.origin {
                project::Origin::Project => "project".into(),
                project::Origin::UpstreamState => "upstream state".into(),
            },
            Pending::Section(_) => "section config".into(),
        }
    }

    /// The research level the document carries, when it carries one.
    fn level(&self) -> Option<u32> {
        match self {
            Pending::Save(imported) => Some(imported.simulation.research_level),
            Pending::Project(project) => Some(project.simulation.research_level),
            Pending::Section(_) => None,
        }
    }

    /// The section a config document targets, for the summary line.
    fn section(&self) -> Option<&str> {
        match self {
            Pending::Section(section) => Some(&section.section),
            _ => None,
        }
    }

    /// How many section configs the document carries.
    fn sections(&self) -> usize {
        match self {
            Pending::Project(project) => project.sections.len(),
            Pending::Section(_) => 1,
            Pending::Save(_) => 0,
        }
    }
}

/// What the dialog is holding while it is open.
pub struct Import {
    pub open: bool,
    /// The pasted JSON or state string.
    pub input: Entity<TextInput>,
    /// Where the previewed text came from: the paste box, or a dropped file.
    pub source: String,
    /// The line under the preview: what the last parse said.
    pub message: String,
    /// Whether the last parse succeeded.
    pub ok: bool,
    /// The parsed document waiting for Apply. A failed parse leaves it empty, so
    /// a failure can never replace the loaded run.
    pub pending: Option<Pending>,
}

impl Import {
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multi_line(4, 10)
                .placeholder("Paste it.json, save.json, a project file or an ROv2 state string…")
                .label("Import JSON")
        });
        Self {
            open: false,
            input,
            source: "pasted text".into(),
            message: "Paste a decoded save, a project file, an ROv2 state string, or drop one."
                .into(),
            ok: true,
            pending: None,
        }
    }
}

/// The line under the preview: what applying would load.
pub fn summary(pending: &Pending) -> String {
    let format = pending.format();
    let level = pending
        .level()
        .map(|level| format!(" · research level {level}"))
        .unwrap_or_default();
    let section = pending
        .section()
        .map(|id| format!(" · {}", section_title(id)))
        .unwrap_or_default();
    let sections = match pending {
        Pending::Project(project) if !project.sections.is_empty() => {
            format!(" · {} section config(s)", pending.sections())
        }
        _ => String::new(),
    };
    let characters = match pending {
        Pending::Save(imported) => format!(" · {} character(s)", imported.characters),
        _ => String::new(),
    };
    let fleet = match pending {
        Pending::Save(imported) => fleet(imported),
        _ => String::new(),
    };
    let farming = match pending {
        Pending::Save(imported) => farming_suffix(imported),
        _ => String::new(),
    };
    let warnings = pending.warnings().len();
    format!(
        "Ready to apply: {format}{section}{level}{fleet}{farming}{characters}{sections} · {warnings} warning(s)."
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

/// The line the dialog keeps once the document is applied.
pub fn applied(pending: &Pending) -> String {
    match pending {
        Pending::Save(imported) => format!(
            "Applied {} · research level {}{}{} · {} character(s).",
            imported.format.label(),
            imported.simulation.research_level,
            fleet(imported),
            farming_suffix(imported),
            imported.characters
        ),
        Pending::Project(project) => format!(
            "Applied the {} · research level {}{} · {} section config(s).",
            match project.origin {
                project::Origin::Project => "project",
                project::Origin::UpstreamState => "upstream state",
            },
            project.simulation.research_level,
            if project.save.is_some() {
                " · save included"
            } else {
                ""
            },
            project.sections.len()
        ),
        Pending::Section(section) => {
            format!("Applied the {} config.", section_title(&section.section))
        }
    }
}

/// A section id's registry title, or the id itself when the build lacks it.
fn section_title(id: &str) -> String {
    crate::sections::find(id)
        .map(|section| section.title.to_string())
        .unwrap_or_else(|| id.to_string())
}

/// The fleet count when the save carries one, so a sailing user knows it loaded.
fn fleet(import: &Imported) -> String {
    if import.fleet.boats.is_empty() {
        String::new()
    } else {
        format!(" · {} boat(s)", import.fleet.boats.len())
    }
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
        .kinds(&["json", "txt"])
        .hint("it.json, save.json, a project file, a section config or an ROv2 state string")
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

    Dialog::new(
        "import-dialog",
        "Import save or project",
        move |window, cx| {
            close.update(cx, |state, cx| state.close_import(window, cx));
        },
    )
    .detail("Drop a file, browse for one, or paste the JSON or state string.")
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
            .aria_label("Import JSON")
            .on_click(move |_event, window, cx| window.focus(&input_focus, cx))
            .child(Input::new(&state.import.input).size(ControlSize::Sm)),
    )
    .child(preview(state, cx))
    // While a document is pending the preview says everything; the line below it
    // is for the prompt and for a parse that failed.
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

/// The detected format, research level, section count and warnings, or the
/// prompt to import something.
fn preview(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let colors = &cx.theme().colors;
    let (muted, subtle, danger) = (colors.fg_muted, colors.fg_subtle, colors.danger);
    let Some(pending) = &state.import.pending else {
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
    let warnings = pending.warnings();
    let shown = warnings.len().min(SHOWN_WARNINGS);
    let mut list = div().flex().flex_col().gap_1();
    for (index, warning) in warnings.iter().take(shown).enumerate() {
        list = list.child(
            div()
                .id(("import-warning", index))
                .role(Role::Label)
                .aria_label(warning.clone())
                .text_size(px(11.0))
                .text_color(muted)
                .child(format!("· {warning}")),
        );
    }
    let hidden = warnings.len() - shown;
    let mut figures = div().flex().flex_wrap().gap_4().child(stat(
        "import-format",
        "Format",
        &pending.format(),
        subtle,
    ));
    match pending {
        Pending::Save(imported) => {
            figures = figures.child(stat(
                "import-characters",
                "Characters",
                &imported.characters.to_string(),
                subtle,
            ));
        }
        Pending::Project(_) => {
            figures = figures.child(stat(
                "import-sections",
                "Sections",
                &pending.sections().to_string(),
                subtle,
            ));
        }
        Pending::Section(section) => {
            figures = figures.child(stat(
                "import-section",
                "Section",
                &section_title(&section.section),
                subtle,
            ));
        }
    }
    if let Some(level) = pending.level() {
        figures = figures.child(stat(
            "import-level",
            "Research level",
            &level.to_string(),
            subtle,
        ));
    }
    figures = figures.child(stat(
        "import-warnings",
        "Warnings",
        &warnings.len().to_string(),
        if warnings.is_empty() { subtle } else { danger },
    ));
    div()
        .id("import-preview")
        .role(Role::Group)
        .aria_label(format!(
            "{} · {} warning(s)",
            pending.format(),
            warnings.len()
        ))
        .flex()
        .flex_col()
        .gap_2()
        .child(figures)
        .child(
            div()
                .id("import-source")
                .role(Role::Label)
                .aria_label(format!("From {}", state.import.source))
                .text_size(px(11.0))
                .text_color(muted)
                .child(format!("From {}", state.import.source)),
        )
        .when(!warnings.is_empty(), |preview| {
            preview.child(list).when(hidden > 0, |preview| {
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
    use idleon_core::save::Format;

    fn imported(format: Format, characters: usize, warnings: &[&str]) -> Pending {
        Pending::Save(Box::new(Imported {
            simulation: Simulation {
                research_level: 122,
                ..Simulation::default()
            },
            context: Default::default(),
            fleet: Default::default(),
            sailing: Default::default(),
            farming: Default::default(),
            tree: Default::default(),
            clamworks: Default::default(),
            jar: None,
            save: idleon_core::save::Save::new(serde_json::Value::Null),
            companion: None,
            statues: Default::default(),
            coral: Default::default(),
            format,
            characters,
            named_characters: characters,
            warnings: warnings.iter().map(|warning| warning.to_string()).collect(),
            shadow: Default::default(),
        }))
    }

    fn project(warnings: &[&str]) -> Pending {
        Pending::Project(Box::new(project::Project {
            version: project::VERSION,
            section: Some("killroy".into()),
            save: Some(serde_json::json!({})),
            simulation: Simulation {
                research_level: 140,
                ..Simulation::default()
            },
            context: Default::default(),
            sections: [("killroy".to_string(), serde_json::json!({ "weeks": 12 }))]
                .into_iter()
                .collect(),
            warnings: warnings.iter().map(|warning| warning.to_string()).collect(),
            origin: project::Origin::UpstreamState,
        }))
    }

    fn section() -> Pending {
        Pending::Section(project::SectionConfig {
            version: project::VERSION,
            section: "killroy".into(),
            config: serde_json::json!({ "weeks": 12 }),
            warnings: Vec::new(),
        })
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
        let Pending::Save(mut import) = imported(Format::SaveJson, 1, &[]) else {
            unreachable!()
        };
        import.farming.plots[0].seed_family = 0;
        let line = summary(&Pending::Save(import));
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
    fn a_project_summary_names_the_upstream_state_and_its_sections() {
        let line = summary(&project(&["a"]));
        assert!(line.contains("upstream state"), "{line}");
        assert!(line.contains("research level 140"), "{line}");
        assert!(line.contains("1 section config"), "{line}");
        assert!(line.contains("1 warning"), "{line}");
        let line = applied(&project(&[]));
        assert!(line.contains("upstream state"), "{line}");
        assert!(line.contains("save included"), "{line}");
    }

    #[test]
    fn a_section_config_summary_names_the_section() {
        let line = summary(&section());
        assert!(line.contains("section config"), "{line}");
        assert!(line.contains("Killroy Calculator"), "{line}");
        assert!(line.contains("0 warning"), "{line}");
        assert_eq!(
            applied(&section()),
            "Applied the Killroy Calculator config."
        );
    }

    #[test]
    fn a_dropped_file_reports_its_name_and_a_root_its_path() {
        assert_eq!(file_name(Path::new("/tmp/save.json")), "save.json");
        assert_eq!(file_name(Path::new("/")), "/");
    }
}
