//! The All Stats Dashboard: the five calculation-context selectors, the map
//! access summary they resolve against, the category breakdowns of SA-02, the
//! formula explorer of SA-03 and the game-input sandbox of SA-04.
//!
//! A context change re-resolves every descriptor, so the page never shows a
//! value from the previous character or map. The breakdown rows and the
//! explorer read the descriptor model in `idleon_stats::breakdowns`; the
//! explorer is [`explorer`], and both formula trees are the shared
//! [`FormulaTree`](idleon_ui::formula::FormulaTree) component, not a second
//! renderer. The sandbox keeps the imported save beside a set of pending edits:
//! the rows above resolve against the edited save, the baseline sheet keeps the
//! saved values, and the ledger lists the inputs and the formulas the two
//! disagree on.
//!
//! Resolving every descriptor takes hundreds of milliseconds, so the sheets
//! are built off the UI thread ([`AppState::resolve_stats`]) and the page
//! shows a skeleton until they land.

mod actions;
mod breakdowns;
mod controls;
mod explorer;
mod sandbox;

pub(crate) use controls::Controls;

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Task, Window, div, px};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::searchable_list::SearchableListDelegate;
use gpui_kit::component::select::Select;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_research::model::{SimContext, Simulation};
use idleon_save::Save;
use idleon_stats::{self as stats, BreakdownCategory, Context as StatsContext};
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, ChoiceSelect, PANEL_GAP, TEXT_CAPTION, Tone,
};

/// The section's own state: the context, the save slice and the breakdowns.
pub(crate) struct State {
    pub(crate) context: StatsContext,
    pub(crate) snapshot: Option<stats::Snapshot>,
    /// The sheet the imported save resolves to: the baseline the ledger
    /// compares a hypothetical edit against.
    pub(crate) saved_sheet: Option<stats::Sheet>,
    /// Every descriptor resolved against the save, the context and the
    /// pending edits.
    pub(crate) sheet: Option<stats::Sheet>,
    /// Bumped whenever new sheets land, so the formula trees know their root
    /// may have moved.
    pub(crate) revision: u64,
    /// The editable game inputs and the edits pending on them.
    pub(crate) sandbox: Option<stats::Sandbox>,
    /// The descriptors the pending edits moved, in registry order.
    pub(crate) affected: Vec<stats::Affected>,
    /// Why the last input edit was refused, when one was.
    pub(crate) input_error: String,
    /// The category tab on screen.
    pub(crate) category: BreakdownCategory,
    /// The descriptor whose formula tree the detail card shows.
    pub(crate) selected: Option<String>,
    /// The formula explorer's filters and its own selection.
    pub(crate) explorer: explorer::State,
    /// The map selector's rows for the current character and list level.
    pub(crate) options: Vec<stats::MapOption>,
    characters: Vec<Choice<usize>>,
    map_choices: Vec<Choice<usize>>,
    /// The save, the context or an edit moved under the sheets. Resolving
    /// every descriptor is the expensive half of an import, so it waits for
    /// the page instead of holding up the window that imported the save.
    dirty: bool,
    /// Sheets are being built off the UI thread.
    building: bool,
    /// The build in flight; replacing it drops a stale one.
    pending: Option<Task<()>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            context: StatsContext::default(),
            snapshot: None,
            saved_sheet: None,
            sheet: None,
            revision: 0,
            sandbox: None,
            affected: Vec::new(),
            input_error: String::new(),
            category: BreakdownCategory::Combat,
            selected: None,
            explorer: explorer::State::default(),
            options: Vec::new(),
            characters: Vec::new(),
            map_choices: Vec::new(),
            dirty: false,
            building: false,
            pending: None,
        }
    }
}

/// The two sheets one build resolves: the baseline and the one on screen.
type Sheets = (stats::Sheet, stats::Sheet);

