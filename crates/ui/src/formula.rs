//! Shared expandable formula tree used by every calculator's breakdown card.
//!
//! The tree owns its search field, open branches and copy state. A section
//! builds a [`FormulaNode`], hands it to [`FormulaTree::set_root`] and
//! renders the entity; every calculator gets the same rows, search highlights
//! and copy text from here. Every element id carries the tree's own id, so two
//! trees can share a page (the All Stats breakdown and formula cards do) without
//! their rows answering to the same id.
//!
//! The visible rows are laid out when the root, the search or an open branch
//! changes, not on every frame: a parent that redraws each frame only maps
//! the cached rows to elements.

use std::collections::HashSet;
use std::f32::consts::FRAC_PI_2;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, Hsla, IntoElement, ParentElement, Render,
    Role, SharedString, Styled, Subscription, Window, div, px, radians,
};
use gpui_kit::component::button::Button;
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, Size};
use idleon_common::FormulaNode;
use idleon_common::ValueFormat;

use crate::widgets::{TEXT_CAPTION, TEXT_MONO, search_input};

/// One calculator's formula tree: its data, its search and its open branches.
pub struct FormulaTree {
    id: SharedString,
    root: Option<FormulaNode>,
    search: Entity<InputState>,
    /// The trimmed search text the rows were laid out for.
    query: String,
    open: HashSet<String>,
    /// The visible rows, root first, for the current root, query and open branches.
    rows: Vec<Row>,
    /// Matching nodes while searching, every node otherwise.
    count: usize,
    _subscription: Subscription,
}

/// One visible node, with everything its row draws already formatted.
struct Row {
    /// The node's path from the root (`root/0/2`), the key of `open`.
    key: SharedString,
    /// The same path with dashes, as the element ids spell it.
    path: SharedString,
    depth: usize,
    has_children: bool,
    open: bool,
    matched: bool,
    label: SharedString,
    value: SharedString,
    format: ValueFormat,
    details: Option<SharedString>,
}

impl FormulaTree {
    /// A tree under `id`; sections pass a stable id such as `"formula"`.
    pub fn new(id: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search formula nodes…"));
        let subscription = cx.subscribe(&search, |tree, search, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                tree.query = search.read(cx).value().trim().to_string();
                tree.layout();
                cx.notify();
            }
        });
        Self {
            id: id.into(),
            root: None,
            search,
            query: String::new(),
            open: HashSet::from(["root".to_string()]),
            rows: Vec::new(),
            count: 0,
            _subscription: subscription,
        }
    }

    /// Replaces the tree's data, keeping the open branches and the search.
    pub fn set_root(&mut self, root: FormulaNode, cx: &mut Context<Self>) {
        if self.root.as_ref() != Some(&root) {
            self.root = Some(root);
            self.layout();
            cx.notify();
        }
    }

    /// Lays out the visible rows again; called whenever what they show changes.
    fn layout(&mut self) {
        self.rows.clear();
        let Some(root) = &self.root else {
            self.count = 0;
            return;
        };
        let searching = !self.query.is_empty();
        self.count = if searching {
            root.match_count(&self.query)
        } else {
            root.node_count()
        };
        push_rows(
            &mut self.rows,
            root,
            "root".to_string(),
            0,
            searching,
            &self.query,
            &self.open,
        );
    }

    fn toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.open.remove(key) {
            self.open.insert(key.to_string());
        }
        self.layout();
        cx.notify();
    }

    fn set_open(&mut self, open: HashSet<String>, cx: &mut Context<Self>) {
        self.open = open;
        self.layout();
        cx.notify();
    }
}

