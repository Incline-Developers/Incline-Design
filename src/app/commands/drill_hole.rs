use anyhow::{Context, Result};

use crate::{
    app::App,
    i18n::{tr, tr_format},
    model::{
        Command, ItemRef, ItemStyle, MemberKind, OpenItem, SceneEntityId,
        drill_hole::{
            DrillColorPreset, DrillColorState, DrillColorStop, DrillFieldKind, DrillHole, DrillHoleDataset, DrillHoleId, DrillHoleRef, DrillHoleSource, LoadedDrillHoleDataset,
            MAX_DRILL_COLOR_STOPS, OpenDrillHoleDataset, OrientationSource, TraceStation, WIDE_CATEGORY_FIELD_HINT,
        },
        formats::csv_drill_hole,
    },
    userspace_log, userspace_warn,
};

#[cfg(target_arch = "wasm32")]
fn remap_browser_source_path(source: &mut DrillHoleSource, path: std::path::PathBuf) {
    match source {
        DrillHoleSource::LegacyDhd { path: source_path } => *source_path = path,
        DrillHoleSource::Csv { browser_path, .. } => *browser_path = Some(path),
        DrillHoleSource::Omf { path: source_path, .. } => *source_path = path,
    }
}

#[cfg(target_arch = "wasm32")]
fn parse_browser_bundle<'bytes>(source: &DrillHoleSource, bytes: impl IntoIterator<Item = &'bytes [u8]>) -> Result<crate::model::drill_hole::DrillHoleDataset> {
    let bytes = bytes.into_iter().collect::<Vec<_>>();
    match source {
        DrillHoleSource::LegacyDhd { .. } => anyhow::bail!("DHD drillhole sources are no longer supported"),
        DrillHoleSource::Csv { files, .. } => {
            if files.len() != bytes.len() {
                anyhow::bail!("Stored CSV manifest contains {} mappings but {} files", files.len(), bytes.len());
            }
            csv_drill_hole::parse_bundle(files.iter().zip(bytes)).map_err(anyhow::Error::new)
        }
        DrillHoleSource::Omf { .. } => anyhow::bail!("OMF drillhole data is loaded through the project importer"),
    }
}