impl State {
    /// Take a freshly imported save, opening on character one and its current
    /// map. The other selectors keep the user's choice, and the sheet resolves
    /// when the page opens.
    pub(crate) fn load(&mut self, save: &Save) {
        let snapshot = stats::Snapshot::from_save(save);
        self.context.character = 0;
        self.context.map = snapshot.current_map(0).unwrap_or(0);
        self.characters = controls::character_choices(&snapshot);
        self.snapshot = Some(snapshot);
        // A new save starts with no hypothetical edits.
        self.sandbox = Some(stats::Sandbox::new(save, self.context.character));
        self.input_error.clear();
        self.clamp(&mut Vec::new());
        self.invalidate(true);
    }

    /// The sheets no longer describe the context. A context change drops them,
    /// so the page never shows the previous character's or map's values; a
    /// sandbox edit keeps them on screen until the edited ones land.
    pub(crate) fn invalidate(&mut self, drop_sheets: bool) {
        if drop_sheets {
            self.sheet = None;
            self.saved_sheet = None;
            self.affected.clear();
        }
        self.pending = None;
        self.dirty = true;
    }

    /// The build the save, the context and the pending edits need, ready to
    /// run off the UI thread; `None` when nothing moved.
    ///
    /// The baseline resolves the imported save; the sheet on screen resolves
    /// the sandbox's current save, so every value, the explorer and the ledger
    /// agree on which hypothetical state they describe.
    fn job(
        &mut self,
        save: &Save,
        simulation: &Simulation,
        sim_context: &SimContext,
    ) -> Option<impl FnOnce() -> Sheets + Send + 'static> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        self.building = true;
        let context = self.context;
        if let Some(sandbox) = self.sandbox.as_mut() {
            sandbox.adopt(save, context.character);
        }
        let edited = self
            .sandbox
            .as_ref()
            .filter(|sandbox| sandbox.count() > 0)
            .map(stats::Sandbox::current);
        let (save, simulation, sim_context) =
            (save.clone(), simulation.clone(), sim_context.clone());
        Some(move || {
            let baseline =
                stats::Sheet::build(&breakdown_inputs(&save, &simulation, &sim_context, context));
            let sheet = match edited {
                Some(edited) => stats::Sheet::build(&breakdown_inputs(
                    &edited,
                    &simulation,
                    &sim_context,
                    context,
                )),
                None => baseline.clone(),
            };
            (baseline, sheet)
        })
    }

    /// Take the sheets a build resolved. A selection the new sheet still
    /// carries is kept, otherwise the first row of the current tab is selected.
    fn adopt(&mut self, (baseline, sheet): Sheets) {
        self.building = false;
        self.revision += 1;
        self.affected = sheet.affected(&baseline);
        let selection = self.selected.take().filter(|id| sheet.get(id).is_some());
        self.selected = selection;
        self.saved_sheet = Some(baseline);
        self.sheet = Some(sheet);
        if self.selected.is_none() {
            self.select_first();
        }
    }

    /// Select the first descriptor of the current category.
    pub(crate) fn select_first(&mut self) {
        let Some(sheet) = self.sheet.as_ref() else {
            return;
        };
        self.selected = sheet
            .in_category(self.category)
            .next()
            .map(|row| row.id.to_string());
    }

    /// The descriptor the detail card shows.
    pub(crate) fn selected_row(&self) -> Option<&stats::Breakdown> {
        let sheet = self.sheet.as_ref()?;
        sheet.get(self.selected.as_deref()?)
    }

    /// Read the map selector's rows again, after the character or the list
    /// level moved.
    pub(crate) fn refresh_options(&mut self) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        self.options = self.context.map_options(snapshot);
        self.map_choices = controls::map_choices(snapshot, &self.options);
    }

    /// Keep the context inside the save and the list level; names the dropped
    /// map in `warnings`, for a project file restoring an unavailable one.
    pub(crate) fn clamp(&mut self, warnings: &mut Vec<String>) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        if self.context.character >= snapshot.characters() {
            warnings.push(format!(
                "stats-all: character {} is outside the {} the save carries; character 1 is used",
                self.context.character + 1,
                snapshot.characters()
            ));
            self.context.character = 0;
        }
        if let Some(dropped) = self.context.clamp_map(snapshot) {
            warnings.push(format!(
                "stats-all: map {dropped} is not in the {} map list; map {} is used",
                self.context.map_visibility.label(),
                self.context.map
            ));
        }
        self.refresh_options();
    }
}

