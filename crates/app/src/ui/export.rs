//! The project export dialog: the whole window state as one versioned JSON
//! document.
//!
//! The project document is plain JSON so a user can read, edit and diff it. It
//! is shown as a head and copied whole from its button.
//!
//! The same document carries one config object per section, so a section config
//! is a project with a single entry under `sections`; the import dialog takes it
//! on its own too.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{App, ClipboardItem, Context, Entity, ParentElement, Role, Styled, div};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{Dialog, DialogDescription, DialogFooter};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_project as project;
use idleon_ui::widgets::{TEXT_CAPTION, TEXT_MONO};

/// How much of the project document the dialog shows; the rest is on the
/// clipboard.
///
/// One line, because a labelled node's text has to be free of control
/// characters for the accessibility tree the harness reads.
const SHOWN: usize = 400;

/// What the export dialog holds while it is open.
///
/// Everything the dialog shows is read once, when it opens: the project can
/// carry a save, and a per-frame clone of it would stall the window.
#[derive(Default)]
pub(crate) struct Export {
    pub(crate) open: bool,
    /// The whole project document, for the copy action.
    pub(crate) project: String,
    /// The document's head, whitespace-collapsed for a labelled one-line node.
    preview: String,
    /// How many section configs the document carries.
    sections: usize,
    /// Whether the document embeds the decoded save.
    has_save: bool,
}

impl Export {
    /// Open the dialog over a freshly written project document.
    pub(crate) fn open(state: &AppState) -> Self {
        let project = crate::project::export(state);
        let document = serde_json::from_str::<serde_json::Value>(&project).ok();
        let sections = document
            .as_ref()
            .and_then(|document| {
                document
                    .get("sections")?
                    .as_object()
                    .map(serde_json::Map::len)
            })
            .unwrap_or(0);
        let has_save = document
            .as_ref()
            .is_some_and(|document| document.get("save").is_some_and(|save| !save.is_null()));
        let one_line = project.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview = one_line.chars().take(SHOWN).collect();
        Self {
            open: true,
            project,
            preview,
            sections,
            has_save,
        }
    }

    /// How many bytes the whole project document has.
    pub(crate) fn bytes(&self) -> usize {
        self.project.len()
    }
}

/// The export dialog's content, built by the kit's dialog layer while it is
/// open (`AppState::open_export`).
pub(crate) fn dialog(entity: &Entity<AppState>, dialog: Dialog, cx: &App) -> Dialog {
    let export = &entity.read(cx).export;
    let theme = cx.theme();
    let close = entity.clone();
    let cancel = entity.clone();
    let copy = entity.clone();
    let bytes = export.bytes();
    let sections = export.sections;
    let save = if export.has_save {
        "save included"
    } else {
        "no save"
    };

    dialog
        .title("Export project")
        .on_close(move |_event, window, cx| {
            close.update(cx, |state, cx| state.close_export(window, cx));
        })
        .child(
            v_flex()
                .id("export-dialog")
                .gap_3()
                .child(DialogDescription::new().child(format!(
                    "Project v{} · {sections} section(s) · {save} — copy the JSON.",
                    project::VERSION
                )))
                .child(
                    div()
                        .id("export-summary")
                        .role(Role::Label)
                        .aria_label(format!("{bytes} bytes, {sections} section config"))
                        .text_size(TEXT_MONO)
                        .text_color(theme.muted_foreground)
                        .child(format!("{bytes} bytes")),
                )
                .child(
                    div()
                        .id("export-text")
                        .role(Role::Label)
                        .aria_label(export.preview.clone())
                        .w_full()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .p_2()
                        .rounded(theme.radius)
                        .bg(theme.muted)
                        .text_size(TEXT_CAPTION)
                        .text_color(theme.table_head_foreground)
                        .child(export.preview.clone()),
                ),
        )
        .footer(
            DialogFooter::new()
                .child(
                    Button::new("export-close")
                        .label("Close")
                        .secondary()
                        .on_click(move |_event, window, cx| {
                            cancel.update(cx, |state, cx| state.close_export(window, cx));
                        }),
                )
                .child(
                    Button::new("export-copy")
                        .label("Copy JSON")
                        .primary()
                        .on_click(move |_event, window, cx| {
                            copy.update(cx, |state, cx| {
                                state.copy_export(cx);
                                state.close_export(window, cx);
                            });
                        }),
                ),
        )
}

/// Copy the whole document, so the box never has to lay out a save-sized
/// string just to hand it over.
pub(crate) fn copy(state: &AppState, cx: &mut Context<AppState>) {
    cx.write_to_clipboard(ClipboardItem::new_string(state.export.project.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> project::Project {
        project::Project {
            version: project::VERSION,
            section: Some("research".into()),
            save: None,
            simulation: idleon_research::model::Simulation::default(),
            context: idleon_research::model::SimContext::default(),
            sections: Default::default(),
            warnings: Vec::new(),
            has_research: true,
            has_supplements: true,
        }
    }

    #[test]
    fn an_open_export_holds_the_project_document() {
        let export = Export {
            open: true,
            project: project::write(&document()),
            ..Export::default()
        };
        assert!(export.open);
        assert!(
            export.project.contains(project::FORMAT),
            "{}",
            export.project
        );
        assert!(export.bytes() > 0);
    }

    #[test]
    fn a_closed_dialog_holds_nothing() {
        let export = Export::default();
        assert!(!export.open);
        assert_eq!(export.bytes(), 0);
        assert!(!export.has_save);
        assert_eq!(export.sections, 0);
    }
}