impl<'a> App<'a> {
    /// Turn the pattern menu's exact preview into a normal project-owned
    /// drillhole dataset. Generated patterns need no reload source: project
    /// saves already embed every collar and trace in OMF.
    pub(crate) fn create_drill_pattern(&mut self, name: String, collars: Vec<glam::DVec3>, depth: f64, diameter: f64) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("{}", tr!(literal = "Enter a name for the drill pattern"));
        }
        if !depth.is_finite() || depth <= 0.0 {
            anyhow::bail!("{}", tr!(literal = "Hole depth must be greater than zero"));
        }
        if !diameter.is_finite() || diameter <= 0.0 {
            anyhow::bail!("{}", tr!(literal = "Hole diameter must be greater than zero"));
        }
        if collars.is_empty() {
            anyhow::bail!("{}", tr!(literal = "The pattern contains no holes"));
        }
        if collars.len() > crate::model::drill_hole::MAX_PATTERN_HOLES || collars.iter().any(|collar| !collar.is_finite()) {
            anyhow::bail!("{}", tr!(literal = "The drill pattern is too large or contains invalid collar coordinates"));
        }

        let width = collars.len().to_string().len().max(3);
        let holes = collars
            .into_iter()
            .enumerate()
            .map(|(index, collar)| DrillHole {
                dhid: format!("H{:0width$}", index + 1, width = width),
                collar,
                diameter: Some(diameter),
                trace: vec![
                    TraceStation { depth: 0.0, position: collar },
                    TraceStation {
                        depth,
                        position: collar - glam::DVec3::Z * depth,
                    },
                ],
                render_ranges: Vec::new(),
                intervals: Vec::new(),
                orientation_source: OrientationSource::Assumed,
            })
            .collect();
        let dataset = std::sync::Arc::new(DrillHoleDataset::new(holes));
        let id = DrillHoleId(self.next_drill_hole_id);
        self.next_drill_hole_id += 1;
        let name = crate::model::project::unique_item_name(name.to_owned(), self.drill_holes.iter().map(|item| item.name.as_str()));
        let item = OpenDrillHoleDataset {
            id,
            state: crate::model::project::ProjectItemState::dirty(MemberKind::DrillHole, None),
            name,
            dataset,
            color: DrillColorState::default(),
        };
        self.execute_edit(Command::AddItem {
            item: ItemRef::DrillHole(id),
            index: self.drill_holes.len(),
            added: Some(OpenItem::DrillHole(Box::new(item))),
        });
        self.editor.active_drill_hole = Some(id);
        self.editor.close_drill_pattern();
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn import_web_drill_hole_source(&mut self, mut source: DrillHoleSource) -> Result<()> {
        let (_, inputs) = self.web_import_files.take().context("Choose the drillhole source files again")?;
        let display_path = crate::app::browser_source_filename(&source.display_name());
        remap_browser_source_path(&mut source, display_path.clone());
        let dataset = parse_browser_bundle(&source, inputs.iter().map(|input| input.bytes.as_slice()))?;
        let loaded = LoadedDrillHoleDataset {
            name: source.display_name(),
            source,
            dataset: std::sync::Arc::new(dataset),
        };
        self.add_loaded_drill_holes(loaded);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn import_drill_hole_source(&mut self, source: DrillHoleSource) -> Result<()> {
        match &source {
            DrillHoleSource::LegacyDhd { .. } => anyhow::bail!("DHD drillhole sources are no longer supported"),
            DrillHoleSource::Csv { files, .. } if files.iter().any(|file| !file.path.is_file()) => anyhow::bail!("One or more drillhole CSV sources no longer exist"),
            _ => {}
        }
        self.open_drill_hole_source(source)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_drill_hole_source(&mut self, source: DrillHoleSource) -> Result<()> {
        if self.pending_drill_hole_loads.iter().any(|(_, pending, _, _)| *pending == source) {
            return Ok(());
        }
        let name = source.display_name();
        let (ticket, progress) = self.begin_reported_task(crate::i18n::tr_format!(literal = "Loading %name%", name = &name));
        let (tx, rx) = std::sync::mpsc::channel();
        let console_report = crate::logging::retain_current_report();
        let worker_report = console_report.as_ref().map(crate::logging::ConsoleReportHandle::child);
        self.pending_drill_hole_loads.push((ticket, source.clone(), rx, console_report));
        let window = self.window.clone();
        crate::app::jobs::spawn_pool_task(move || {
            let run = || {
                crate::app::jobs::run_compute_catching_panic(|| -> Result<_> {
                    progress.set_fraction(0.1);
                    let dataset = match &source {
                        DrillHoleSource::LegacyDhd { .. } => anyhow::bail!("DHD drillhole sources are no longer supported"),
                        DrillHoleSource::Csv { files, .. } => csv_drill_hole::parse_paths(files).with_context(|| "Failed to parse mapped drillhole CSV bundle")?,
                        DrillHoleSource::Omf { .. } => anyhow::bail!("OMF drillhole data is loaded through the project importer"),
                    };
                    progress.set_fraction(1.0);
                    Ok(LoadedDrillHoleDataset {
                        name,
                        source,
                        dataset: std::sync::Arc::new(dataset),
                    })
                })
            };
            let result = if let Some(report) = worker_report.as_ref() { report.scope(run) } else { run() };
            let _ = tx.send(result);
            if let Some(window) = window {
                window.request_redraw();
            }
        });
        Ok(())
    }

    pub(crate) fn poll_drill_hole_loads(&mut self) {
        let pending = std::mem::take(&mut self.pending_drill_hole_loads);
        for (ticket, source, receiver, report) in pending {
            match receiver.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.pending_drill_hole_loads.push((ticket, source, receiver, report));
                    continue;
                }
                Ok(Ok(loaded)) => {
                    self.finish_background_task(ticket, true);
                    self.add_loaded_drill_holes(loaded);
                }
                Ok(Err(error)) => {
                    userspace_warn!("{}", tr_format!(literal = "Failed to load drillholes: %error%", error = format!("{error:#}")));
                    self.finish_background_task(ticket, false);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    userspace_warn!("{}", tr_format!(literal = "Drillhole loader disconnected for %name%", name = source.display_name()));
                    self.finish_background_task(ticket, false);
                }
            }
            drop(report);
        }
    }

    pub(super) fn add_loaded_drill_holes(&mut self, loaded: LoadedDrillHoleDataset) {
        let id = DrillHoleId(self.next_drill_hole_id);
        self.next_drill_hole_id += 1;
        let name = crate::model::project::unique_item_name(loaded.name, self.drill_holes.iter().map(|item| item.name.as_str()));
        userspace_log!(
            "{}",
            tr_format!(
                literal = "Loaded drillhole dataset '%name%': %holes% holes, %fields% colour fields",
                name = name.clone(),
                holes = loaded.dataset.holes.len(),
                fields = loaded.dataset.fields.len()
            )
        );
        // More distinct strings than any dictionary holds is usually free text:
        // name the class and its extent, repair nothing.
        for field in &loaded.dataset.fields {
            if let DrillFieldKind::Categorical { categories } = &field.kind
                && categories.len() > WIDE_CATEGORY_FIELD_HINT
            {
                userspace_warn!(
                    "{}",
                    tr_format!(
                        literal = "Drillhole field '%label%' has %count% distinct codes, more than a coded field would typically have; it looks like free text rather than a categorical field, but every code is kept and coloured",
                        label = field.label.clone(),
                        count = categories.len()
                    )
                );
            }
        }
        self.drill_holes.push(OpenDrillHoleDataset {
            id,
            state: crate::model::project::ProjectItemState::dirty(MemberKind::DrillHole, Some(loaded.source.display_name())),
            name,
            dataset: loaded.dataset,
            color: DrillColorState::default(),
        });
        self.touch_active_project_content();
        self.persist_session();
        self.invalidate_topology_bounds_and_redraw();
    }

    /// Record a change to one dataset's colour state as an undo step.
    fn set_drill_hole_color(&mut self, id: DrillHoleId, change: impl FnOnce(&OpenDrillHoleDataset, &mut DrillColorState)) {
        let Some(dataset) = self.drill_holes.iter().find(|item| item.id == id) else {
            return;
        };
        let mut color = dataset.color.clone();
        change(dataset, &mut color);
        self.set_item_style(
            ItemRef::DrillHole(id),
            ItemStyle::DrillHole {
                loaded: dataset.state.loaded,
                color,
            },
        );
        self.request_topology_redraw();
    }

    pub(crate) fn set_drill_hole_width(&mut self, id: DrillHoleId, radius_scale: f64, min_pixel_diameter: f32) {
        // `clamp` passes NaN through, and a NaN radius renders nothing.
        let scale = if radius_scale.is_finite() {
            radius_scale.clamp(*crate::model::drill_hole::RADIUS_SCALE_RANGE.start(), *crate::model::drill_hole::RADIUS_SCALE_RANGE.end())
        } else {
            crate::model::drill_hole::default_radius_scale()
        };
        let floor = if min_pixel_diameter.is_finite() {
            min_pixel_diameter.clamp(
                *crate::model::drill_hole::MIN_PIXEL_DIAMETER_RANGE.start(),
                *crate::model::drill_hole::MIN_PIXEL_DIAMETER_RANGE.end(),
            )
        } else {
            crate::model::drill_hole::MIN_RENDER_PIXEL_DIAMETER
        };
        self.set_drill_hole_color(id, |_, color| {
            color.radius_scale = scale;
            color.min_pixel_diameter = floor;
        });
    }

    pub(crate) fn set_drill_hole_color_field(&mut self, id: DrillHoleId, field: Option<String>) {
        if self
            .drill_holes
            .iter()
            .find(|item| item.id == id)
            .is_none_or(|dataset| field.as_deref().is_some_and(|key| dataset.dataset.field(key).is_none()))
        {
            return;
        }
        self.set_drill_hole_color(id, |dataset, color| {
            color.active_field = field.clone();
            color.by_working_section = false;
            color.preset = DrillColorPreset::Rainbow;
            color.smooth = true;
            color.stops = DrillColorPreset::Rainbow.stops();
            // Switching field fills in what the new field adds and keeps every
            // colour already chosen; the dialog's reset asks for fresh ones.
            if let Some(field) = field.as_deref().and_then(|key| dataset.dataset.field(key)) {
                color.reconcile_categories(field);
            }
        });
    }

    pub(crate) fn set_drill_hole_color_preset(&mut self, id: DrillHoleId, preset: DrillColorPreset) {
        self.set_drill_hole_color(id, |_, color| {
            color.preset = preset;
            color.smooth = preset.smooth();
            color.stops = preset.stops();
        });
    }

    pub(crate) fn set_drill_hole_color_stops(&mut self, id: DrillHoleId, mut stops: Vec<DrillColorStop>) {
        stops.retain(|stop| stop.t.is_finite() && stop.color.iter().all(|value| value.is_finite()));
        stops.sort_by(|a, b| a.t.total_cmp(&b.t));
        for stop in &mut stops {
            stop.t = stop.t.clamp(0.0, 1.0);
        }
        if (2..=MAX_DRILL_COLOR_STOPS).contains(&stops.len()) {
            self.set_drill_hole_color(id, |_, color| color.stops = stops);
        }
    }

    pub(crate) fn set_drill_hole_category_colors(&mut self, id: DrillHoleId, categories: Vec<crate::model::drill_hole::DrillCategoryColor>) {
        // No cap on categories. A non-finite component is dropped, as a ramp
        // stop's is, so no NaN reaches the shader.
        let categories = categories.into_iter().filter(|category| category.color.iter().all(|value| value.is_finite())).collect();
        self.set_drill_hole_color(id, |_, color| color.set_categories(categories));
    }

    pub(crate) fn set_drill_hole_working_sections(&mut self, id: DrillHoleId, sections: Vec<crate::model::drill_hole::WorkingSection>) {
        let Some(dataset) = self.drill_holes.iter().find(|item| item.id == id) else {
            return;
        };
        let (sections, dropped) = crate::model::drill_hole::tidy_working_sections(sections, &dataset.dataset.fields);
        if !dropped.is_empty() {
            let reasons = dropped
                .iter()
                .map(|section| tr_format!(literal = "'%name%': %reason%", name = section.name.clone(), reason = section.problem.message()))
                .collect::<Vec<_>>()
                .join(" ");
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "Working sections not kept in '%dataset%'. %reasons%",
                    dataset = dataset.name.clone(),
                    reasons = reasons
                )
            );
        }
        // Nothing left to change is no undo step.
        if sections == dataset.color.working_sections {
            return;
        }
        self.set_drill_hole_color(id, |dataset, color| {
            color.working_sections = sections;
            // A field left with no sections has nothing to colour by.
            if color.by_working_section
                && !color
                    .active_field
                    .as_deref()
                    .is_some_and(|field| color.working_sections.iter().any(|section| section.field == field))
            {
                color.by_working_section = false;
            }
            if let Some(field) = color.active_field.as_deref().and_then(|key| dataset.dataset.field(key)) {
                color.reconcile_categories(field);
            }
        });
    }

    pub(crate) fn set_drill_hole_color_by_working_section(&mut self, id: DrillHoleId, field: String) {
        let valid = self.drill_holes.iter().find(|item| item.id == id).is_some_and(|dataset| {
            matches!(dataset.dataset.field(&field).map(|field| &field.kind), Some(DrillFieldKind::Categorical { .. }))
                && dataset.color.working_sections.iter().any(|section| section.field == field)
        });
        if !valid {
            return;
        }
        self.set_drill_hole_color(id, |dataset, color| {
            if color.active_field.as_deref() != Some(field.as_str()) {
                color.active_field = Some(field.clone());
                color.preset = DrillColorPreset::Rainbow;
                color.smooth = true;
                color.stops = DrillColorPreset::Rainbow.stops();
            }
            color.by_working_section = true;
            if let Some(field) = dataset.dataset.field(&field) {
                color.reconcile_categories(field);
            }
        });
    }

    pub(crate) fn close_drill_hole(&mut self, id: DrillHoleId) {
        self.set_item_loaded(crate::model::ItemRef::DrillHole(id), false);
    }

    pub(crate) fn release_drillhole_runtime(&mut self, id: DrillHoleId) {
        self.cancel_collar_move_touching(id);
        self.cancel_jobs(|key| *key == crate::app::jobs::JobKey::DrillHole(id));
        let entity = SceneEntityId::DrillHole(id);
        self.editor.selected_handles.remove(&entity);
        self.editor.hidden_handles.remove(&entity);
        self.editor.translucent_handles.remove(&entity);
        if self.editor.drill_hole_color_dialog == Some(id) {
            self.editor.drill_hole_color_dialog = None;
        }
        if self.editor.active_drill_hole == Some(id) {
            self.editor.active_drill_hole = None;
        }
        if self.editor.tie_anchor.is_some_and(|anchor| anchor.dataset == id) {
            self.editor.end_tie_chain();
        }
        self.editor.retain_drill_hole_datasets(|dataset| dataset != id);
        if self.editor.initiation_dialog.as_ref().is_some_and(|dialog| dialog.target.dataset == id) {
            self.editor.initiation_dialog = None;
        }
        self.invalidate_topology_bounds_and_redraw();
    }

    pub(crate) fn remove_drill_hole(&mut self, id: DrillHoleId) {
        self.cancel_collar_move_touching(id);
        self.cancel_jobs(|key| *key == crate::app::jobs::JobKey::DrillHole(id));
        let entity = SceneEntityId::DrillHole(id);
        self.editor.selected_handles.remove(&entity);
        self.editor.hidden_handles.remove(&entity);
        self.editor.explicitly_frozen.remove(&entity);
        self.editor.frozen_handles.remove(&entity);
        self.editor.translucent_handles.remove(&entity);
        if self.editor.active_drill_hole == Some(id) {
            self.editor.active_drill_hole = None;
        }
        if self.editor.tie_anchor.is_some_and(|anchor| anchor.dataset == id) {
            self.editor.end_tie_chain();
        }
        self.editor.retain_drill_hole_datasets(|dataset| dataset != id);
        if self.editor.initiation_dialog.as_ref().is_some_and(|dialog| dialog.target.dataset == id) {
            self.editor.initiation_dialog = None;
        }
        self.delete_project_item(ItemRef::DrillHole(id));
        self.request_topology_redraw();
    }

    /// Sends one hole to the borehole inspector, opening the panel if it is
    /// hidden. Ignores the inspector lock: this is an explicit request.
    pub(crate) fn inspect_drill_hole(&mut self, hole: DrillHoleRef) -> Result<()> {
        self.editor.inspected_hole = Some(hole);
        self.redraw_requested = true;
        // The one switch, shared with the menu and the Interface tab.
        if !self.editor.show_borehole_inspector
            && let Err(error) = self.toggle_view_option(crate::ui::state::ViewToggle::BoreholeInspector)
        {
            // A setting that will not save must not hold the panel shut.
            self.editor.show_borehole_inspector = true;
            userspace_warn!(
                "{}",
                tr_format!(literal = "Opened the Borehole Inspector, but could not save the setting: %error%", error = error)
            );
        }
        Ok(())
    }
}

