//! The Jelly Operator section: the overview and the layout editor.
//!
//! JO-01 ports the upstream page's Overview and Layout tabs: the save's board
//! with every cell type placeable, the account snapshot and the seeded operation
//! estimate with its outlook. The simulator (JO-02), upgrades and breakdowns
//! (JO-03), section planner (JO-04) and coordinator optimizer (JO-05) are the
//! later features on the same section and are not rendered here.
//!
//! The maths lives in `idleon_core::jelly`; this module owns the board on
//! screen and the shared task runner's inputs.

mod estimate;
mod grid;
mod layout_editor;
mod outlook;
mod overview;

use crate::state::AppState;
use crate::ui::theme;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::ControlSize;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use idleon_core::jelly::Jelly;
use idleon_core::jelly::board::{self, Layout};

/// The upstream tabs this issue builds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    #[default]
    Overview,
    Layout,
}

impl Tab {
    pub fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Layout => "layout",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Layout => "Layout",
        }
    }
}

/// The inputs one estimate run reads, so a later board edit marks it stale.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunInputs {
    pub jelly: Option<Jelly>,
    pub layout: Layout,
    pub trials: u32,
    pub seed: u32,
}

/// The section's state: the board on screen, the palette pick and the run knobs.
#[derive(Clone, Debug)]
pub struct State {
    pub tab: Tab,
    /// The board on screen, which starts as the saved layout.
    pub layout: Layout,
    /// The palette cell type the next slot click places.
    pub selected: usize,
    /// The last placement answer, shown under the board.
    pub message: String,
    pub trials: u32,
    pub seed: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            layout: Layout::new(),
            selected: 0,
            message: String::new(),
            trials: 32,
            seed: 1,
        }
    }
}

impl State {
    /// Take a newly imported save's board.
    pub fn adopt(&mut self, jelly: &Jelly) {
        self.layout = jelly.layout.clone();
        self.selected = 0;
        self.message = String::new();
    }

    /// Put the board back to the save's own layout.
    pub fn reset(&mut self, jelly: &Jelly) {
        self.layout = jelly.layout.clone();
        self.message = "Board reset to the saved layout.".to_string();
    }

    /// Pick a palette cell type.
    pub fn select(&mut self, cell_type: usize) {
        self.selected = cell_type;
        self.message = String::new();
    }

    /// Place the picked cell at `anchor`, or explain why it does not fit.
    pub fn place(&mut self, jelly: &Jelly, anchor: usize) {
        match board::place(jelly, &self.layout, anchor, self.selected) {
            Ok(next) => {
                self.layout = next;
                self.message = format!(
                    "Placed {} at slot {anchor}.",
                    idleon_core::jelly::data().cell_name(self.selected)
                );
            }
            Err(reason) => self.message = reason,
        }
    }

    /// Remove whatever cell covers `anchor`.
    pub fn remove_at(&mut self, layout: &Layout, anchor: usize) {
        let Some((owner, _)) = board::occupancy(layout).get(&anchor).copied() else {
            self.message = format!("Slot {anchor} is empty.");
            return;
        };
        self.layout = board::remove(&self.layout, owner);
        self.message = format!("Removed the cell at slot {owner}.");
    }

    /// The inputs a run started now would use.
    pub fn inputs(&self, jelly: Option<&Jelly>) -> RunInputs {
        RunInputs {
            jelly: jelly.cloned(),
            layout: self.layout.clone(),
            trials: self.trials,
            seed: self.seed,
        }
    }
}

/// The section view: the tab strip and the selected tab.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let Some(jelly) = state.jelly.clone() else {
        return unavailable(state, cx);
    };
    let entity = cx.entity();
    let active = state.jelly_state.tab;
    let tabs = div().id("jelly-tabs").flex().flex_none().gap_2().children(
        [Tab::Overview, Tab::Layout].map(|tab| {
            let entity = entity.clone();
            Button::new(format!("jelly-tab-{}", tab.id()), tab.label())
                .variant(if tab == active {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    let entity = entity.clone();
                    entity.update(cx, |state, cx| state.set_jelly_tab(tab, cx));
                })
                .into_any_element()
        }),
    );
    let body = match state.jelly_state.tab {
        Tab::Overview => overview::overview(state, window, cx, &jelly),
        Tab::Layout => layout_editor::layout_editor(state, window, cx, &jelly),
    };
    div()
        .id("jelly-operator")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(tabs)
        .child(
            ScrollArea::new("jelly-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(body),
        )
        .into_any_element()
}

