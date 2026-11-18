//! The window's cached views: the header, the sidebar, Home, the status line
//! and one page per opened section ([ADR 0004]).
//!
//! `AppState` stays the window's root view and its one model. The shell embeds
//! these views cached, so a frame replays a view's last output unless that
//! view was notified. Each view re-renders only for what it reads: a section
//! page on every notify of the state (its controls make them) and on its own
//! runs' reports; the other views when the fields their `reads` function
//! returns differ from the last render's. A run's progress is a
//! [`RunReported`] event, not a notify, so it reaches only the views that show it.
//!
//! [ADR 0004]: ../../../../docs/adr/0004-the-window-renders-cached-views-that-follow-what-they-read.md

use crate::sections::{SECTIONS, Section};
use crate::session::SidebarFold;
use crate::state::{AppState, Route, RunReported, SaveStatus, Shown};
use crate::ui::{dashboard, header, sidebar};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Empty, Entity, IntoElement, ParentElement, Role, Styled,
    Subscription, WeakEntity, Window, div,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Draws a view's part of the window from the state.
type Draw = fn(&mut AppState, &mut Window, &mut Context<AppState>) -> AnyElement;

/// What the header reads: the fold its toggle names, the history and the save status.
#[derive(PartialEq)]
pub(crate) struct HeaderReads {
    fold: SidebarFold,
    places: Vec<Route>,
    position: usize,
    save: SaveStatus,
}

/// What the sidebar reads: the fold, the open place and every section's activity.
#[derive(PartialEq)]
pub(crate) struct SidebarReads {
    fold: SidebarFold,
    route: Route,
    activity: Vec<Option<Shown>>,
}

/// What Home reads: the search text and the section Resume offers.
#[derive(PartialEq)]
pub(crate) struct HomeReads {
    search: String,
    resume: Option<&'static str>,
}

/// What the status line reads.
#[derive(PartialEq)]
pub(crate) struct StatusReads(String);

/// The views the shell embeds, made on the first frame: they observe the
/// state, whose entity only exists once it is built.
pub(crate) struct Views {
    pub(crate) header: Entity<Pane<HeaderReads>>,
    pub(crate) sidebar: Entity<Pane<SidebarReads>>,
    pub(crate) home: Entity<Pane<HomeReads>>,
    pub(crate) status: Entity<Pane<StatusReads>>,
    /// One page per section opened in this window, kept so a revisit reuses it.
    pages: HashMap<&'static str, Entity<Page>>,
    fold: Fold,
}

impl Views {
    fn new(state: &Entity<AppState>, cx: &mut App) -> Self {
        Self {
            header: cx.new(|cx| Pane::new(state, header_reads, draw_header, cx)),
            sidebar: cx.new(|cx| Pane::new(state, sidebar_reads, draw_sidebar, cx)),
            home: cx.new(|cx| Pane::new(state, home_reads, draw_home, cx)),
            status: cx.new(|cx| Pane::new(state, status_reads, draw_status, cx)),
            pages: HashMap::new(),
            fold: Fold::default(),
        }
    }

    /// The window's views, made on its first frame.
    pub(crate) fn of<'a>(state: &'a mut AppState, cx: &mut Context<AppState>) -> &'a mut Self {
        state
            .views
            .get_or_insert_with(|| Self::new(&cx.entity(), cx))
    }

    /// `section`'s page, made the first time it opens.
    pub(crate) fn page(
        &mut self,
        section: &'static Section,
        state: &Entity<AppState>,
        cx: &mut App,
    ) -> Entity<Page> {
        self.pages
            .entry(section.id)
            .or_insert_with(|| cx.new(|cx| Page::new(state, section, cx)))
            .clone()
    }

    /// Whether the sidebar is folding or unfolding to `rail` within `duration`
    /// of the change. Ely's `Sidebar` animates its own width meanwhile, which a
    /// cached view laid out at a fixed width would clip, so the shell draws it
    /// live until the animation ends.
    pub(crate) fn sidebar_folding(&mut self, rail: SidebarFold, duration: Duration) -> bool {
        self.fold.folding(rail, duration)
    }
}