impl<'a> App<'a> {
    /// Every hole the reference tools run on, each once: a hole picked in
    /// the viewport names itself, a dataset selected whole names all of its.
    /// A visitor, because the menus count this each frame.
    pub(crate) fn for_each_reference_hole(&self, mut visit: impl FnMut(DrillHoleRef)) {
        for dataset in self.drill_holes.iter().filter(|dataset| dataset.state.loaded) {
            if self.editor.selected_handles.contains(&SceneEntityId::DrillHole(dataset.id)) {
                for hole in 0..dataset.dataset.holes.len() {
                    visit(DrillHoleRef { dataset: dataset.id, hole });
                }
                continue;
            }
            // Holes picked one at a time, skipped above so a dataset named
            // both ways does not place two points on the one hole.
            for hole in self.editor.selected_drill_holes.iter().filter(|hole| hole.dataset == dataset.id) {
                if hole.hole < dataset.dataset.holes.len() {
                    visit(*hole);
                }
            }
        }
    }

    /// One reference point per hole for `value` in `field`, on `side`, as a
    /// new layer of points: the first step of a reference surface. Points are
    /// derived from the holes and never edited; running again makes another
    /// layer, so a corrected pick shows up as a new set beside the old.
    ///
    /// Runs on the holes the dialog was opened on, which may span datasets.
    pub(crate) fn build_reference_points(
        &mut self,
        holes: Vec<DrillHoleRef>,
        field: String,
        target: crate::model::drill_hole::ReferenceTarget,
        side: crate::model::drill_hole::ReferenceSide,
    ) {
        let value = target.name().to_owned();
        let mut picks = Vec::new();
        let mut absent = 0usize;
        let mut flagged: Vec<String> = Vec::new();
        let mut involved: Vec<&OpenDrillHoleDataset> = Vec::new();
        for reference in &holes {
            if !involved.iter().any(|dataset| dataset.id == reference.dataset)
                && let Some(dataset) = self.drill_holes.iter().find(|dataset| dataset.id == reference.dataset && dataset.state.loaded)
            {
                involved.push(dataset);
            }
        }
        let (codes_by_dataset, disagree) = reference_codes(&involved, &field, &target);
        if disagree {
            userspace_warn!(
                "{}",
                tr_format!(
                    literal = "Working section '%name%' is not the same in every selected dataset; each dataset's own was used.",
                    name = value.clone()
                )
            );
        }
        for reference in &holes {
            // Unloaded or removed under the open dialog: the hole is not
            // there to pick from, so it counts as one that gave nothing.
            let Some(dataset) = involved.iter().find(|dataset| dataset.id == reference.dataset) else {
                absent += 1;
                continue;
            };
            let Some(hole) = dataset.dataset.holes.get(reference.hole) else {
                absent += 1;
                continue;
            };
            let codes = codes_by_dataset.iter().find(|(id, _)| *id == dataset.id).map_or(&[][..], |(_, codes)| codes.as_slice());
            let pick = crate::model::drill_hole::reference_pick(hole, &field, codes, side);
            let Some(depth) = pick.depth else {
                absent += 1;
                continue;
            };
            if pick.runs > 1 {
                flagged.push(hole.dhid.clone());
            }
            match hole.position_at_depth(depth) {
                Some(position) => picks.push(position),
                None => absent += 1,
            }
        }
        if picks.is_empty() {
            userspace_warn!("{}", tr_format!(literal = "No hole holds '%value%' in that field", value = target.label()));
            return;
        }
        let Some(project) = self.workspace.active_project_mut() else {
            return;
        };
        let document = &mut project.project.document;
        let layer_id = document.allocate_layer_id();
        let layer = crate::model::Layer {
            id: layer_id,
            name: tr_format!(literal = "%value% %side%", value = value.clone(), side = side.label()),
            color_index: None,
            color: [1.0, 1.0, 1.0, 1.0],
            loaded: true,
            elevation: 0.0,
            folder: None,
            // Derived from the holes, so it is tagged for Modelling rather
            // than left where a hand-drawn layer lands.
            section: crate::model::SectionKind::Modelling,
        };
        let objects: Vec<crate::model::Object> = picks
            .iter()
            .map(|&pos| crate::model::Object::Point {
                id: document.allocate_object_id(),
                layer: layer_id,
                pos,
                color: crate::model::ObjectColor::ByLayer,
            })
            .collect();
        let used = objects.len();
        self.execute_edit(Command::AddLayerSnapshot { layer, objects });
        userspace_log!(
            "{}",
            tr_format!(
                literal = "Reference points: %used% holes placed, %absent% without '%value%', %flagged% flagged as possible fault repeats",
                used = used.to_string(),
                absent = absent.to_string(),
                value = value,
                flagged = flagged.len().to_string()
            )
        );
        if !flagged.is_empty() {
            userspace_warn!("{}", tr_format!(literal = "Uppermost run used, flagged: %holes%", holes = flagged.join(", ")));
        }
        self.invalidate_geometry();
    }
}