/// No save carries a Jelly Operator board yet.
fn unavailable(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    let empty = EmptyState::new(
        "jelly-unavailable",
        IconName::Map,
        "No Jelly Operator board",
    )
    .body(
        "This save does not carry Research[14] and Research[17], so there is no board to read. Export a save after the Jelly Operator update and import it again.",
    )
    .action(
        Button::new("jelly-import", "Import save")
            .icon(IconName::ArrowLeft)
            .on_click(move |_event, window, cx| {
                import.update(cx, |state, cx| state.open_import(window, cx));
            }),
    );
    div()
        .id("jelly-unavailable-body")
        .flex()
        .flex_col()
        .size_full()
        .gap_3()
        .child(
        Card::new()
            .header(
                CardHeader::new("Jelly Operator board")
                    .description("Cell layouts, obstruction clears, Bloodcells and the operation estimate")
                    .action(theme::WorldBadge::new(state.current_section().map_or(
                        crate::sections::Group::W7,
                        |section| section.group,
                    ))),
            )
            .child(empty),
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{RunInputs, State, Tab};
    use idleon_core::jelly::board;

    /// A flat save with the eight cell unlocks and two Plasmids on the board.
    fn save() -> String {
        let mut research: Vec<Vec<f64>> = vec![Vec::new(); 19];
        research[7] = vec![0.0; 15];
        research[7][9] = 0.0;
        research[7][10] = 3.0;
        research[14] = vec![-1.0; 180];
        research[14][77] = 1.0;
        research[14][95] = 1.0;
        research[15] = vec![0.0; 9];
        research[16] = vec![0.0; 9];
        research[17] = vec![0.0; 40];
        research[17][..8].fill(1.0);
        // Every plot bought, so a 4×4 Gigacyst has somewhere to go.
        research[18] = (0..40).map(f64::from).collect();
        format!(
            "{{\"Research\":{}, \"Lv0_0\":{}}}",
            serde_json::to_string(&research).unwrap(),
            serde_json::to_string(&vec![0.0; 30]).unwrap()
        )
    }

    #[test]
    fn a_saved_layout_loads_and_every_cell_type_is_placeable() {
        let imported = idleon_core::save::import(&save()).expect("the save imports");
        let jelly = imported.jelly.expect("the save carries a board");
        let mut state = State::default();
        state.adopt(&jelly);
        assert_eq!(state.layout, jelly.layout, "the save layout loads");
        assert_eq!(jelly.units_owned(), 8);

        let mut layout = state.layout.clone();
        for cell_type in 0..8 {
            let occupied = board::occupancy(&layout);
            let anchor = (0..180)
                .find(|anchor| {
                    board::footprint(*anchor, cell_type)
                        .is_some_and(|slots| slots.iter().all(|slot| !occupied.contains_key(slot)))
                        && board::place(&jelly, &layout, *anchor, cell_type).is_ok()
                })
                .unwrap_or_else(|| panic!("no free slot for cell type {cell_type}"));
            state.select(cell_type);
            state.layout = layout.clone();
            state.place(&jelly, anchor);
            assert!(
                state.message.starts_with("Placed"),
                "cell type {cell_type} is refused: {}",
                state.message
            );
            layout = state.layout.clone();
        }
        let types: std::collections::BTreeSet<usize> = layout.values().copied().collect();
        assert_eq!(
            types,
            (0..8).collect::<std::collections::BTreeSet<usize>>(),
            "the palette placed every cell type"
        );
        board::validate(&jelly, &layout).expect("the edited board stays valid");

        // A click on a placed slot removes the cell that covers it.
        state.remove_at(&layout, 77);
        assert!(!state.layout.contains_key(&77));

        state.reset(&jelly);
        assert_eq!(state.layout, jelly.layout);
        assert_eq!(state.tab, Tab::Overview);
    }

    #[test]
    fn a_refused_placement_names_the_reason() {
        let imported = idleon_core::save::import(&save()).expect("the save imports");
        let jelly = imported.jelly.expect("the save carries a board");
        let mut state = State::default();
        state.adopt(&jelly);
        state.select(7);
        state.place(&jelly, 100);
        assert!(
            state.message.contains("locked") || state.message.contains("Slot"),
            "a locked slot explains itself: {}",
            state.message
        );
        assert_eq!(
            state.layout, jelly.layout,
            "a refused place changes nothing"
        );
    }

    #[test]
    fn the_runner_inputs_follow_the_board_and_the_knobs() {
        let imported = idleon_core::save::import(&save()).expect("the save imports");
        let jelly = imported.jelly.expect("the save carries a board");
        let mut state = State::default();
        state.adopt(&jelly);
        let first = state.inputs(Some(&jelly));
        state.select(0);
        state.place(&jelly, 96);
        let second = state.inputs(Some(&jelly));
        assert_ne!(
            first, second,
            "an edited board marks a stored estimate stale"
        );
        assert_eq!(second.trials, 32);
        state.trials = 8;
        state.seed = 3;
        assert_eq!(
            state.inputs(Some(&jelly)),
            RunInputs {
                jelly: Some(jelly.clone()),
                layout: state.layout.clone(),
                trials: 8,
                seed: 3,
            }
        );
    }
}
