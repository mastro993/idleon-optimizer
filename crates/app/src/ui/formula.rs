//! Shared expandable formula tree used by every calculator's breakdown card.
//!
//! The tree owns its search field, open branches and copy state. A section
//! builds a [`FormulaNode`] in core, hands it to [`FormulaTree::set_root`] and
//! renders the entity; every calculator gets the same rows, search highlights
//! and copy text from here.

use std::collections::HashSet;

use ely_gpui_component::buttons::{Button, CopyButton};
use ely_gpui_component::forms::{InputEvent, SearchInput, TextInput};
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::Disclosure;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, Palette};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, Hsla, IntoElement, ParentElement, Render, Role, Styled,
    Subscription, Window, div, px,
};
use idleon_core::formula::{FormulaNode, ValueFormat};

/// One calculator's formula tree: its data, its search and its open branches.
pub struct FormulaTree {
    id: String,
    root: Option<FormulaNode>,
    search: Entity<TextInput>,
    open: HashSet<String>,
    _subscription: Subscription,
}

impl FormulaTree {
    /// A tree under `id`; sections pass a stable id such as `"formula"`.
    pub fn new(id: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Search formula nodes…")
                .label("Search formula nodes")
        });
        let subscription = cx.subscribe(&search, |_tree, _search, event: &InputEvent, cx| {
            if *event == InputEvent::Changed {
                cx.notify();
            }
        });
        Self {
            id: id.into(),
            root: None,
            search,
            open: HashSet::from(["root".to_string()]),
            _subscription: subscription,
        }
    }

    /// Replaces the tree's data, keeping the open branches and the search.
    pub fn set_root(&mut self, root: FormulaNode, cx: &mut Context<Self>) {
        if self.root.as_ref() != Some(&root) {
            self.root = Some(root);
            cx.notify();
        }
    }
}

impl Render for FormulaTree {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(root) = self.root.as_ref() else {
            return div().into_any_element();
        };
        let query = self.search.read(cx).text().trim().to_string();
        let searching = !query.is_empty();
        let matches = root.match_count(&query);
        let colors = cx.theme().colors.clone();
        let entity = cx.entity();
        let id = self.id.clone();
        let search_focus = self.search.read(cx).focus().clone();

        let expand = entity.clone();
        let expand_keys = root.keys();
        let collapse = entity.clone();
        let copy = root.copy_text();

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
                        SearchInput::new(format!("{id}-search-field"), &self.search)
                            .size(ControlSize::Sm),
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
                                format!("{matches} matching formula nodes")
                            } else {
                                format!("{} formula nodes", root.node_count())
                            })
                            .text_size(px(11.0))
                            .text_color(colors.fg_muted)
                            .child(if searching {
                                format!("{matches} matching")
                            } else {
                                format!("{} nodes", root.node_count())
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new(format!("{id}-expand"), "Expand")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                let keys = expand_keys.clone();
                                expand.update(cx, |tree, cx| {
                                    tree.open = keys.into_iter().collect();
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new(format!("{id}-collapse"), "Collapse")
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                collapse.update(cx, |tree, cx| {
                                    tree.open.clear();
                                    cx.notify();
                                });
                            }),
                    )
                    .child(CopyButton::new(format!("{id}-copy"), copy)),
            );

        let mut rows = div().flex().flex_col().p_2();
        rows = rows.child(node_tree(
            root, "root", 0, searching, &query, &self.open, &entity, &colors,
        ));

        div()
            .id(format!("{id}-tree"))
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(controls)
            .child(
                ScrollArea::new(format!("{id}-scroll"))
                    .vertical()
                    .shadows()
                    .flex_1()
                    .min_h_0()
                    .child(rows),
            )
            .into_any_element()
    }
}

#[allow(clippy::too_many_arguments)]
fn node_tree(
    node: &FormulaNode,
    key: &str,
    depth: usize,
    searching: bool,
    query: &str,
    open: &HashSet<String>,
    entity: &Entity<FormulaTree>,
    colors: &Palette,
) -> AnyElement {
    let has_children = !node.children.is_empty();
    let is_open = has_children && (searching || open.contains(key));
    let matched = node.matches(query);
    let path = key.replace('/', "-");
    let label = node.format.format(node.value);
    let toggle = entity.clone();
    let toggle_key = key.to_string();

    let mut row = div()
        .id(format!("formula-node-{path}"))
        .role(if has_children {
            Role::Button
        } else {
            Role::Label
        })
        .aria_label(format!("{}: {label}", node.label))
        .flex()
        .flex_col()
        .gap_0p5()
        .py_1()
        .pr_1()
        .pl(px(4.0 + depth as f32 * 12.0))
        .rounded(px(4.0))
        .when(matched, |row| row.bg(colors.selection))
        .hover(|style| style.bg(colors.hover))
        .when(has_children, |row| {
            row.cursor_pointer().on_click(move |_event, _window, cx| {
                toggle.update(cx, |tree, cx| {
                    if !tree.open.remove(&toggle_key) {
                        tree.open.insert(toggle_key.clone());
                    }
                    cx.notify();
                });
            })
        });

    let disclosure: AnyElement = if has_children {
        Disclosure::new(format!("formula-disclosure-{path}"), is_open).into_any_element()
    } else {
        div().w(px(14.0)).into_any_element()
    };

    row = row.child(
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(disclosure)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.0))
                    .when(matched, |label| label.text_color(colors.accent))
                    .child(node.label.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .justify_end()
                    .min_w(px(58.0))
                    .text_size(px(12.0))
                    .text_color(value_color(node.format, colors))
                    .child(label),
            ),
    );

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
    if !details.is_empty() {
        row = row.child(
            div()
                .text_size(px(10.0))
                .line_height(px(13.0))
                .text_color(colors.fg_subtle)
                .line_clamp(2)
                .child(details.join(" · ")),
        );
    }

    let mut tree = div().flex().flex_col().child(row);
    if is_open {
        for (index, child) in node.children.iter().enumerate() {
            let child_key = format!("{key}/{index}");
            tree = tree.child(node_tree(
                child,
                &child_key,
                depth + 1,
                searching,
                query,
                open,
                entity,
                colors,
            ));
        }
    }
    tree.into_any_element()
}

fn value_color(format: ValueFormat, colors: &Palette) -> Hsla {
    match format {
        ValueFormat::Rate => colors.success,
        ValueFormat::Percent => colors.accent,
        ValueFormat::Multiplier => colors.info,
        ValueFormat::Count | ValueFormat::Raw => colors.fg,
    }
}