/// Appends `node`'s row under `key`, then its children's when it is open.
fn push_rows(
    rows: &mut Vec<Row>,
    node: &FormulaNode,
    key: String,
    depth: usize,
    searching: bool,
    query: &str,
    open: &HashSet<String>,
) {
    let has_children = !node.children.is_empty();
    let is_open = has_children && (searching || open.contains(&key));

    let mut details = Vec::new();
    if let Some(input) = &node.input {
        details.push(format!("input {input}"));
    }
    if let Some(multiplier) = node.multiplier {
        details.push(format!("×{}", ValueFormat::Raw.format(multiplier)));
    }
    if let Some(source) = &node.source {
        details.push(source.clone());
    }
    if let Some(note) = &node.note {
        details.push(note.clone());
    }

    rows.push(Row {
        path: key.replace('/', "-").into(),
        key: key.clone().into(),
        depth,
        has_children,
        open: is_open,
        matched: node.matches(query),
        label: node.label.clone().into(),
        value: node.format.format(node.value).into(),
        format: node.format,
        details: (!details.is_empty()).then(|| details.join(" · ").into()),
    });
    if is_open {
        for (index, child) in node.children.iter().enumerate() {
            push_rows(
                rows,
                child,
                format!("{key}/{index}"),
                depth + 1,
                searching,
                query,
                open,
            );
        }
    }
}

impl Render for FormulaTree {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.root.is_none() {
            return div().into_any_element();
        }
        let entity = cx.entity();
        let id = self.id.clone();
        let searching = !self.query.is_empty();
        let count = self.count;
        let search_focus = self.search.focus_handle(cx);

        let expand = entity.clone();
        let collapse = entity.clone();
        let copy = entity.clone();

        let controls = div()
            .id(format!("{id}-controls"))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .pb_2()
            .child(
                div()
                    .id(format!("{id}-search"))
                    .role(Role::Search)
                    .aria_label("Search formula nodes")
                    .w_full()
                    .on_click(move |_event, window, cx| {
                        window.focus(&search_focus, cx);
                    })
                    .child(
                        search_input(&self.search)
                            .id(format!("{id}-search-field"))
                            .aria_label("Search formula nodes")
                            .small(),
                    ),
            )
            .child(
                div()
                    .id(format!("{id}-actions"))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id(format!("{id}-match-count"))
                            .role(Role::Label)
                            .aria_label(if searching {
                                format!("{count} matching formula nodes")
                            } else {
                                format!("{count} formula nodes")
                            })
                            .text_size(TEXT_CAPTION)
                            .text_color(cx.theme().muted_foreground)
                            .child(if searching {
                                format!("{count} matching")
                            } else {
                                format!("{count} nodes")
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(format!("{id}-expand"))
                            .label("Expand")
                            .small()
                            .on_click(move |_event, _window, cx| {
                                expand.update(cx, |tree, cx| {
                                    let keys = tree.root.as_ref().map(FormulaNode::keys);
                                    tree.set_open(keys.into_iter().flatten().collect(), cx);
                                });
                            }),
                    )
                    .child(
                        Button::new(format!("{id}-collapse"))
                            .label("Collapse")
                            .small()
                            .on_click(move |_event, _window, cx| {
                                collapse.update(cx, |tree, cx| tree.set_open(HashSet::new(), cx));
                            }),
                    )
                    .child(
                        Clipboard::new(format!("{id}-copy"))
                            .tooltip("Copy formula")
                            .small()
                            .value_fn(move |_window, cx| {
                                copy.read(cx)
                                    .root
                                    .as_ref()
                                    .map(|root| root.copy_text().into())
                                    .unwrap_or_default()
                            }),
                    ),
            );

        let rows = div().flex().flex_col().p_2().children(
            self.rows
                .iter()
                .map(|row| row_element(&id, row, &entity, cx)),
        );

        div()
            .id(format!("{id}-tree"))
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(controls)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(rows)
                    .overflow_y_scrollbar()
                    .id(format!("{id}-scroll")),
            )
            .into_any_element()
    }
}

