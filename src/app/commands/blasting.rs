//! The Solids page's Blasting step: dividing each bench into the shapes it is
//! blasted in.
//!
//! The step frames the 3D viewport rather than owning the window, so the
//! outlines are drawn with the same tools as any other design geometry. What
//! lives here is the part that is specific to blasting: owning the plan view
//! while the step is open, deriving each bench's outline from the slab the
//! Benching step already generated, and cutting that outline into shapes with
//! the lines the user has drawn across it.
//!
//! Cuts live in the owning bench's planning data. The drawing tools and
//! undo commands share the polyline representation, but the project design
//! object and layer collections never contain these primitives.
//! Blast faces are derived; names follow an interior anchor across edits.

use std::hash::{DefaultHasher, Hash, Hasher};

use glam::{DVec2, DVec3};

use crate::{
    model::{BlastShape, Command, LayerId, Object, SceneEntityId, SolidEdit, SolidId, arrangement},
    ui::state::{BenchSelection, BlastOutline, BlastShapeRef, BlastingRestore},
};

impl crate::app::App<'_> {
    /// Hold the camera in plan view for as long as the step is open, and put
    /// the user's own view back when it closes.
    pub(crate) fn sync_blasting_view(&mut self) {
        let active = self.editor.is_planning_cut_step();
        if active == self.editor.blasting_restore.is_some() {
            return;
        }
        if active {
            let Some(graphics) = self.graphics.as_mut() else {
                return;
            };
            let (forward, up) = graphics.camera_orientation();
            graphics.set_view_direction(DVec3::NEG_Z, DVec3::Y);
            self.editor.blasting_restore = Some(BlastingRestore {
                forward,
                up,
                layer: self.editor.active_layer,
                z_level: self.editor.z_level,
                z_input: self.editor.z_input,
            });
        } else {
            let Some(restore) = self.editor.blasting_restore.take() else {
                return;
            };
            // The step's tools belong to the step. Leaving with one still
            // armed would drop the next cut line on a bench that is no longer
            // on screen, and onto whatever layer the restore hands back.
            self.cancel_active_tool();
            self.editor.active_tool = crate::ui::state::ActiveTool::None;
            self.editor.selected_handles.clear();
            self.editor.tool_highlight_id = None;
            self.editor.active_layer = restore.layer;
            self.editor.z_level = restore.z_level;
            self.editor.z_input = restore.z_input;
            self.editor.blasting_bench_key = None;
            if let Some(graphics) = self.graphics.as_mut() {
                graphics.set_view_direction(restore.forward, restore.up);
            }
        }
        self.scene_document_key = None;
        self.invalidate_geometry();
    }

    /// Point the drawing tools at the selected bench: its cut layer, and its
    /// crest as the working elevation.
    ///
    /// Keyed on the bench, so the user can still pick another layer or type
    /// another Z without having it taken back from them next frame.
    pub(crate) fn sync_blasting_bench(&mut self) {
        if !self.editor.is_planning_cut_step() {
            self.editor.blasting_bench_key = None;
            self.editor.planning_cut_name = None;
            return;
        }
        let bench = self.editor.planning_cut_target();
        let mut hasher = DefaultHasher::new();
        self.workspace.active_project().map(|project| project.runtime_id).hash(&mut hasher);
        bench
            .map(|(solid, band)| (solid, band.base.to_bits(), band.top.to_bits(), band.is_flitch))
            .hash(&mut hasher);
        let key = hasher.finish();
        if self.editor.blasting_bench_key == Some(key) {
            return;
        }
        self.cancel_active_tool();
        self.editor.tool_highlight_id = None;
        self.editor.blasting_bench_key = Some(key);
        self.editor.selected_blast = None;
        self.editor.selected_dig_block = None;
        self.editor.selected_handles.clear();
        self.scene_document_key = None;
        self.invalidate_geometry();
        // A half-drawn cut belongs to the bench it was started on.
        if !self.editor.pending_stroke.is_empty() {
            self.discard_stroke();
        }
        let Some((solid, band)) = bench else {
            self.editor.active_layer = None;
            self.editor.planning_cut_name = None;
            return;
        };
        self.editor.planning_cut_name = self.workspace.active_document().and_then(|document| document.solid(solid)).map(|solid| {
            use crate::ui::elements::solids_view::{bench_path, flitch_path};
            if !band.is_flitch {
                return bench_path(&solid.name, band.base);
            }
            let bench = solid
                .benching
                .benches()
                .into_iter()
                .find(|bench| bench.contains_rl(band.base))
                .map_or(band.base, |bench| bench.base);
            flitch_path(&solid.name, bench, band.base)
        });
        self.editor.z_level = band.top;
        self.editor.z_input = band.top;
        self.editor.active_layer = self.bench_cut_layer(solid, band.base);
    }

    /// Open each cut step on something to draw on rather than an empty view:
    /// Blasting on a bench, Dig Strips on a flitch. A bench already picked
    /// carries over - Dig Strips opens on its top flitch, Blasting on the
    /// bench of a picked flitch - and otherwise the first solid's top bench is
    /// used, once Benching has said which benches hold any of it. A selection
    /// the step can already draw on is left alone.
    ///
    /// Run before the solid view syncs, as a click in the tree would be, so
    /// the ground is drawn and framed on the same frame it is picked.
    pub(crate) fn pick_entry_ground(&mut self) {
        let step = self.editor.is_planning_cut_step().then(|| self.editor.is_dig_strips_step());
        if step != self.planning_entry_step {
            self.planning_entry_step = step;
            self.planning_entry_pending = step.is_some();
        }
        if !self.planning_entry_pending {
            return;
        }
        let dig = step == Some(true);
        let selection = &self.editor.solids_view_selection;
        let keeps = self.editor.planning_cut_target().is_some() || (!dig && !selection.is_empty() && selection.iter().all(|row| row.band.is_none_or(|band| !band.is_flitch)));
        if keeps {
            self.planning_entry_pending = false;
            self.editor.solids_tree_reveal = true;
            return;
        }
        let Some(document) = self.workspace.active_document() else {
            self.planning_entry_pending = false;
            return;
        };
        let picked = selection.iter().find_map(|row| row.band.map(|band| (row.solid, band.base)));
        let Some(solid) = picked.and_then(|(solid, _)| document.solid(solid)).or_else(|| document.solids().first()) else {
            self.planning_entry_pending = false;
            return;
        };
        // Wait for the bands rather than give up: they arrive with the body.
        let Some(occupied) = self.editor.solid_view_bands.get(&solid.id) else {
            return;
        };
        self.planning_entry_pending = false;
        let holds = |base: f64, top: f64| crate::ui::elements::solids_view::holds(Some(occupied), base, top);
        let benches: Vec<_> = solid.benching.benches().into_iter().rev().filter(|bench| holds(bench.base, bench.top())).collect();
        let carried = picked
            .filter(|(id, _)| *id == solid.id)
            .and_then(|(_, base)| benches.iter().find(|bench| bench.contains_rl(base)));
        let Some(bench) = carried.or(benches.first()) else { return };
        let band = if dig {
            bench.flitches.iter().rev().find(|flitch| holds(flitch.base, flitch.top())).map(|flitch| BenchSelection {
                base: flitch.base,
                top: flitch.top(),
                is_flitch: true,
            })
        } else {
            Some(BenchSelection {
                base: bench.base,
                top: bench.top(),
                is_flitch: false,
            })
        };
        if let Some(band) = band {
            self.editor.solids_view_selection = vec![crate::ui::state::SolidsViewRow {
                solid: solid.id,
                band: Some(band),
            }];
            self.editor.selected_blast = None;
            self.editor.solids_tree_reveal = true;
        }
    }

    /// This bench's cut layer, if it has one that still exists.
    fn bench_cut_layer(&self, solid: SolidId, base: f64) -> Option<LayerId> {
        let document = self.workspace.active_document()?;
        let layer = document.solid(solid)?.blasting.drawing(base, self.editor.is_dig_strips_step())?.planning_layer.as_ref()?.id;
        document.layer(layer).map(|layer| layer.id)
    }

    /// The layer this bench's cut lines go on, created if this is the first
    /// tool armed on it.
    ///
    /// Deliberately lazy. Creating a layer is a document edit and an undo
    /// entry, and looking through a pit's benches to see how they came out is
    /// not an edit - so nothing is created until the user reaches for a tool.
    /// Copy the selected lines of a planning cut step: blast lines on a
    /// bench in Blasting, strips on a flitch in Dig Strips.
    pub(crate) fn copy_planning_cuts(&mut self) {
        if !self.editor.is_planning_cut_step() {
            return;
        }
        let lines = self
            .scene_document
            .objects()
            .iter()
            .filter(|object| self.editor.selected_handles.contains(&SceneEntityId::Object(object.id())))
            .filter(|object| matches!(object, Object::Polyline { .. }))
            .cloned()
            .collect();
        self.editor.cut_clipboard = (self.editor.is_dig_strips_step(), lines);
        self.redraw_requested = true;
    }

    /// Paste copied lines onto the bench or flitch selected in the step they
    /// were copied from, at its top, and select them.
    pub(crate) fn paste_planning_cuts(&mut self) {
        let (strips, lines) = &self.editor.cut_clipboard;
        if !self.editor.is_planning_cut_step() || *strips != self.editor.is_dig_strips_step() || lines.is_empty() {
            return;
        }
        let Some((_, band)) = self.editor.planning_cut_target() else { return };
        let Some(layer) = self.ensure_bench_cut_layer() else { return };
        let mut commands = Vec::new();
        let mut selected = std::collections::HashSet::new();
        let Some(document) = self.workspace.active_document_mut() else { return };
        for source in &self.editor.cut_clipboard.1 {
            let id = document.allocate_object_id();
            let mut object = source.with_id_and_layer(id, layer);
            if let Object::Polyline { verts, .. } = &mut object {
                for vertex in verts {
                    vertex.pos.z = band.top;
                }
            }
            selected.insert(SceneEntityId::Object(id));
            commands.push(Command::AddObject(object));
        }
        self.execute_edit(Command::Batch(commands));
        self.editor.selected_handles = selected;
        self.invalidate_geometry();
    }

    pub(crate) fn ensure_bench_cut_layer(&mut self) -> Option<LayerId> {
        let (solid, band) = self.editor.planning_cut_target()?;
        if let Some(layer) = self.bench_cut_layer(solid, band.base) {
            self.editor.active_layer = Some(layer);
            return Some(layer);
        }
        let layer = self.workspace.active_project_mut()?.project.document.allocate_layer_id();
        let mut plan = self.workspace.active_document()?.solid(solid)?.blasting.clone();
        plan.drawing_mut(band.base, self.editor.is_dig_strips_step()).planning_layer = Some(crate::model::Layer {
            id: layer,
            name: if self.editor.is_dig_strips_step() {
                crate::i18n::tr!("planning-dig-strips")
            } else {
                crate::i18n::tr!("planning-blast-cuts")
            },
            color_index: None,
            color: self.editor.tool_line_color,
            loaded: true,
            elevation: band.top as f32,
            folder: None,
            section: crate::model::SectionKind::natural_layer(),
        });
        self.update_solid(solid, SolidEdit::Blasting(plan));
        self.editor.active_layer = Some(layer);
        self.editor.blasting_outlines_key = None;
        Some(layer)
    }

    /// Frame the benches that are selected, once per change of selection.
    ///
    /// Framed on the bench slabs alone rather than the whole scene: the
    /// project's other geometry is still loaded, and fitting it too would
    /// leave the bench a speck in the middle of the pit.
    pub(crate) fn sync_blasting_frame(&mut self) {
        if !self.editor.is_planning_cut_step() {
            self.editor.blasting_framed_key = None;
            return;
        }
        let mut hasher = DefaultHasher::new();
        for row in &self.editor.solids_view_selection {
            row.solid.hash(&mut hasher);
            row.band.map(|band| (band.base.to_bits(), band.top.to_bits())).hash(&mut hasher);
        }
        let key = hasher.finish();
        if self.editor.blasting_framed_key == Some(key) {
            return;
        }
        // Wait for the geometry: framing an empty body would do nothing and
        // then never be retried for this selection.
        let mut bounds: Option<(DVec3, DVec3)> = None;
        for mesh in &self.solid_view_body {
            let box_ = mesh.mesh.bounds();
            let (low, high) = (DVec3::new(box_.min.x, box_.min.y, box_.min.z), DVec3::new(box_.max.x, box_.max.y, box_.max.z));
            bounds = Some(bounds.map_or((low, high), |(min, max)| (min.min(low), max.max(high))));
        }
        let Some((min, max)) = bounds else { return };
        // Nor while the swing into plan view is still running: it holds the
        // camera on the target it started from until it lands, so a frame
        // taken now would be undone.
        if self.graphics.as_ref().is_some_and(|graphics| graphics.view_transition_running()) {
            return;
        }
        self.editor.blasting_framed_key = Some(key);
        let document = &self.scene_document;
        let triangulations = &self.solid_view_body;
        if let Some(graphics) = self.graphics.as_mut() {
            graphics.frame_bounds(
                min,
                max,
                document,
                triangulations,
                &self.block_models,
                &self.drill_holes,
                &self.point_clouds,
                &self.editor.hidden_handles,
            );
            self.redraw_requested = true;
        }
    }

    /// Show the blast shapes of every bench the selection covers, named from
    /// the stored plan.
    ///
    /// Display only. Faces come from the geometry artifact rather than being
    /// re-traced here, and a face with no stored name is shown under the
    /// number it would be given rather than being given one: opening a page
    /// must not edit the project. Committing names is the Blasting stage's
    /// job - see [`crate::app::App::commit_blast_names`].
    pub(crate) fn sync_blasting_outlines(&mut self) {
        if !self.editor.is_planning_cut_step() && !self.editor.is_solids_view() {
            if self.editor.blasting_outlines_key.is_some() {
                self.editor.blasting_outlines.clear();
                self.editor.blasting_outlines_key = None;
                self.invalidate_overlay();
            }
            return;
        }
        let Some(project) = self.workspace.active_project() else {
            return;
        };
        let runtime = project.runtime_id;
        let solids = self.workspace.active_document().map(|document| document.solids().to_vec()).unwrap_or_default();

        let mut hasher = DefaultHasher::new();
        runtime.hash(&mut hasher);
        // Solid and cut edits both bump the document revision, so this stands
        // in for the plan contents without re-serializing them every redraw.
        self.workspace.active_document().map(crate::model::Document::revision).hash(&mut hasher);
        self.editor.is_blasting_step().hash(&mut hasher);
        for row in &self.editor.solids_view_selection {
            row.solid.hash(&mut hasher);
            row.band.map(|band| (band.base.to_bits(), band.top.to_bits(), band.is_flitch)).hash(&mut hasher);
        }
        for solid in &solids {
            solid.id.hash(&mut hasher);
            self.solid_view_cache
                .get(&solid.id)
                .map(|cache| (cache.key, cache.built_through(crate::app::planning_pipeline::GeometryDemand::Blasting)))
                .hash(&mut hasher);
        }
        let key = hasher.finish();
        if self.editor.blasting_outlines_key == Some(key) {
            return;
        }
        self.editor.blasting_outlines_key = Some(key);

        let previous_selection = self.editor.selected_blast;
        let mut next_selection = None;
        let mut outlines = Vec::new();
        for solid in &solids {
            // The blast faces alone: this derivation is the Blasting stage's
            // own, and must not wait on the dig blocks cut out of them.
            let Some(faces) = self.solid_view_cache.get(&solid.id).and_then(|cache| cache.blast_faces()) else {
                continue;
            };
            for (bench_base, faces) in group_faces_by_bench(faces) {
                let band = BenchSelection {
                    base: bench_base,
                    top: faces[0].bench.top,
                    is_flitch: false,
                };
                if self.editor.is_blasting_step() && !super::solids_view::selected(&self.editor.solids_view_selection, solid.id, Some(band)) {
                    continue;
                }
                let stored = solid.blasting.bench(bench_base);
                // Numbers already spoken for by a stored name, so a provisional
                // number never collides with one the project has committed.
                let mut used: Vec<u64> = stored
                    .map(|entry| entry.blasts.iter().filter_map(|blast| blast.name.parse().ok()).collect())
                    .unwrap_or_default();
                for face in faces {
                    let name = if solid.exclusions.blast_too_small(face.area) {
                        String::new()
                    } else {
                        match stored.and_then(|entry| entry.blasts.iter().find(|blast| arrangement::point_in_face(&face.face, DVec2::from(blast.anchor)))) {
                            Some(blast) => blast.name.clone(),
                            None => {
                                let number = lowest_unused(&mut used);
                                number.to_string()
                            }
                        }
                    };
                    if let Some(selected) = previous_selection
                        && selected.solid == solid.id
                        && (selected.bench_base() - bench_base).abs() < 1e-6
                        && arrangement::point_in_face(&face.face, DVec2::from(selected.anchor()))
                    {
                        next_selection = Some(face.shape_ref(solid.id));
                    }
                    outlines.push(BlastOutline {
                        solid: solid.id,
                        bench_base,
                        plane: face.bench.top,
                        name,
                        anchor: face.anchor,
                        area: face.area,
                        blast: None,
                        excluded: solid.exclusions.blast_face_excluded(bench_base, &face.face),
                        rings: face
                            .face
                            .iter()
                            .map(|ring| ring.iter().map(|point| DVec3::new(point.x, point.y, face.bench.top)).collect())
                            .collect(),
                    });
                }
            }
        }

        // Highest bench first, and within a bench by number where the names
        // are numbers - so 10 follows 9 rather than 1. A name that is not a
        // number sorts after every one that is, then alphabetically.
        let number = |name: &str| name.parse::<u64>().unwrap_or(u64::MAX);
        outlines.sort_by(|a, b| {
            b.bench_base
                .total_cmp(&a.bench_base)
                .then_with(|| number(&a.name).cmp(&number(&b.name)))
                .then_with(|| a.name.cmp(&b.name))
        });
        self.editor.selected_blast = next_selection.or_else(|| {
            previous_selection.filter(|selected| {
                self.solid_view_cache
                    .get(&selected.solid)
                    .is_some_and(|cache| !cache.built_through(crate::app::planning_pipeline::GeometryDemand::Blasting))
            })
        });
        if self.editor.blasting_outlines != outlines {
            self.editor.blasting_outlines = outlines;
            self.invalidate_overlay();
        }
    }

    /// Commit the blast names the current geometry implies into the project.
    ///
    /// This is the document-writing half of the Blasting stage, and runs only
    /// when the stage is run. Ground that is gone takes its name with it,
    /// which is what frees a number again, so undoing a cut and redrawing it
    /// gives 2 back rather than 3. Where two shapes merge into one, the
    /// survivor keeps the first of the two names.
    ///
    /// Returns the number of solids whose plan changed.
    pub(crate) fn commit_blast_names(&mut self) -> usize {
        let solids = self.workspace.active_document().map(|document| document.solids().to_vec()).unwrap_or_default();
        let mut named = Vec::new();
        for solid in &solids {
            // Committing names is the Blasting stage's own work, so it reads
            // the blast faces it just built. Asking for a finished partition
            // here left the commit silently doing nothing whenever the strips
            // below it had been retired - which is every run of this step.
            let Some(faces) = self.solid_view_cache.get(&solid.id).and_then(|cache| cache.blast_faces()) else {
                continue;
            };
            let mut plan = solid.blasting.clone();
            let mut plan_changed = false;
            let mut exclusions = solid.exclusions.clone();
            for (bench_base, faces) in group_faces_by_bench(faces) {
                // An excluded blast follows its face the way its name does,
                // so the list and the menu can still address it exactly.
                for entry in exclusions.blasts.iter_mut().filter(|entry| (entry.base - bench_base).abs() < 1e-6) {
                    if let Some(face) = faces.iter().find(|face| arrangement::point_in_face(&face.face, DVec2::from(entry.anchor))) {
                        entry.anchor = face.anchor;
                    }
                }
                let entry = plan.bench_mut(bench_base);
                let before = entry.blasts.len();
                entry
                    .blasts
                    .retain(|blast| faces.iter().any(|face| arrangement::point_in_face(&face.face, DVec2::from(blast.anchor))));
                plan_changed |= entry.blasts.len() != before;
                let mut surviving_anchors = Vec::new();
                // A blast under the minimum size is never mined, so it takes
                // no name - and no number from the blasts that are.
                for face in faces.iter().filter(|face| !exclusions.blast_too_small(face.area)) {
                    surviving_anchors.push(face.anchor);
                    match entry.blasts.iter().position(|blast| arrangement::point_in_face(&face.face, DVec2::from(blast.anchor))) {
                        // Re-anchor: the face may have moved under the name
                        // since it was last stored.
                        Some(index) if entry.blasts[index].anchor != face.anchor => {
                            entry.blasts[index].anchor = face.anchor;
                            plan_changed = true;
                        }
                        Some(_) => {}
                        None => {
                            let name = entry.next_number().to_string();
                            entry.blasts.push(BlastShape { name, anchor: face.anchor });
                            plan_changed = true;
                        }
                    }
                }
                let before = entry.blasts.len();
                entry.blasts.retain(|blast| surviving_anchors.contains(&blast.anchor));
                plan_changed |= before != entry.blasts.len();
            }
            if plan_changed {
                named.push((solid.id, SolidEdit::Blasting(plan)));
            }
            if exclusions != solid.exclusions {
                named.push((solid.id, SolidEdit::Exclusions(exclusions)));
            }
        }
        let count = named.len();
        for (solid, edit) in named {
            self.update_solid(solid, edit);
        }
        if count > 0 {
            self.editor.blasting_outlines_key = None;
            self.redraw_requested = true;
        }
        count
    }
}

