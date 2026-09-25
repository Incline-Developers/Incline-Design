use anyhow::{Context, Result};

use crate::{
    app::App,
    i18n::{tr, tr_format},
    model::{
        Command, ItemRef, ItemStyle, MemberKind, OpenItem, SceneEntityId,
        drill_hole::{
            DrillColorPreset, DrillColorState, DrillColorStop, DrillFieldKind, DrillHole, DrillHoleDataset, DrillHoleId, DrillHoleRef, DrillHoleSource, DrillHoleStyle,
            LoadedDrillHoleDataset, MAX_DRILL_COLOR_STOPS, OpenDrillHoleDataset, OrientationSource, TraceStation, WIDE_CATEGORY_FIELD_HINT, clamp_disc_diameter,
            clamp_string_pixel_width,
        },
        formats::{csv_drill_hole, csv_geophysics::StreamControl},
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

/// A parsed bundle on its way to the App: the dataset, and the link to any
/// geophysics files it carried.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type LoadedBundle = (LoadedDrillHoleDataset, Option<crate::model::geophysics::GeophysicsLink>);

/// The key a bundle load runs under: one bundle for one project always gives
/// the same key, so a second import of it is refused while the first runs,
/// and a load that outlives its project is discarded.
pub(crate) fn drill_hole_load_key(source: &DrillHoleSource, runtime_id: u32) -> crate::app::jobs::JobKey {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    crate::app::jobs::JobKey::DrillHoleLoad {
        source: hasher.finish(),
        runtime_id,
    }
}

/// A load's result on the UI thread. The busy state lasts until the new
/// holes are uploaded to the GPU.
#[cfg(not(target_arch = "wasm32"))]
fn apply_loaded_bundle(app: &mut App, result: Result<LoadedBundle>) {
    match result {
        Ok(bundle) => {
            app.add_loaded_bundle(bundle);
            app.job_needs_gpu_upload();
        }
        Err(error) => userspace_warn!("{}", tr_format!(literal = "Failed to load drillholes: %error%", error = format!("{error:#}"))),
    }
}

/// A pattern's planned holes as a new set: vertical from each collar to
/// `depth`, all at the design `diameter`, drawn at that true diameter.
fn planned_dataset(id: DrillHoleId, name: String, collars: Vec<glam::DVec3>, depth: f64, diameter: f64) -> OpenDrillHoleDataset {
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
    OpenDrillHoleDataset {
        id,
        state: crate::model::project::ProjectItemState::dirty(MemberKind::DrillHole, None),
        name,
        dataset: std::sync::Arc::new(DrillHoleDataset::new(holes)),
        color: DrillColorState::for_planned_holes(),
        geophysics: None,
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

        let id = DrillHoleId(self.next_drill_hole_id);
        self.next_drill_hole_id += 1;
        let name = crate::model::project::unique_item_name(name.to_owned(), self.drill_holes.iter().map(|item| item.name.as_str()));
        let item = planned_dataset(id, name, collars, depth, diameter);
        self.execute_edit(Command::AddItem {
            item: ItemRef::DrillHole(id),
            index: self.drill_holes.len(),
            added: Some(OpenItem::DrillHole(Box::new(item))),
        });
        self.editor.active_drill_hole = Some(id);
        self.editor.close_drill_pattern();
        Ok(())
    }

    /// Reads a browser bundle's table files whole on the page thread (a
    /// table is small; its geophysics files, kept apart, are streamed once
    /// the dataset exists so gigabytes never sit in memory at once), then
    /// hands the bytes back through [`AppEvent::DrillHoleTablesRead`] to
    /// continue on the job queue in [`Self::continue_web_drill_hole_import`].
    /// The picked files are split by role here, once.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn import_web_drill_hole_source(&mut self, mut source: DrillHoleSource) -> Result<()> {
        let key = match self.drill_hole_load(&source) {
            Ok(Some(key)) => key,
            refused => {
                self.clear_browser_import_selection(crate::ui::state::DataMenu::CsvDrillHole);
                return refused.map(|_| ());
            }
        };
        let DrillHoleSource::Csv { files: mappings, .. } = &source else {
            self.clear_browser_import_selection(crate::ui::state::DataMenu::CsvDrillHole);
            anyhow::bail!("{}", tr!(literal = "Only mapped CSV bundles are imported in the browser"));
        };
        let picked = self.take_web_import_picked_files();
        if picked.len() != mappings.len() {
            self.clear_browser_import_selection(crate::ui::state::DataMenu::CsvDrillHole);
            anyhow::bail!("{}", tr!(literal = "Choose the drillhole source files again"));
        }
        let mut table_files = Vec::new();
        let mut geophysics = Vec::new();
        for (mapping, file) in mappings.iter().cloned().zip(picked) {
            if mapping.role == csv_drill_hole::CsvDrillFileRole::Geophysics {
                geophysics.push((mapping, file));
            } else {
                table_files.push((mapping, file));
            }
        }
        self.clear_browser_import_selection(crate::ui::state::DataMenu::CsvDrillHole);
        let display_path = crate::app::browser_source_filename(&source.display_name());
        remap_browser_source_path(&mut source, display_path);
        let proxy = self.web_event_loop_proxy.clone().context("browser event loop is unavailable")?;
        let (ticket, _progress) = self.begin_reported_task(tr_format!(literal = "Reading %name%", name = source.display_name()));
        let workspace = self.workspace_generation;
        wasm_bindgen_futures::spawn_local(async move {
            let mut tables = Vec::with_capacity(table_files.len());
            let mut failed = None;
            for (mapping, file) in table_files {
                match crate::model::input::read_browser_file(file).await {
                    Ok(input) => tables.push((mapping, input)),
                    Err(error) => {
                        failed = Some(error);
                        break;
                    }
                }
            }
            let result = if let Some(error) = failed { Err(error) } else { Ok(tables) };
            let _ = proxy.send_event(crate::app::AppEvent::DrillHoleTablesRead {
                key,
                source,
                geophysics,
                ticket,
                workspace,
                result,
            });
        });
        Ok(())
    }

    /// Continues a browser bundle import once its table files are read:
    /// parses them on the job queue, adds the dataset, then links any
    /// geophysics files to it, which needs the new dataset's id.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn continue_web_drill_hole_import(
        &mut self,
        key: crate::app::jobs::JobKey,
        source: DrillHoleSource,
        geophysics: Vec<(csv_drill_hole::CsvDrillFileMapping, web_sys::File)>,
        result: std::result::Result<Vec<(csv_drill_hole::CsvDrillFileMapping, crate::model::input::InputFile)>, String>,
    ) {
        let tables = match result {
            Ok(tables) => tables,
            Err(error) => {
                userspace_warn!("{}", tr_format!(literal = "Failed to load drillholes: %error%", error = error));
                return;
            }
        };
        // Only a CSV bundle gets this far: the import refuses any other.
        let DrillHoleSource::Csv { files: mappings, .. } = &source else { return };
        let mappings = mappings.clone();
        let label = tr_format!(literal = "Loading %name%", name = source.display_name());
        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<LoadedDrillHoleDataset> {
            let control = StreamControl {
                progress: &|fraction| progress.set_fraction(fraction),
                cancelled: &|| cancel.is_cancelled(),
            };
            let dataset = csv_drill_hole::parse_bundle_tables(&mappings, tables.iter().map(|(mapping, table)| (mapping, table.bytes.as_slice())), control)?;
            // The table bytes go before the apply step.
            drop(tables);
            Ok(LoadedDrillHoleDataset {
                name: source.display_name(),
                source,
                dataset: std::sync::Arc::new(dataset),
            })
        };
        let apply = move |app: &mut App, result: Result<LoadedDrillHoleDataset>| match result {
            Ok(loaded) => {
                let id = app.add_loaded_drill_holes(loaded);
                app.job_needs_gpu_upload();
                if !geophysics.is_empty() {
                    app.link_bundle_geophysics(id, geophysics);
                }
            }
            Err(error) => userspace_warn!("{}", tr_format!(literal = "Failed to load drillholes: %error%", error = format!("{error:#}"))),
        };
        self.spawn_job_reporting_progress(label, vec![key], compute, apply);
    }

    /// The key a load of `source` runs under, or `None` when the same bundle
    /// is already loading for this project.
    fn drill_hole_load(&self, source: &DrillHoleSource) -> Result<Option<crate::app::jobs::JobKey>> {
        let runtime_id = self
            .workspace
            .active_project()
            .map(|project| project.runtime_id)
            .context("Open a project before importing drillholes")?;
        let key = drill_hole_load_key(source, runtime_id);
        if self.job_pending(&key) {
            userspace_log!("{}", tr_format!(literal = "'%name%' is already loading", name = source.display_name()));
            return Ok(None);
        }
        Ok(Some(key))
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

    /// Parsed on the job queue, which cancels it when the project closes.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_drill_hole_source(&mut self, source: DrillHoleSource) -> Result<()> {
        let Some(key) = self.drill_hole_load(&source)? else {
            return Ok(());
        };
        let name = source.display_name();
        let label = tr_format!(literal = "Loading %name%", name = name.clone());
        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> Result<LoadedBundle> {
            let control = StreamControl {
                progress: &|fraction| progress.set_fraction(fraction),
                cancelled: &|| cancel.is_cancelled(),
            };
            let parsed = match &source {
                DrillHoleSource::LegacyDhd { .. } => anyhow::bail!("DHD drillhole sources are no longer supported"),
                DrillHoleSource::Csv { files, .. } => csv_drill_hole::parse_paths(files, control).with_context(|| "Failed to parse mapped drillhole CSV bundle")?,
                DrillHoleSource::Omf { .. } => anyhow::bail!("OMF drillhole data is loaded through the project importer"),
            };
            progress.set_fraction(1.0);
            let loaded = LoadedDrillHoleDataset {
                name,
                source,
                dataset: std::sync::Arc::new(parsed.dataset),
            };
            Ok((loaded, parsed.geophysics))
        };
        self.spawn_job_reporting_progress(label, vec![key], compute, apply_loaded_bundle);
        Ok(())
    }

    /// A CSV bundle's dataset, then the link to its geophysics files.
    #[cfg(not(target_arch = "wasm32"))]
    fn add_loaded_bundle(&mut self, (loaded, geophysics): LoadedBundle) {
        let id = self.add_loaded_drill_holes(loaded);
        if let Some(link) = geophysics {
            self.keep_geophysics_link(id, link);
        }
    }

    /// Add a loaded dataset under a fresh id, which is returned.
    pub(super) fn add_loaded_drill_holes(&mut self, loaded: LoadedDrillHoleDataset) -> DrillHoleId {
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
            color: DrillColorState::for_logged_holes(),
            geophysics: None,
        });
        self.touch_active_project_content();
        self.persist_session();
        self.invalidate_topology_bounds_and_redraw();
        id
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

    pub(crate) fn set_drill_hole_style(&mut self, id: DrillHoleId, style: DrillHoleStyle) {
        self.set_drill_hole_color(id, |_, color| {
            color.hole_style = style;
        });
    }

    pub(crate) fn set_drill_hole_discs(&mut self, id: DrillHoleId, disc_diameter: f64, string_pixel_width: f32) {
        let disc_diameter = clamp_disc_diameter(disc_diameter);
        let string_pixel_width = clamp_string_pixel_width(string_pixel_width);
        self.set_drill_hole_color(id, |_, color| {
            color.disc_diameter = disc_diameter;
            color.string_pixel_width = string_pixel_width;
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

    /// A closed dataset gives back the geophysics read from its linked files
    /// and the memory it held; the link stays with it.
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
            name: reference_layer_name(&target, side),
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
                value = target.label(),
                flagged = flagged.len().to_string()
            )
        );
        if !flagged.is_empty() {
            userspace_warn!("{}", tr_format!(literal = "Uppermost run used, flagged: %holes%", holes = flagged.join(", ")));
        }
        self.invalidate_geometry();
    }
}

/// The new layer's name for a reference pick. A working section says so, so
/// a section and a code of the same name (e.g. a section "COO" holding a
/// code also called "COO") never collide on the layer they build.
fn reference_layer_name(target: &crate::model::drill_hole::ReferenceTarget, side: crate::model::drill_hole::ReferenceSide) -> String {
    match target {
        crate::model::drill_hole::ReferenceTarget::Section(name) => {
            tr_format!(literal = "%name% working section %side%", name = name.clone(), side = side.label())
        }
        crate::model::drill_hole::ReferenceTarget::Code(name) => tr_format!(literal = "%name% %side%", name = name.clone(), side = side.label()),
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
