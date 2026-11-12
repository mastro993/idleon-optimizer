//! The import and export dialogs: preview, apply and copy documents.

use crate::sections::endgame;
use crate::session::RecentImport;
use crate::state::{AppState, discard_run};
use crate::ui::export::Export;
use crate::ui::import::{self, Pending};
use gpui::{Context, Window};
use idleon_fishing::FishingSave;
use idleon_import::{Imported, pipeline};
use idleon_monument as monument;
use idleon_opal::Opal;
use idleon_sailing::{AssignmentMode, Goal};

impl AppState {
    /// Open the shared import dialog, where every section's header import goes.
    pub(crate) fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import.open = true;
        let focus = self.import.input.read(cx).focus().clone();
        window.focus(&focus, cx);
        self.set_status(
            "Importing a save: drop a file, browse, or paste the JSON.",
            cx,
        );
    }

    /// Close the dialog without applying; the loaded run stays as it was.
    pub(crate) fn close_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.import.open = false;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Re-check the paste box. Runs on every edit, so the preview always describes
    /// what is on screen.
    pub(crate) fn check_paste(&mut self, cx: &mut Context<Self>) {
        let text = self.import.input.read(cx).text().to_string();
        self.check_import(&text, "pasted text", cx);
    }

    /// Parse `text` into a pending import and describe it. Nothing is applied
    /// here: the import pipeline decodes everything the document needs, so a
    /// failure only replaces the message and Apply cannot fail halfway.
    pub(crate) fn check_import(&mut self, text: &str, source: &str, cx: &mut Context<Self>) {
        self.import.source = source.to_string();
        if text.trim().is_empty() {
            self.import.pending = None;
            self.import.ok = true;
            self.import.message =
                "Paste a decoded save, a project file, an ROv2 state string, or drop one.".into();
            return cx.notify();
        }
        match pipeline::read(text) {
            Ok(mut document) => {
                let ids: Vec<String> = match &document {
                    pipeline::Document::Project(project) => project
                        .document
                        .sections
                        .keys()
                        .chain(project.document.section.iter())
                        .cloned()
                        .collect(),
                    pipeline::Document::Section(section) => vec![section.section.clone()],
                    pipeline::Document::Save(_) => Vec::new(),
                };
                for id in ids {
                    warn_unknown_section(document.warnings_mut(), &id);
                }
                // Decode every section config now: a document that cannot be
                // applied whole never becomes pending.
                match crate::project::prepare(&mut document) {
                    Ok(configs) => {
                        self.import.message = import::summary(&document);
                        self.import.pending = Some(Pending { document, configs });
                        self.import.ok = true;
                    }
                    Err(error) => self.fail_import(source, &error, cx),
                }
            }
            Err(error) => self.fail_import(source, &error.to_string(), cx),
        }
        cx.notify();
    }

    /// Record a dropped file that could not even be read.
    pub(crate) fn fail_import(&mut self, source: &str, reason: &str, cx: &mut Context<Self>) {
        self.import.source = source.to_string();
        self.import.pending = None;
        self.import.ok = false;
        self.import.message = format!("Could not read {source}: {reason}");
        cx.notify();
    }

    /// Apply the previewed document; a dialog with nothing pending changes
    /// nothing. Everything was decoded while previewing, so this only assigns:
    /// a document either applies whole or never becomes pending. Every change
    /// is logged.
    pub(crate) fn apply_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Pending { document, configs }) = self.import.pending.take() else {
            return;
        };
        let format = document.format().to_string();
        let characters = document.characters();
        let research_level = document.research_level();
        let mut warnings = document.warnings().to_vec();
        let mut message = import::applied(&document);
        let before_configs = crate::project::configs(self);
        let before_cog = self.cog.save.clone();
        let mut changes = Vec::new();
        match document {
            pipeline::Document::Save(imported) => {
                changes.push(format!(
                    "save: {} · {} character(s)",
                    imported.format.label(),
                    imported.characters
                ));
                changes.extend(crate::project::research_changes(
                    &self.simulation,
                    &imported.simulation,
                ));
                changes.extend(crate::project::supplement_changes(
                    &self.context,
                    &imported.context,
                ));
                self.load_imported(*imported, cx);
            }
            pipeline::Document::Project(project) => {
                crate::project::apply(self, *project, configs, &mut warnings, &mut changes, cx);
                self.save_partial |= !warnings.is_empty();
            }
            pipeline::Document::Section(_) => {
                for config in configs {
                    crate::project::assign(self, config, &mut warnings, cx);
                }
            }
        }
        if self.cog.save != before_cog {
            discard_run(&self.cog_run, cx);
            discard_run(&self.cog_perfect_run, cx);
        }
        changes.extend(crate::project::config_changes(
            &before_configs,
            &crate::project::configs(self),
        ));
        if !changes.is_empty() {
            message = format!("{message} {}", crate::project::change_line(&changes));
        }
        if !warnings.is_empty() {
            message = format!("{message} {}", warnings_line(&warnings));
        }
        self.import.ok = true;
        self.import.changes = changes;
        self.import.message = message.clone();
        self.status = message;
        self.session.record(RecentImport::new(
            format,
            self.import.source.clone(),
            characters,
            research_level,
            warnings.len(),
        ));
        self.flush_session();
        self.close_import(window, cx);
    }

    /// Take a save the importer already read into every section that reads it.
    pub(crate) fn load_imported(&mut self, imported: Imported, cx: &mut Context<Self>) {
        self.monument_save = Some(monument::Save::from_parts(
            &imported.save,
            imported.companion.as_ref(),
        ));
        let monument_characters = imported.save.character_count();
        if monument_characters == 0 || self.monument.character >= monument_characters {
            self.monument.character = 0;
        }
        // The Justice worksheet transcribes this account's story, so a new save
        // starts it from the new fresh-story stats.
        if let Some(save) = self.monument_save.as_ref() {
            let initial = monument::justice::initial_state(save);
            self.monument.worksheet.reset(initial);
            self.monument.notice.clear();
        }
        self.endgame = endgame::State::audit(&imported);
        self.coin_multi.load(&imported, cx);
        self.fishing_save = FishingSave::from_save(&imported.save);
        self.fishing.adopt(&self.fishing_save);
        self.simulation = imported.simulation;
        // A new save replaces the layout the trail would undo into.
        self.research.history.clear();
        self.context = imported.context.clone();
        self.context_base = imported.context;
        self.normalize_target();
        let combat_character = if self.combat.character < imported.save.character_count() {
            self.combat.character
        } else {
            0
        };
        let character_count = imported.save.character_count();
        self.combat.load(&imported.save, combat_character);
        self.stats_all
            .load(&imported.save, &self.simulation, &self.context);
        self.drop_rate.load(&imported.save);
        self.damage.load(&imported.save);
        self.save = Some(imported.save);
        self.research
            .minehead
            .load(self.save.as_ref(), &self.simulation);
        self.save_revision = self.save_revision.wrapping_add(1);
        self.prisma.adopt(self.save.as_ref());
        self.statue.load(imported.statues, cx);
        self.coral.adopt(&imported.coral);
        self.worship_adopt(imported.worship);
        self.fleet = imported.fleet;
        self.sailing = imported.sailing;
        self.farm = imported.farming;
        self.farming.adopt(&self.farm);
        self.cog.load(imported.cog);
        self.cglunko = imported.cglunko;
        // A plan from the previous save describes inputs this one no longer has.
        discard_run(&self.cglunko_run, cx);
        self.clamworks = imported.clamworks;
        // The plan's default budget follows the new balance; a projection from
        // the previous save no longer describes this one.
        self.clamworks_budget = self.clamworks.pearl_balance;
        self.clamworks_projection = None;
        self.clamworks_claim.load(character_count);
        self.jar = imported.jar;
        self.spelunking_save = imported.spelunking;
        if let Some(save) = self.spelunking_save.as_ref() {
            // Seed from the save alone: a previous save's maximum-stamina
            // overrides must not become the new save's saved stamina.
            let inputs = save.inputs(&idleon_spelunking::Account::neutral());
            self.spelunking.adopt(&inputs);
        }
        self.ladle_state
            .adopt(&imported.ladle.clone().unwrap_or_default());
        self.ladle = imported.ladle;
        self.mastery = imported.mastery;
        self.yellow = imported.yellow;
        // Both budgets default to the new save's own point total.
        self.mastery_plan.adopt();
        self.jelly = imported.jelly;
        if let Some(jelly) = self.jelly.clone() {
            self.jelly_state.adopt(&jelly);
        }
        self.fountain = imported.fountain;
        self.fountain_state.adopt(self.fountain.as_ref());
        // A plan from the previous cavern describes inputs this one no longer has.
        discard_run(&self.fountain_plan, cx);
        self.opal = imported.opal;
        self.opal_selected = None;
        self.opal_target = None;
        // The plan's default total is the new save's invested opals.
        self.opal_total = self.opal.as_ref().map_or(0.0, Opal::total_opals);
        self.royal = imported.royal;
        self.sneaking = imported.sneaking;
        self.research.selected_cell = None;
        self.sailing_state.island = self
            .fleet
            .boats
            .first()
            .and_then(|boat| boat.target_island)
            .filter(|island| *island < self.fleet.available_islands())
            .unwrap_or(0);
        // The simulation has its own destination; it follows the save's island
        // count like the fleet selector does.
        self.sailing_state.sim_island = self
            .sailing_state
            .sim_island
            .min(self.fleet.available_islands().saturating_sub(1));
        self.sailing_state.mode = AssignmentMode::Optimized;
        self.sailing_state.goal = Goal::Treasure;
        self.save_loaded = true;
        self.save_partial = !imported.warnings.is_empty();
        self.shadow = imported.shadow;
        // The stored comparison stays; a save that changes the plan marks it
        // stale through the runner's own input check.
        self.killroy_default_selection();
    }

    /// Open the export dialog over a freshly written project document.
    pub(crate) fn open_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.export = Export::open(self);
        self.status = format!(
            "Exporting the project ({} bytes) and its state string ({} bytes).",
            self.export.bytes(),
            self.export.state_bytes()
        );
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Close the export dialog without touching the project.
    pub(crate) fn close_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.export.open = false;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Put the whole project document on the clipboard.
    pub(crate) fn copy_export(&mut self, cx: &mut Context<Self>) {
        crate::ui::export::copy(self, cx);
        self.set_status(
            format!(
                "Copied the project JSON to the clipboard ({} bytes).",
                self.export.bytes()
            ),
            cx,
        );
    }

    /// Put the `ROv2:` state string on the clipboard, for the upstream tool or
    /// the import dialog's paste box.
    pub(crate) fn copy_export_state(&mut self, cx: &mut Context<Self>) {
        crate::ui::export::copy_state(self, cx);
        self.set_status(
            format!(
                "Copied the ROv2 state string to the clipboard ({} bytes).",
                self.export.state_bytes()
            ),
            cx,
        );
    }
}

/// A warning count worth the status line, naming the first one.
fn warnings_line(warnings: &[String]) -> String {
    match warnings.first() {
        None => "No warnings.".into(),
        Some(first) if warnings.len() == 1 => first.clone(),
        Some(first) => format!("{first} (and {} more warnings)", warnings.len() - 1),
    }
}

/// Name a section id the registry does not declare in the preview's warnings,
/// and leave a declared id alone.
fn warn_unknown_section(warnings: &mut Vec<String>, id: &str) {
    if crate::sections::find(id).is_none() {
        warnings.push(format!(
            "section: {id} is not a section this build declares"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_section_id_warns_and_a_declared_one_does_not() {
        let mut warnings = Vec::new();
        warn_unknown_section(&mut warnings, "research");
        assert!(warnings.is_empty(), "{warnings:?}");
        warn_unknown_section(&mut warnings, "no-such-section");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("no-such-section"), "{warnings:?}");
    }
}
