//! The project export dialog: the whole window state as one versioned JSON
//! document, ready to copy.
//!
//! Ported from the upstream `js/ui/state-io.js` state box, which shows the
//! encoded string and a copy button. The document here is plain JSON (the
//! upstream `ROv2:` gzip+base64 state is an *import* format only) so a user can
//! read, edit and diff it; the box shows the head of it and the copy button
//! carries the whole thing.
//!
//! The same document carries one config object per section, so a section config
//! is a project with a single entry under `sections`; the import dialog takes it
//! on its own too.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::overlays::Dialog;
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{AnyElement, ClipboardItem, Context, IntoElement, ParentElement, Role, div, px};
use idleon_core::project;

/// How much of the document the dialog shows; the rest is on the clipboard.
///
/// One line, because a labelled node's text has to be free of control
/// characters for the accessibility tree the harness reads.
const SHOWN: usize = 400;

/// What the export dialog holds while it is open.
///
/// Everything the dialog shows is read once, when it opens: the document can
/// carry a save, and a per-frame clone of it would stall the window.
#[derive(Default)]
pub struct Export {
    pub open: bool,
    /// The whole project document, for the copy action.
    pub text: String,
    /// The document's head, whitespace-collapsed for a labelled one-line node.
    preview: String,
    /// How many section configs the document carries.
    sections: usize,
    /// Whether the document embeds the decoded save.
    has_save: bool,
}

impl Export {
    /// Open the dialog over a freshly written project document.
    pub fn open(state: &AppState) -> Self {
        let text = crate::project::export(state);
        let document = serde_json::from_str::<serde_json::Value>(&text).ok();
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
        let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview = one_line.chars().take(SHOWN).collect();
        Self {
            open: true,
            text,
            preview,
            sections,
            has_save,
        }
    }

    /// How many bytes the whole document has.
    pub fn bytes(&self) -> usize {
        self.text.len()
    }
}

/// The export dialog, rendered on top of the shell while it is open.
pub fn dialog(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let close = entity.clone();
    let cancel = entity.clone();
    let copy = entity.clone();
    let muted = cx.theme().colors.fg_muted;
    let subtle = cx.theme().colors.fg_subtle;
    let preview = state.export.preview.clone();
    let bytes = state.export.bytes();
    let sections = state.export.sections;
    let save = if state.export.has_save {
        "save included"
    } else {
        "no save"
    };

    Dialog::new("export-dialog", "Export project", move |window, cx| {
        close.update(cx, |state, cx| state.close_export(window, cx));
    })
    .detail(format!(
        "Project v{} · {sections} section(s) · {save} — copy it, or paste it back into Import.",
        project::VERSION
    ))
    .child(
        div()
            .id("export-summary")
            .role(Role::Label)
            .aria_label(format!("{bytes} bytes, {sections} section config"))
            .text_size(px(12.0))
            .text_color(muted)
            .child(format!("{bytes} bytes")),
    )
    .child(
        div()
            .id("export-text")
            .role(Role::Label)
            .aria_label(preview.clone())
            .w_full()
            .overflow_hidden()
            .whitespace_nowrap()
            .p_2()
            .rounded_md()
            .bg(cx.theme().colors.sunken)
            .text_size(px(11.0))
            .text_color(subtle)
            .child(preview),
    )
    .action(move |close| {
        Button::new("export-close", "Close")
            .variant(ButtonVariant::Subtle)
            .on_click(move |_event, window, cx| {
                cancel.update(cx, |state, cx| state.close_export(window, cx));
                close(window, cx);
            })
    })
    .action(move |close| {
        Button::new("export-copy", "Copy JSON")
            .primary()
            .on_click(move |_event, window, cx| {
                copy.update(cx, |state, cx| state.copy_export(cx));
                close(window, cx);
            })
    })
    .into_any_element()
}

/// Copy the whole document, so the box never has to lay out a save-sized
/// string just to hand it over.
pub fn copy(state: &AppState, cx: &mut Context<AppState>) {
    cx.write_to_clipboard(ClipboardItem::new_string(state.export.text.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> project::Project {
        project::Project {
            version: project::VERSION,
            section: Some("research".into()),
            save: None,
            simulation: idleon_core::model::Simulation::default(),
            context: idleon_core::model::SimContext::default(),
            sections: Default::default(),
            warnings: Vec::new(),
            origin: project::Origin::Project,
        }
    }

    #[test]
    fn an_open_export_holds_the_project_document() {
        let export = Export {
            open: true,
            text: project::write(&document()),
            ..Export::default()
        };
        assert!(export.open);
        assert!(export.text.contains(project::FORMAT), "{}", export.text);
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