/// The blast faces of each bench, highest bench first.
fn group_faces_by_bench(faces: &[super::solids_view::BlastFace]) -> Vec<(f64, Vec<&super::solids_view::BlastFace>)> {
    let mut benches: Vec<(f64, Vec<&super::solids_view::BlastFace>)> = Vec::new();
    for face in faces {
        match benches.iter_mut().find(|(base, _)| (base - face.bench.base).abs() < 1e-6) {
            Some((_, group)) => group.push(face),
            None => benches.push((face.bench.base, vec![face])),
        }
    }
    benches.sort_by(|a, b| b.0.total_cmp(&a.0));
    benches
}

/// The lowest number not already taken, marking it taken.
fn lowest_unused(used: &mut Vec<u64>) -> u64 {
    let mut next = 1;
    let mut sorted = used.clone();
    sorted.sort_unstable();
    for value in sorted {
        match value.cmp(&next) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => next += 1,
            std::cmp::Ordering::Greater => break,
        }
    }
    used.push(next);
    next
}

impl crate::app::App<'_> {
    /// The stored name of one blast, or `None` once its ground is gone.
    pub(crate) fn blast_name(&self, blast: BlastShapeRef) -> Option<String> {
        self.stored_blast(blast).map(|stored| stored.name.clone())
    }

    fn stored_blast(&self, blast: BlastShapeRef) -> Option<&BlastShape> {
        self.workspace
            .active_document()?
            .solid(blast.solid)?
            .blasting
            .bench(blast.bench_base())?
            .blasts
            .iter()
            .find(|stored| stored.anchor == blast.anchor())
    }

    /// Rename one blast, refusing a number its bench is already using.
    ///
    /// Deliberately not routed through `unique_item_name` the way the other
    /// in-place renames are: silently turning a second 4269 into "4269 (2)"
    /// would hand the user a blast number that is not a blast number.
    pub(crate) fn rename_blast(&mut self, blast: BlastShapeRef, new_name: String) {
        let new_name = new_name.trim().to_owned();
        if new_name.is_empty() {
            return;
        }
        let Some(solid) = self.workspace.active_document().and_then(|document| document.solid(blast.solid)) else {
            return;
        };
        let mut plan = solid.blasting.clone();
        let Some(bench) = plan.benches.iter_mut().find(|bench| (bench.base - blast.bench_base()).abs() <= 1e-6) else {
            return;
        };
        if bench.blasts.iter().any(|stored| stored.name == new_name && stored.anchor != blast.anchor()) {
            crate::userspace_warn!(
                "{}",
                crate::i18n::tr!(
                    "planning-blast-name-taken",
                    blast = crate::ui::elements::solids_view::blast_path(&solid.name, blast.bench_base(), &new_name)
                )
            );
            return;
        }
        let Some(stored) = bench.blasts.iter_mut().find(|stored| stored.anchor == blast.anchor()) else {
            return;
        };
        if stored.name == new_name {
            return;
        }
        stored.name = new_name;
        self.update_solid(blast.solid, SolidEdit::Blasting(plan));
        self.editor.blasting_outlines_key = None;
    }

    /// Drop a blast's stored name so it is numbered automatically again.
    pub(crate) fn reset_blast_name(&mut self, blast: BlastShapeRef) {
        let Some(solid) = self.workspace.active_document().and_then(|document| document.solid(blast.solid)) else {
            return;
        };
        let mut plan = solid.blasting.clone();
        let Some(bench) = plan.benches.iter_mut().find(|bench| (bench.base - blast.bench_base()).abs() <= 1e-6) else {
            return;
        };
        let before = bench.blasts.len();
        bench.blasts.retain(|stored| stored.anchor != blast.anchor());
        if bench.blasts.len() == before {
            return;
        }
        self.update_solid(blast.solid, SolidEdit::Blasting(plan));
        self.editor.blasting_outlines_key = None;
    }
}