/// The codes a reference pick on `target` looks for in each dataset, found
/// once per dataset rather than per hole, and whether the datasets define
/// the section differently. A code stands for itself. A working section
/// stands for its codes as each dataset defines it; a dataset without one of
/// that name on `field` borrows the first definition among the others. A
/// section none of them defines picks nothing: its name is never looked
/// for as a code.
fn reference_codes(datasets: &[&OpenDrillHoleDataset], field: &str, target: &crate::model::drill_hole::ReferenceTarget) -> (Vec<(DrillHoleId, Vec<String>)>, bool) {
    let name = match target {
        crate::model::drill_hole::ReferenceTarget::Code(code) => return (datasets.iter().map(|dataset| (dataset.id, vec![code.clone()])).collect(), false),
        crate::model::drill_hole::ReferenceTarget::Section(name) => name,
    };
    let own = datasets
        .iter()
        .map(|dataset| dataset.color.working_section_named(field, name).map(|section| section.codes.as_slice()))
        .collect::<Vec<_>>();
    fn as_set(codes: &[String]) -> std::collections::BTreeSet<&str> {
        codes.iter().map(String::as_str).collect()
    }
    let first = own.iter().flatten().next().copied();
    let disagree = first.is_some_and(|first| own.iter().flatten().any(|codes| as_set(codes) != as_set(first)));
    let codes = datasets
        .iter()
        .zip(&own)
        .map(|(dataset, codes)| (dataset.id, codes.or(first).map(<[String]>::to_vec).unwrap_or_default()))
        .collect();
    (codes, disagree)
}