/// The All Stats Dashboard page: the context card, the map access summary,
/// the sandbox, the breakdowns and the explorer.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    state.resolve_stats(None, cx);
    state.stats_controls.sync(&state.stats_all, window, cx);
    let muted = cx.theme().muted_foreground;
    let stats = &state.stats_all;
    let context = stats.context;
    let snapshot = stats.snapshot.as_ref();
    let note = match snapshot {
        None => "Load a save to calculate all descriptors.".to_string(),
        Some(snapshot) => {
            let class = stats::classify(context.map);
            let name = snapshot.name(context.character);
            let warning = if class.warning.is_empty() {
                String::new()
            } else {
                format!(" · {}", class.warning)
            };
            format!(
                "{name} · {} · {} · Construction {}{warning}",
                class.name,
                context.skill.label(),
                context.construction.label()
            )
        }
    };
    let has_save = snapshot.is_some();
    let controls = &state.stats_controls;
    let pick =
        |select: &ChoiceSelect<usize>| Select::new(select.state()).placeholder("Load a save first");
    let row_one = div()
        .flex()
        .items_end()
        .gap_3()
        .child(context_select(
            pick(&controls.character),
            "stats-character",
            "Character",
            has_save,
            muted,
        ))
        .child(context_select(
            pick(&controls.map),
            "stats-map",
            "Map",
            has_save,
            muted,
        ))
        .child(context_select(
            Select::new(controls.visibility.state()),
            "stats-map-visibility",
            "Map List",
            true,
            muted,
        ));
    let row_two = div()
        .flex()
        .items_end()
        .gap_3()
        .child(context_select(
            Select::new(controls.skill.state()),
            "stats-skill",
            "Skill",
            true,
            muted,
        ))
        .child(context_select(
            Select::new(controls.construction.state()),
            "stats-construction",
            "Construction Mode",
            true,
            muted,
        ));

    let mut page = div().flex().flex_col().gap(PANEL_GAP).child(
        Card::new()
            .header(CardHeader::new("Calculation Context").description(
                "Every breakdown below resolves against this character, map, skill and construction mode.",
            ))
            .child(
                div()
                    .id("stats-context-controls")
                    .flex()
                    .flex_col()
                    .gap(PANEL_GAP)
                    .child(row_one)
                    .child(row_two),
            )
            .child(
                div()
                    .id("stats-context-note")
                    .role(Role::Label)
                    .aria_label(note.clone())
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(note),
            ),
    );
    if let Some(snapshot) = snapshot {
        page = page.child(map_card(context, snapshot, &stats.options, muted));
    }
    page = if has_save && stats.sheet.is_none() {
        page.child(resolving())
    } else {
        page.child(sandbox::input_ledger(state, cx))
            .child(sandbox::affected_formulas(&state.stats_all, cx))
            .child(breakdowns::region(state, cx))
            .child(explorer::region(state, cx))
    };
    div()
        .id("stats-all-dashboard")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .child(page)
                .overflow_y_scrollbar()
                .id("stats-all-scroll"),
        )
        .into_any_element()
}

/// What the page shows while the sheets resolve off the UI thread.
fn resolving() -> AnyElement {
    div()
        .id("stats-resolving")
        .role(Role::Label)
        .aria_label("Resolving every descriptor")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(Skeleton::new().h(px(160.0)))
        .child(Skeleton::new().h(px(64.0)))
        .child(Skeleton::new().h(px(320.0)))
        .into_any_element()
}

/// The save slice one sheet resolves against.
fn breakdown_inputs<'a>(
    save: &'a Save,
    simulation: &'a Simulation,
    sim_context: &'a SimContext,
    context: StatsContext,
) -> stats::BreakdownInputs<'a> {
    stats::BreakdownInputs {
        save,
        simulation,
        sim_context,
        character: context.character,
        map: context.map,
    }
}

