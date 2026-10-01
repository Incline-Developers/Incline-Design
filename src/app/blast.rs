//! Drill & Blast's charging, and its reviews of the fired pattern.
//!
//! The charge library - products and loading rules - is configuration,
//! edited here through the command round-trip and written to the config file
//! like the delay palette. Loading holes is a project edit: one
//! [`Command::SetCharges`] per gesture, so a box of holes loads, and undoes,
//! in one step.

use crate::{
    app::App,
    i18n::{tr, tr_format},
    logging::CommandReportSpec,
    model::{
        Command,
        blast::{ChargeFailure, ChargeProduct, ChargeRule, charge_hole},
        drill_hole::DrillHoleRef,
    },
    ui::state::{ActiveTool, BlastHover, BlastLibraryItem, Workspace},
    userspace_log, userspace_warn,
};

impl App<'_> {
    /// Add a charge product, or replace the one named `original`. A rename
    /// carries through to every rule that loads it.
    pub(crate) fn save_charge_product(&mut self, original: Option<String>, product: ChargeProduct) {
        let library = &mut self.editor.blast_library;
        let name = product.name.clone();
        match original
            .as_ref()
            .and_then(|original| library.products.iter().position(|existing| &existing.name == original))
        {
            Some(index) => {
                let previous = std::mem::replace(&mut library.products[index], product);
                if previous.name != name {
                    for deck in library.rules.iter_mut().flat_map(|rule| rule.decks.iter_mut()) {
                        if deck.product == previous.name {
                            deck.product = name.clone();
                        }
                    }
                }
                userspace_log!("{}", tr_format!(literal = "Updated charge product %name%", name = &name));
            }
            None => {
                library.products.push(product);
                userspace_log!("{}", tr_format!(literal = "Added charge product %name%", name = &name));
            }
        }
        self.persist_blast_library();
    }

    /// Add a loading rule, or replace the one named `original`. A new rule
    /// becomes the one the Charge Holes tool loads with.
    pub(crate) fn save_charge_rule(&mut self, original: Option<String>, rule: ChargeRule, reload: bool) {
        // Holes loaded by the rule before the edit, found before it renames.
        let reload_holes: Vec<usize> = match (reload, original.as_ref(), self.tie_target()) {
            (true, Some(original), Some(dataset)) => dataset
                .dataset
                .charges
                .iter()
                .filter(|(_, charge)| &charge.rule == original)
                .map(|(hole, _)| *hole)
                .collect(),
            _ => Vec::new(),
        };
        let reload_rule = rule.clone();
        let library = &mut self.editor.blast_library;
        let name = rule.name.clone();
        match original.as_ref().and_then(|original| library.rules.iter().position(|existing| &existing.name == original)) {
            Some(index) => {
                library.rules[index] = rule;
                if self.editor.active_charge_rule.as_ref() == original.as_ref() {
                    self.editor.active_charge_rule = Some(name.clone());
                }
                userspace_log!("{}", tr_format!(literal = "Updated charge rule %name%", name = &name));
            }
            None => {
                library.rules.push(rule);
                self.editor.active_charge_rule = Some(name.clone());
                userspace_log!("{}", tr_format!(literal = "Added charge rule %name%", name = &name));
            }
        }
        self.persist_blast_library();
        if !reload_holes.is_empty() {
            self.charge_holes(&reload_holes, Some(&reload_rule));
        }
    }

    pub(crate) fn delete_blast_library_item(&mut self, item: BlastLibraryItem) {
        let library = &mut self.editor.blast_library;
        let removed = match &item {
            BlastLibraryItem::Product(name) => {
                let before = library.products.len();
                library.products.retain(|product| &product.name != name);
                before != library.products.len()
            }
            BlastLibraryItem::Rule(name) => {
                let before = library.rules.len();
                library.rules.retain(|rule| &rule.name != name);
                before != library.rules.len()
            }
        };
        if !removed {
            userspace_warn!("{}", tr!(literal = "That entry is no longer in the charge library"));
            return;
        }
        userspace_log!("{}", tr_format!(literal = "Deleted %name% from the charge library", name = item.name()));
        self.persist_blast_library();
    }

    /// Write the library back to the config file, whole, the way the delay
    /// palette is - see [`crate::app::commands::view::config_from`].
    fn persist_blast_library(&mut self) {
        let preferences = self.editor.current_preferences();
        if let Err(error) = crate::app::io::save_config(&crate::app::commands::view::config_from(
            &preferences,
            self.editor.workspace_order,
            self.editor.delay_products.iter().map(crate::ui::state::DelayProduct::to_stored).collect(),
            self.editor.blast_library.clone(),
            self.editor.survey.definitions.clone(),
            self.editor.survey.local_system.clone(),
        )) {
            userspace_warn!("{}", tr_format!(literal = "Failed to save the charge library: %error%", error = error));
        }
    }

    /// Load `holes` of the active dataset with `rule`, or unload them with
    /// `None`, as one undoable step. Holes the rule cannot fit are left as
    /// they were and counted in the report.
    pub(crate) fn charge_holes(&mut self, holes: &[usize], rule: Option<&ChargeRule>) {
        let Some(dataset) = self.tie_target() else {
            return;
        };
        let id = dataset.id;
        let data = &dataset.dataset;
        let products = &self.editor.blast_library.products;
        if let Some(problem) = rule.and_then(|rule| rule.problem(products)) {
            userspace_warn!("{}", tr_format!(literal = "Cannot load with this rule: %problem%", problem = problem));
            return;
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let (mut too_short, mut no_depth, mut no_diameter) = (0usize, 0usize, 0usize);
        for &index in holes {
            let Some(hole) = data.holes.get(index) else {
                continue;
            };
            let current = data.charges.get(&index).cloned();
            let next = match rule {
                Some(rule) => match charge_hole(rule, products, hole) {
                    Ok(charge) => {
                        no_diameter += usize::from(hole.diameter.is_none());
                        Some(charge)
                    }
                    Err(ChargeFailure::TooShort) => {
                        too_short += 1;
                        continue;
                    }
                    Err(ChargeFailure::NoDepth) => {
                        no_depth += 1;
                        continue;
                    }
                },
                None => None,
            };
            if current != next {
                before.push((index, current));
                after.push((index, next));
            }
        }
        if too_short > 0 {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "%count% hole(s) are too short for the fixed decks of this rule and were left as they were",
                    count = too_short
                )
            );
        }
        if no_depth > 0 {
            userspace_warn!("{}", tr_format!(literal = "%count% hole(s) have no depth to load", count = no_depth));
        }
        if no_diameter > 0 {
            userspace_warn!(
                "{}",
                tr_format!(literal = "%count% loaded hole(s) have no diameter, so their explosive mass is unknown", count = no_diameter)
            );
        }
        if after.is_empty() {
            return;
        }
        let count = after.len();
        self.execute_edit(Command::SetCharges { dataset: id, before, after });
        let (title, detail) = match rule {
            Some(rule) => (
                tr!(literal = "Charge Holes"),
                tr_format!(literal = "Loaded %count% hole(s) with %rule%", count = count, rule = &rule.name),
            ),
            None => (tr!(literal = "Unload Holes"), tr_format!(literal = "Unloaded %count% hole(s)", count = count)),
        };
        crate::logging::report_completed_action(CommandReportSpec::new(title, rule.map_or_else(String::new, |rule| rule.name.clone())), detail);
    }

    /// Load or unload the selected holes of the active dataset, from the
    /// products panel's rule menu.
    pub(crate) fn charge_selected_holes(&mut self, rule: Option<String>) {
        let Some(id) = self.editor.active_drill_hole else {
            return;
        };
        let mut holes: Vec<usize> = self.editor.selected_drill_holes.iter().filter(|hole| hole.dataset == id).map(|hole| hole.hole).collect();
        holes.sort_unstable();
        if holes.is_empty() {
            userspace_warn!("{}", tr!(literal = "Select holes of the active pattern first"));
            return;
        }
        let rule = match rule {
            Some(name) => match self.editor.blast_library.rules.iter().find(|rule| rule.name == name).cloned() {
                Some(rule) => Some(rule),
                None => {
                    userspace_warn!("{}", tr!(literal = "That rule is no longer in the charge library"));
                    return;
                }
            },
            None => None,
        };
        self.charge_holes(&holes, rule.as_ref());
    }

    /// A press with the Charge Holes tool: load the hole under the pointer -
    /// or the whole selection, when the hole is part of it - and start a box
    /// over open ground. Shift unloads.
    pub(crate) fn charge_holes_press(&mut self) {
        let Some(hole) = self.pick_hole_at_cursor() else {
            // Over open ground the press may become a box: the drag is the
            // ordinary marquee, finished in `finish_charge_box`.
            self.begin_select_or_drag();
            return;
        };
        let holes: Vec<usize> = if self.editor.selected_drill_holes.contains(&hole) {
            let mut holes: Vec<usize> = self
                .editor
                .selected_drill_holes
                .iter()
                .filter(|other| other.dataset == hole.dataset)
                .map(|other| other.hole)
                .collect();
            holes.sort_unstable();
            holes
        } else {
            vec![hole.hole]
        };
        self.charge_with_tool(&holes);
    }

    /// The box half of the Charge Holes tool: everything enclosed is loaded
    /// (or, with Shift, unloaded) and the selection is left alone.
    pub(crate) fn finish_charge_box(&mut self, enclosed: Vec<DrillHoleRef>) {
        let Some(id) = self.tie_target().map(|dataset| dataset.id) else {
            return;
        };
        let mut holes: Vec<usize> = enclosed.into_iter().filter(|hole| hole.dataset == id).map(|hole| hole.hole).collect();
        holes.sort_unstable();
        holes.dedup();
        self.charge_with_tool(&holes);
    }

    fn charge_with_tool(&mut self, holes: &[usize]) {
        if holes.is_empty() {
            return;
        }
        if self.modifiers.shift_key() {
            self.charge_holes(holes, None);
            return;
        }
        let Some(rule) = self.editor.active_rule().cloned() else {
            userspace_warn!("{}", tr!(literal = "There is no charge rule to load with: add one in the products panel"));
            return;
        };
        self.charge_holes(holes, Some(&rule));
    }

    /// Find the hole under the pointer for the hole card. Only while nothing
    /// else owns the pointer: a tie-in chain or a drag has its own preview.
    pub(crate) fn refresh_blast_hover(&mut self) {
        let wants_card = self.editor.active_workspace == Workspace::DrillAndBlast
            && matches!(self.editor.active_tool, ActiveTool::None | ActiveTool::ChargeHoles | ActiveTool::SetInitiationPoint)
            && self.editor.selection_box_start_px.is_none()
            && self.editor.initiation_dialog.is_none();
        let hover = wants_card
            .then(|| {
                let hole = self.pick_hole_at_cursor()?;
                let graphics = self.graphics.as_ref()?;
                let dataset = self.drill_holes.iter().find(|dataset| dataset.id == hole.dataset)?;
                let collar = dataset.dataset.holes.get(hole.hole)?.collar_position();
                let screen_px = graphics.world_to_window_px(&graphics.view_proj(), collar)?;
                Some(BlastHover { hole, screen_px })
            })
            .flatten();
        if hover != self.editor.blast_hover {
            self.editor.blast_hover = hover;
            self.redraw_requested = true;
        }
    }
}
