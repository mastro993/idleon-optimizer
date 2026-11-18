//! A dashed area that takes one dropped file; its Browse button opens the
//! file dialog instead. The kit has file chips (`attachment`) but no target.

use gpui::prelude::*;
use gpui::{
    App, ElementId, ExternalPaths, IntoElement, ParentElement, PathPromptOptions, RenderOnce,
    SharedString, Styled, Window, div,
};
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _};
use std::path::{Path, PathBuf};
use std::rc::Rc;

type OnDrop = Rc<dyn Fn(PathBuf, &mut Window, &mut App)>;

/// A drop target for one file of `kinds`, by extension in any case; no kinds
/// takes any file.
#[derive(IntoElement)]
pub struct DropZone {
    id: ElementId,
    hint: Option<SharedString>,
    kinds: &'static [&'static str],
    on_drop: Option<OnDrop>,
}

impl DropZone {
    /// A drop zone under `id` that takes one file of any kind. Its Browse
    /// button is `(id, "browse")`.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            hint: None,
            kinds: &[],
            on_drop: None,
        }
    }

    /// Takes only files with these extensions.
    pub fn kinds(mut self, kinds: &'static [&'static str]) -> Self {
        self.kinds = kinds;
        self
    }

    /// A line under the prompt, such as the files it takes.
    pub fn hint(mut self, text: impl Into<SharedString>) -> Self {
        self.hint = Some(text.into());
        self
    }

    /// Runs with the file dropped or chosen.
    pub fn on_drop(mut self, handler: impl Fn(PathBuf, &mut Window, &mut App) + 'static) -> Self {
        self.on_drop = Some(Rc::new(handler));
        self
    }
}

/// The one file `paths` brings when it is a file of `kinds`, or why not.
fn taken(paths: &[PathBuf], kinds: &[&str]) -> Result<PathBuf, &'static str> {
    let [path] = paths else {
        return Err(if paths.is_empty() {
            "nothing was dropped"
        } else {
            "one file at a time"
        });
    };
    if !path.is_file() {
        return Err("files only, not folders");
    }
    if !kinds.is_empty() && !fits(path, kinds) {
        return Err("not a kind this takes");
    }
    Ok(path.clone())
}

fn fits(path: &Path, kinds: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            kinds
                .iter()
                .any(|kind| kind.eq_ignore_ascii_case(extension))
        })
}

impl RenderOnce for DropZone {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (kinds, id) = (self.kinds, self.id.clone());
        let take: OnDrop = {
            let (id, on_drop) = (id.clone(), self.on_drop);
            Rc::new(move |path, window, cx| {
                log::info!("drop zone {id:?}: took {}", path.display());
                if let Some(on_drop) = &on_drop {
                    on_drop(path, window, cx);
                }
            })
        };
        let (dropped, chosen) = (take.clone(), take);
        let (hover_ok, hover_refused) = (
            (theme.drag_border, theme.drop_target),
            (theme.danger, theme.danger.opacity(0.1)),
        );
        div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .px_6()
            .py_8()
            .rounded(theme.radius_lg)
            .border_1()
            .border_dashed()
            .border_color(theme.border)
            .child(Icon::new(IconName::Inbox).text_color(theme.muted_foreground))
            .child(div().text_color(theme.foreground).child("Drop a file here"))
            .when_some(self.hint, |zone, hint| {
                zone.child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(hint),
                )
            })
            .child(
                Button::new((self.id, "browse"))
                    .label("Browse…")
                    .small()
                    .on_click(move |_, window, cx| browse(kinds, chosen.clone(), window, cx)),
            )
            .drag_over::<ExternalPaths>(move |style, paths, _, _| {
                let (border, bg) = if taken(paths.paths(), kinds).is_ok() {
                    hover_ok
                } else {
                    hover_refused
                };
                style.border_color(border).bg(bg)
            })
            .on_drop(
                move |paths: &ExternalPaths, window, cx| match taken(paths.paths(), kinds) {
                    Ok(path) => dropped(path, window, cx),
                    Err(reason) => log::info!("drop zone {id:?}: refused a drop: {reason}"),
                },
            )
    }
}

/// Opens the file dialog; a chosen file of `kinds` goes to `take`.
fn browse(kinds: &'static [&'static str], take: OnDrop, window: &mut Window, cx: &mut App) {
    let chosen = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("Choose".into()),
    });
    window
        .spawn(cx, async move |cx| {
            let paths = match chosen.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => return log::error!("drop zone: dialog failed: {error:#}"),
                Err(_) => return log::error!("drop zone: dialog closed without an answer"),
            };
            match taken(&paths, kinds) {
                Ok(path) => {
                    let _ = cx.update(|window, cx| take(path, window, cx));
                }
                Err(reason) => log::info!("drop zone: refused a choice: {reason}"),
            }
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::{DropZone, taken};
    use gpui::{
        Context, ExternalPaths, FileDropEvent, IntoElement, Modifiers, Render, TestAppContext,
        Window, point, px,
    };
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    /// A temporary directory holding `names` as files, removed on drop.
    struct Files(PathBuf);

    impl Files {
        fn new(tag: &str, names: &[&str]) -> Self {
            let dir =
                std::env::temp_dir().join(format!("idleon-drop-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(dir.join("folder")).unwrap();
            for name in names {
                std::fs::write(dir.join(name), "{}").unwrap();
            }
            Self(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_zone_takes_one_file_of_its_kinds_in_any_case() {
        let files = Files::new("kinds", &["save.json", "LOUD.JSON", "notes.md"]);
        let kinds = ["json", "txt"];
        let save = files.path("save.json");
        assert_eq!(taken(std::slice::from_ref(&save), &kinds), Ok(save.clone()));
        let loud = files.path("LOUD.JSON");
        assert_eq!(taken(std::slice::from_ref(&loud), &kinds), Ok(loud.clone()));
        assert_eq!(
            taken(&[files.path("notes.md")], &kinds),
            Err("not a kind this takes")
        );
        assert_eq!(
            taken(&[save.clone(), loud], &kinds),
            Err("one file at a time")
        );
        assert_eq!(
            taken(&[files.path("folder")], &[]),
            Err("files only, not folders")
        );
        assert_eq!(taken(&[], &kinds), Err("nothing was dropped"));
        assert_eq!(
            taken(&[files.path("notes.md")], &[]),
            Ok(files.path("notes.md"))
        );
    }

    struct Zone(Rc<RefCell<Vec<PathBuf>>>);

    impl Render for Zone {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let taken = self.0.clone();
            DropZone::new("import-drop")
                .kinds(&["json"])
                .on_drop(move |path, _, _| taken.borrow_mut().push(path))
        }
    }

    #[gpui::test]
    fn dropping_a_file_of_its_kind_runs_the_handler(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let files = Files::new("event", &["save.json", "notes.md"]);
        let taken = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view({
            let taken = taken.clone();
            move |_, _| Zone(taken)
        });
        let center = point(px(100.0), px(60.0));
        for name in ["notes.md", "save.json"] {
            let paths = ExternalPaths(vec![files.path(name)].into());
            cx.simulate_event(FileDropEvent::Entered {
                position: center,
                paths,
            });
            cx.simulate_event(FileDropEvent::Submit { position: center });
        }
        let _ = Modifiers::none();
        assert_eq!(*taken.borrow(), vec![files.path("save.json")]);
    }
}