/// When the sidebar last changed fold.
#[derive(Default)]
struct Fold {
    rail: Option<SidebarFold>,
    changed: Option<Instant>,
}

impl Fold {
    fn folding(&mut self, rail: SidebarFold, duration: Duration) -> bool {
        if self.rail.is_some_and(|was| was != rail) {
            self.changed = Some(Instant::now());
        }
        self.rail = Some(rail);
        self.changed.is_some_and(|at| at.elapsed() < duration)
    }
}

/// A part of the shell that re-renders when what `reads` returns changes.
pub(crate) struct Pane<K> {
    state: WeakEntity<AppState>,
    reads: fn(&AppState, &App) -> K,
    draw: Draw,
    /// What the last render, or the last change check, read.
    read: Option<K>,
    _subscriptions: [Subscription; 2],
}

impl<K: PartialEq + 'static> Pane<K> {
    fn new(
        state: &Entity<AppState>,
        reads: fn(&AppState, &App) -> K,
        draw: Draw,
        cx: &mut Context<Self>,
    ) -> Self {
        let notified = cx.observe(state, |pane, state, cx| pane.check(&state, cx));
        let ran = cx.subscribe(state, |pane, state, _: &RunReported, cx| {
            pane.check(&state, cx)
        });
        Self {
            state: state.downgrade(),
            reads,
            draw,
            read: None,
            _subscriptions: [notified, ran],
        }
    }

    /// Re-render when the state no longer reads as it did.
    fn check(&mut self, state: &Entity<AppState>, cx: &mut Context<Self>) {
        let read = (self.reads)(state.read(cx), cx);
        if self.read.as_ref() != Some(&read) {
            self.read = Some(read);
            cx.notify();
        }
    }
}

impl<K: PartialEq + 'static> Render for Pane<K> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(state) = self.state.upgrade() else {
            return Empty.into_any_element();
        };
        let (reads, draw) = (self.reads, self.draw);
        let (read, element) =
            state.update(cx, |state, cx| (reads(state, cx), draw(state, window, cx)));
        self.read = Some(read);
        element
    }
}

/// One section's page around the registry's renderer. It re-renders on every
/// notify of the state and on its own section's runs, never on another
/// section's.
pub(crate) struct Page {
    state: WeakEntity<AppState>,
    section: &'static Section,
    _subscriptions: [Subscription; 2],
}

impl Page {
    fn new(state: &Entity<AppState>, section: &'static Section, cx: &mut Context<Self>) -> Self {
        let notified = cx.observe(state, |_, _, cx| cx.notify());
        let ran = cx.subscribe(state, |page: &mut Self, _, run: &RunReported, cx| {
            if run.section == page.section.id {
                cx.notify();
            }
        });
        Self {
            state: state.downgrade(),
            section,
            _subscriptions: [notified, ran],
        }
    }
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(state) = self.state.upgrade() else {
            return Empty.into_any_element();
        };
        let view = self.section.view;
        let content = state.update(cx, |state, cx| view(state, window, cx));
        div()
            .id(format!("section-{}", self.section.id))
            .size_full()
            .p_3()
            .min_h_0()
            .child(content)
            .into_any_element()
    }
}

fn header_reads(state: &AppState, _: &App) -> HeaderReads {
    HeaderReads {
        fold: state.session.shell.sidebar,
        places: state.history.places().to_vec(),
        position: state.history.position(),
        save: state.save_status(),
    }
}

fn draw_header(state: &mut AppState, _: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    header::header(state, cx)
}