/// The map access summary: classification, evidence and the list size.
fn map_card(
    context: StatsContext,
    snapshot: &stats::Snapshot,
    options: &[stats::MapOption],
    muted: gpui::Hsla,
) -> AnyElement {
    let class = stats::classify(context.map);
    let selected = options
        .iter()
        .find(|option| option.class.map == context.map)
        .cloned();
    let evidence = selected
        .as_ref()
        .map(|option| option.evidence.clone())
        .filter(|evidence| !evidence.is_empty())
        .unwrap_or_else(|| "No access evidence for this map.".into());
    let state_label = match &selected {
        Some(option) if option.current => "Current map",
        Some(option) if option.accessible => "Accessible",
        Some(option) if option.visited => "Visited",
        Some(_) => "Not proven accessible",
        None => "Not in this map list",
    };
    let summary = format!(
        "{state_label} · {} maps in the {} list",
        options.len(),
        context.map_visibility.label()
    );
    let target = if class.target_name.is_empty() {
        "no AFK target".to_string()
    } else {
        class.target_name.replace('_', " ")
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            Card::new()
                .header(CardHeader::new("Map Access").description(format!(
                    "{} · World {} · {} · {target}",
                    class.name,
                    class.world,
                    class.category.label()
                )))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .id("stats-map-class")
                                .role(Role::Label)
                                .aria_label(format!("Map class: {}", class.category.label()))
                                .text_size(TEXT_CAPTION)
                                .child(format!(
                                    "{} · World {} · {}",
                                    class.name,
                                    class.world,
                                    class.category.label()
                                )),
                        )
                        .child(
                            div()
                                .id("stats-map-evidence")
                                .role(Role::Label)
                                .aria_label(format!("Map evidence: {evidence}"))
                                .text_size(TEXT_CAPTION)
                                .text_color(muted)
                                .child(evidence),
                        )
                        .child(
                            div()
                                .id("stats-map-count")
                                .role(Role::Label)
                                .aria_label(summary.clone())
                                .text_size(TEXT_CAPTION)
                                .text_color(muted)
                                .child(summary),
                        ),
                )
                .child(
                    Caption::new(format!(
                        "{} characters in this save · {} maps in the game data",
                        snapshot.characters(),
                        stats::map_count()
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// One labelled context select under its harness id.
fn context_select<D: SearchableListDelegate + 'static>(
    select: Select<D>,
    id: &'static str,
    label: &'static str,
    enabled: bool,
    muted: gpui::Hsla,
) -> gpui::Div {
    field(label, muted).child(
        div().flex_none().w(px(280.0)).child(
            select
                .id(id)
                .accessibility_label(label)
                .disabled(!enabled)
                .small(),
        ),
    )
}

fn field(label: &'static str, muted: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
}

/// A small status tag.
pub(super) fn tag(label: &'static str, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label)
}

/// Restore a project file's context, warning about values the save cannot show.
pub(crate) fn restore(state: &mut AppState, context: StatsContext, warnings: &mut Vec<String>) {
    state.stats_all.context = context;
    state.stats_all.clamp(warnings);
    // The restored character and map re-resolve every breakdown on the next
    // render, so the page never shows the previous context's values.
    state.stats_all.invalidate(true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_stats::{ConstructionMode, MapVisibility, Skill};
    use serde_json::json;

    /// Build what the state left pending, as the page does off the UI thread.
    fn build(state: &mut State, save: &Save) {
        if let Some(job) = state.job(save, &simulation(), &context()) {
            state.adopt(job());
        }
    }

    fn simulation() -> Simulation {
        Simulation::default()
    }

    fn context() -> SimContext {
        SimContext::default()
    }

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
        let save = save();
        state.load(&save);
        build(&mut state, &save);
        assert_eq!(state.context.character, 0);
        assert_eq!(state.context.map, 0);
        assert!(state.snapshot.is_some());
        assert_eq!(
            state.options.first().map(|option| option.class.map),
            Some(0)
        );
    }

    #[test]
    fn a_restored_context_outside_the_save_is_clamped_and_named() {
        let mut state = State::default();
        let save = save();
        state.load(&save);
        build(&mut state, &save);
        let mut warnings = Vec::new();
        state.context = StatsContext {
            character: 12,
            map: 29,
            map_visibility: MapVisibility::Accessible,
            skill: Skill::Spelunking,
            construction: ConstructionMode::Inactive,
        };
        state.clamp(&mut warnings);
        assert_eq!(state.context.character, 0);
        assert_eq!(state.context.map, 0);
        assert_eq!(state.context.skill, Skill::Spelunking);
        assert_eq!(state.context.construction, ConstructionMode::Inactive);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].contains("character 13"), "{warnings:?}");
        assert!(warnings[1].contains("map 29"), "{warnings:?}");
    }

    #[test]
    fn every_category_is_a_tab_and_an_unmodelled_row_warns() {
        let mut state = State::default();
        let save = save();
        state.load(&save);
        build(&mut state, &save);
        let sheet = state.sheet.as_ref().expect("a save builds the sheet");
        assert_eq!(BreakdownCategory::ALL.len(), 9);
        for category in BreakdownCategory::ALL {
            assert!(sheet.count(category) > 0, "{} is empty", category.label());
        }
        // A save without MaxCarryCap rows reports the capacity rows as missing
        // rather than inventing a value.
        let mining = sheet.get("carry-capacity-mining").expect("the row");
        assert!(mining.display_value().is_none());
        assert!(
            mining
                .warning()
                .unwrap_or_default()
                .contains("MaxCarryCap_0")
        );
        // A row this build does not model warns instead of showing a value.
        let crit = sheet.get("crit-damage").expect("the row");
        assert!(crit.display_value().is_none());
        assert!(crit.warning().is_some_and(|warning| !warning.is_empty()));
    }

    /// One character's bag row and four gem-shop purchases, so a hypothetical
    /// edit has a computed formula to move.
    fn capacity_save() -> Save {
        let mut gems = vec![json!(0); 59];
        gems[58] = json!(4);
        Save::new(json!({
            "charNames": ["Ana"],
            "MaxCarryCap_0": "{\"Mining\":1000}",
            "GemItemsPurchased": gems,
        }))
    }

    #[test]
    fn a_pending_edit_moves_the_value_and_the_ledger_keeps_the_saved_one() {
        let save = capacity_save();
        let mut state = State::default();
        state.load(&save);
        build(&mut state, &save);
        build(&mut state, &save);
        let value = |state: &State| {
            state
                .sheet
                .as_ref()
                .and_then(|sheet| sheet.get("carry-capacity-mining"))
                .and_then(|row| row.resolution.value())
                .expect("the capacity row resolves")
        };
        let saved = value(&state);
        assert!(saved > 0.0);
        assert!(state.affected.is_empty());
        assert!(state.saved_sheet.is_some());

        state
            .sandbox
            .as_mut()
            .expect("a save builds the sandbox")
            .set("gem-items-58", 0.0)
            .expect("a purchase count");
        state.invalidate(false);
        build(&mut state, &save);
        assert!(value(&state) < saved, "the edit halves the value");
        assert_eq!(state.affected.len(), 1, "{:?}", state.affected);
        assert_eq!(state.affected[0].id, "carry-capacity-mining");
        // The baseline keeps the saved value, so the ledger can show both.
        let baseline = state.saved_sheet.as_ref().expect("the baseline sheet");
        assert_eq!(
            baseline
                .get("carry-capacity-mining")
                .and_then(|row| row.resolution.value()),
            Some(saved)
        );

        state
            .sandbox
            .as_mut()
            .expect("the sandbox survives")
            .reset_all();
        state.invalidate(false);
        build(&mut state, &save);
        assert_eq!(value(&state), saved);
        assert!(state.affected.is_empty());
    }

    #[test]
    fn a_real_capacity_row_selects_its_formula_tree() {
        let save = Save::new(json!({
            "charNames": ["Ana"],
            "MaxCarryCap_0": "{\"Mining\":1000}",
        }));
        let mut state = State::default();
        state.load(&save);
        build(&mut state, &save);
        build(&mut state, &save);
        state.category = BreakdownCategory::Capacity;
        state.select_first();
        let row = state.selected_row().expect("the first capacity row");
        assert_eq!(row.id, "carry-capacity-mining");
        assert!(row.resolution.tree().is_some());
        assert!(row.display_value().is_some());
    }
}
