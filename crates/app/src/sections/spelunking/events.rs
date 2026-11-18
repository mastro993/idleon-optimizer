//! The expedition run's event log as a virtualized kit table.
//!
//! A run logs thousands of events, so the rows are built once when the run is
//! stored and the table draws only the ones in view. Each row keeps the
//! `spelunk-run-event-<n>` id on its first cell.

use gpui::prelude::*;
use gpui::{App, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, px};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::table::{Column, TableDelegate, TableState};
use idleon_spelunking::{EventKind, RunResult, data};
use idleon_ui::format;
use idleon_ui::widgets::TEXT_MONO;

struct Row {
    label: SharedString,
    detail: SharedString,
    when: SharedString,
}

#[derive(Default)]
pub(crate) struct Events {
    rows: Vec<Row>,
}

impl Events {
    /// Show `run`'s events, or none.
    pub(crate) fn show(&mut self, run: Option<&RunResult>) {
        self.rows = run.map_or_else(Vec::new, |run| {
            run.events
                .iter()
                .map(|event| {
                    let minutes = (event.time / 60.0).floor();
                    let seconds = event.time - minutes * 60.0;
                    Row {
                        label: event_label(&event.kind).into(),
                        detail: event_detail(&event.kind).into(),
                        when: format!("D{} · {minutes:.0}:{seconds:04.1}", event.depth).into(),
                    }
                })
                .collect()
        });
    }

    fn text(&self, row_ix: usize, col_ix: usize) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        Some(match col_ix {
            0 => row.label.clone(),
            1 => row.detail.clone(),
            _ => row.when.clone(),
        })
    }
}

impl TableDelegate for Events {
    fn columns_count(&self, _: &App) -> usize {
        3
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("event", "Event").width(px(220.0)),
            1 => Column::new("detail", "Detail").width(px(360.0)),
            _ => Column::new("when", "Depth · time")
                .width(px(140.0))
                .text_right(),
        }
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let text = self.text(row_ix, col_ix).unwrap_or_default();
        match col_ix {
            0 => div()
                .id(SharedString::from(format!("spelunk-run-event-{row_ix}")))
                .role(Role::Label)
                .aria_label(text.clone())
                .child(text)
                .into_any_element(),
            1 => div()
                .text_color(cx.theme().muted_foreground)
                .child(text)
                .into_any_element(),
            _ => div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(TEXT_MONO)
                .text_color(cx.theme().muted_foreground)
                .child(text)
                .into_any_element(),
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        self.text(row_ix, col_ix)
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        self.text(row_ix, col_ix)
            .map(String::from)
            .unwrap_or_default()
    }
}

fn event_label(kind: &EventKind) -> String {
    match kind {
        EventKind::Floor => "Entered floor".into(),
        EventKind::Dig { .. } => "Dig".into(),
        EventKind::Elixir { id, .. } => data()
            .elixirs
            .get(*id)
            .map_or_else(|| format!("Elixir {id}"), |elixir| elixir.name.clone()),
        EventKind::Switch { character } => format!("Switch to character {}", character + 1),
        EventKind::Boss => "Boss defeated".into(),
        EventKind::Discovery { name } => format!("Discovered {}", name.replace('_', " ")),
        EventKind::GrandDiscovery => "Grand Discovery".into(),
        EventKind::ExitFound => "Exit revealed".into(),
        EventKind::Descend => "Next floor".into(),
        EventKind::AmberPhase { phase, .. } => format!("Amber phase: {}", phase.label()),
        EventKind::Exhausted => "Stamina exhausted".into(),
        EventKind::Rope => "Rope exit".into(),
        EventKind::Blocked => "No way down".into(),
    }
}

fn event_detail(kind: &EventKind) -> String {
    match kind {
        EventKind::Dig {
            tool,
            character,
            used,
        } => format!(
            "{} · {} stamina · character {}",
            data()
                .tools
                .iter()
                .find(|candidate| candidate.id == *tool)
                .map_or_else(|| format!("tool {tool}"), |tool| tool.name.clone()),
            format::value(*used),
            character + 1
        ),
        EventKind::Elixir { slot, .. } => format!("Belt slot {}", slot + 1),
        EventKind::AmberPhase { reason, .. } => reason.clone(),
        _ => String::new(),
    }
}