fn sidebar_reads(state: &AppState, _: &App) -> SidebarReads {
    let activity = SECTIONS
        .iter()
        .map(|section| state.activity.shown(section.id))
        .collect();
    SidebarReads {
        fold: state.session.shell.sidebar,
        route: state.route().clone(),
        activity,
    }
}

fn draw_sidebar(state: &mut AppState, _: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    sidebar::sidebar(state, cx)
}

fn home_reads(state: &AppState, cx: &App) -> HomeReads {
    HomeReads {
        search: state.search.read(cx).text().to_string(),
        resume: state.last_section,
    }
}

fn draw_home(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("pane-dashboard")
        .size_full()
        .p_6()
        .min_h_0()
        .child(dashboard::dashboard_pane(state, window, cx))
        .into_any_element()
}

fn status_reads(state: &AppState, _: &App) -> StatusReads {
    StatusReads(state.status.clone())
}

/// No status bar is drawn; the last run or import message stays a labelled
/// node for a screen reader and the harnesses.
fn draw_status(state: &mut AppState, _: &mut Window, _: &mut Context<AppState>) -> AnyElement {
    div()
        .id("status-status")
        .role(Role::Label)
        .aria_label(state.status.clone())
        .size_0()
        .overflow_hidden()
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::headless;
    use std::cell::Cell;
    use std::rc::Rc;

    /// How often `view` is notified, which is how often it re-renders.
    fn count_notifies<V: 'static>(
        view: &Entity<V>,
        cx: &mut gpui::TestAppContext,
    ) -> (Rc<Cell<usize>>, gpui::Subscription) {
        let count = Rc::new(Cell::new(0));
        let counted = count.clone();
        let subscription =
            cx.update(|cx| cx.observe(view, move |_, _| counted.set(counted.get() + 1)));
        (count, subscription)
    }

    #[test]
    fn a_run_redraws_its_page_and_the_sidebar_but_not_home_or_another_page() {
        let dir = std::env::temp_dir().join(format!("idleon-views-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/data.json");
        let mut cx = gpui::TestAppContext::single();
        let window = headless(&mut cx, &dir);
        for id in ["farming", "sailing"] {
            window
                .update(&mut cx, |state, window, cx| {
                    if id == "farming" {
                        state.import_file(&fixture, window, cx);
                    }
                    state.open_place(Route::open(id), window, cx);
                })
                .expect("the window is open");
            cx.run_until_parked();
        }
        let (home, sidebar, farming, sailing) = window
            .update(&mut cx, |state, _, cx| {
                let entity = cx.entity();
                let views = state.views.as_mut().expect("the shell drew its views");
                let page = |id| crate::sections::find(id).expect("a registry id");
                (
                    views.home.clone(),
                    views.sidebar.clone(),
                    views.page(page("farming"), &entity, cx),
                    views.page(page("sailing"), &entity, cx),
                )
            })
            .expect("the window is open");
        // Starting is a click on Farming's page, which redraws what it may;
        // only the run that follows is counted.
        window
            .update(&mut cx, |state, _, cx| state.run_farming(cx))
            .expect("the window is open");
        let (home_count, _home) = count_notifies(&home, &mut cx);
        let (sailing_count, _sailing) = count_notifies(&sailing, &mut cx);
        let (sidebar_count, _sidebar) = count_notifies(&sidebar, &mut cx);
        let (farming_count, _farming) = count_notifies(&farming, &mut cx);

        cx.run_until_parked();
        window
            .update(&mut cx, |state, _, cx| {
                assert!(
                    state.farming_run.read(cx).result().is_some(),
                    "{}",
                    state.status
                );
            })
            .expect("the window is open");
        assert_eq!(home_count.get(), 0, "Home did not redraw for Farming's run");
        assert_eq!(
            sailing_count.get(),
            0,
            "Sailing did not redraw for Farming's run"
        );
        assert!(sidebar_count.get() > 0, "the sidebar redrew Farming's mark");
        assert!(farming_count.get() > 0, "Farming's page redrew its run");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