fn row_element(id: &str, row: &Row, entity: &Entity<FormulaTree>, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let toggle = entity.clone();
    let toggle_key = row.key.clone();

    let disclosure: AnyElement = if row.has_children {
        div()
            .id(format!("{id}-disclosure-{}", row.path))
            .flex()
            .flex_none()
            .child(
                Icon::new(IconName::ChevronRight)
                    .with_size(Size::XSmall)
                    .text_color(theme.muted_foreground)
                    .rotate(radians(if row.open { FRAC_PI_2 } else { 0.0 })),
            )
            .into_any_element()
    } else {
        div().w(px(12.0)).flex_none().into_any_element()
    };

    div()
        .id(format!("{id}-node-{}", row.path))
        .role(if row.has_children {
            Role::Button
        } else {
            Role::Label
        })
        .aria_label(format!("{}: {}", row.label, row.value))
        .flex()
        .flex_col()
        .gap_0p5()
        .py_1()
        .pr_1()
        .pl(px(4.0 + row.depth as f32 * 12.0))
        .rounded(theme.radius)
        .when(row.matched, |el| el.bg(theme.list_active))
        .hover(|style| style.bg(theme.list_hover))
        .when(row.has_children, |el| {
            el.cursor_pointer().on_click(move |_event, _window, cx| {
                toggle.update(cx, |tree, cx| tree.toggle(&toggle_key, cx));
            })
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(disclosure)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(TEXT_MONO)
                        .when(row.matched, |label| label.text_color(theme.primary))
                        .child(row.label.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .justify_end()
                        .min_w(px(58.0))
                        .text_size(TEXT_MONO)
                        .text_color(value_color(row.format, cx))
                        .child(row.value.clone()),
                ),
        )
        .when_some(row.details.clone(), |el, details| {
            el.child(
                div()
                    .text_size(px(10.0))
                    .line_height(px(13.0))
                    .text_color(theme.table_head_foreground)
                    .line_clamp(2)
                    .child(details),
            )
        })
        .into_any_element()
}

fn value_color(format: ValueFormat, cx: &App) -> Hsla {
    let theme = cx.theme();
    match format {
        ValueFormat::Rate => theme.success,
        ValueFormat::Percent => theme.primary,
        ValueFormat::Multiplier => theme.info,
        ValueFormat::Count | ValueFormat::Raw => theme.foreground,
    }
}

#[cfg(test)]
mod tests {
    use super::FormulaTree;
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use idleon_common::FormulaNode;

    fn sample() -> FormulaNode {
        FormulaNode::new("Total", 6.0).with_children([
            FormulaNode::new("Base", 2.0).with_children([FormulaNode::new("Takin' Notes", 2.0)]),
            FormulaNode::new("Bonus", 4.0).with_source("Grid"),
        ])
    }

    fn open(cx: &mut TestAppContext) -> (Entity<FormulaTree>, &mut VisualTestContext) {
        cx.update(gpui_kit::init);
        let (tree, cx) = cx.add_window_view(|window, cx| FormulaTree::new("formula", window, cx));
        tree.update(cx, |tree, cx| tree.set_root(sample(), cx));
        cx.run_until_parked();
        (tree, cx)
    }

    fn paths(tree: &Entity<FormulaTree>, cx: &mut VisualTestContext) -> Vec<String> {
        tree.read_with(cx, |tree, _| {
            tree.rows.iter().map(|row| row.path.to_string()).collect()
        })
    }

    #[gpui::test]
    fn the_root_opens_and_branches_toggle(cx: &mut TestAppContext) {
        let (tree, cx) = open(cx);
        assert_eq!(paths(&tree, cx), ["root", "root-0", "root-1"]);
        assert_eq!(tree.read_with(cx, |tree, _| tree.count), 4);

        tree.update(cx, |tree, cx| tree.toggle("root/0", cx));
        assert_eq!(paths(&tree, cx), ["root", "root-0", "root-0-0", "root-1"]);

        tree.update(cx, |tree, cx| tree.set_open(Default::default(), cx));
        assert_eq!(paths(&tree, cx), ["root"]);
    }

    #[gpui::test]
    fn searching_opens_every_branch_and_counts_matches(cx: &mut TestAppContext) {
        let (tree, cx) = open(cx);
        tree.update(cx, |tree, cx| tree.set_open(Default::default(), cx));
        let search = tree.read_with(cx, |tree, _| tree.search.clone());
        cx.update(|window, cx| search.update(cx, |state, cx| state.focus(window, cx)));
        cx.simulate_input("takin");
        cx.run_until_parked();

        assert_eq!(paths(&tree, cx), ["root", "root-0", "root-0-0", "root-1"]);
        tree.read_with(cx, |tree, _| {
            assert_eq!(tree.count, 1);
            let matched: Vec<_> = tree.rows.iter().filter(|row| row.matched).collect();
            assert_eq!(matched.len(), 1);
            assert_eq!(matched[0].label.as_ref(), "Takin' Notes");
        });
    }
}
