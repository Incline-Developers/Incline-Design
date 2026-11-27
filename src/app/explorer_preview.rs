//! The explorer's hover card: a small picture of the row under the pointer,
//! turning slowly about it. See [`crate::ui::state::ExplorerPreview`].
//!
//! A loaded item the renderer draws as it is. An unloaded one is read in for
//! the card alone, held beside the project's items and never among them, so
//! the scene does not show it; and loading it while it is held takes the copy
//! rather than reading it again, so it loads at once. The copy is let go once
//! the pointer moves on. A design layer's strings are handed to the card,
//! which draws them itself.

use std::sync::Arc;

use glam::DVec3;

use crate::{
    app::{App, jobs::JobKey},
    i18n::tr,
    model::{ItemRef, LayerId, Object, OpenItem, block_model::BlockModelId, drill_hole::DrillHoleId, point_cloud::PointCloudId, triangulation::TriangulationId},
    ui::state::{ExplorerPreview, PreviewLine, PreviewStatus, PreviewSubject},
};

/// The card's copy's id, of each kind: clear of every id the project hands
/// out.
const PREVIEW_ID: u64 = u64::MAX - 2_000_000;

/// An unloaded item read in for the card, and the item it is a copy of.
pub(crate) struct PreviewCopy {
    pub(crate) source: ItemRef,
    /// Under [`PREVIEW_ID`], and loaded.
    pub(crate) item: OpenItem,
    /// When the card stopped showing it. It is kept a while after: a click
    /// closes the card, and the double-click that loads it, or the pointer
    /// coming back to it, should still find it.
    released: Option<web_time::Instant>,
}

/// An unloaded design layer's objects read in for the card, kept as
/// [`PreviewCopy`] keeps an item: loading the layer takes them.
pub(crate) struct HeldLayer {
    id: LayerId,
    /// The stored payload's, so a layer that changed since is read afresh.
    payload_hash: u64,
    payload: crate::model::layer_residency::LayerPayload,
    released: Option<web_time::Instant>,
}

/// How long a copy is kept after its card closes.
const KEEP_RELEASED: std::time::Duration = std::time::Duration::from_secs(30);

/// The most points a layer's card draws. Past it, every string keeps an
/// evenly spaced share of its own: the card is a few hundred pixels across.
const MOST_LINE_POINTS: usize = 20_000;

/// Sides to a circle on the card.
const CIRCLE_SIDES: usize = 32;

impl<'a> App<'a> {
    /// Settle the card on what the explorer last said is under the pointer.
    pub(crate) fn refresh_explorer_preview(&mut self) {
        let hovered = self.editor.explorer_preview.hovered.take();
        let expired = |released: Option<web_time::Instant>| released.is_some_and(|released| released.elapsed() > KEEP_RELEASED);
        if self.explorer_preview_item.as_ref().is_some_and(|held| expired(held.released)) {
            self.explorer_preview_item = None;
        }
        if self.explorer_preview_layer.as_ref().is_some_and(|held| expired(held.released)) {
            self.explorer_preview_layer = None;
        }
        if hovered == self.editor.explorer_preview.subject {
            // Loaded while the card is open: it shows it from then on.
            if let Some(subject) = hovered.filter(|_| self.editor.explorer_preview.status == PreviewStatus::NotLoaded) {
                self.settle_explorer_preview(subject);
            }
            return;
        }
        // The card has closed, or moved to another row: what it read in for
        // this one is kept a while yet.
        self.explorer_preview_generation += 1;
        self.editor.explorer_preview = ExplorerPreview::default();
        if let Some(held) = self.explorer_preview_item.as_mut() {
            held.released.get_or_insert_with(web_time::Instant::now);
        }
        if let Some(held) = self.explorer_preview_layer.as_mut() {
            held.released.get_or_insert_with(web_time::Instant::now);
        }
        let Some(subject) = hovered else {
            return;
        };
        let preview = &mut self.editor.explorer_preview;
        preview.subject = Some(subject);
        preview.started = Some(web_time::Instant::now());
        preview.background = crate::rendering::color::rgba_to_color32(self.editor.renderer_background_color);
        self.settle_explorer_preview(subject);
    }

    /// Let go of whatever the card showed.
    pub(crate) fn clear_explorer_preview(&mut self) {
        self.explorer_preview_generation += 1;
        self.explorer_preview_item = None;
        self.explorer_preview_layer = None;
        self.editor.explorer_preview = ExplorerPreview::default();
    }

    /// Work out what the card can show of `subject`, reading an unloaded
    /// surface in for it.
    fn settle_explorer_preview(&mut self, subject: PreviewSubject) {
        let (status, bounds, lines) = match subject {
            PreviewSubject::Layer(layer) => self.layer_preview(layer),
            PreviewSubject::Item(item) => {
                let (status, bounds) = self.item_preview(item);
                (status, bounds, Vec::new())
            }
        };
        let preview = &mut self.editor.explorer_preview;
        preview.status = status;
        preview.bounds = bounds;
        preview.lines = Arc::new(lines);
    }

    fn item_preview(&mut self, item: ItemRef) -> (PreviewStatus, Option<(DVec3, DVec3)>) {
        if let Some(held) = self.explorer_preview_item.as_mut().filter(|held| held.source == item) {
            held.released = None;
            let image = raster_image(&held.item);
            let bounds = item_bounds(&held.item);
            return self.shown(image, bounds);
        }
        let Some(copy) = self.clone_project_item(item) else {
            return (PreviewStatus::Idle, None);
        };
        if copy.state().loaded {
            return self.shown(raster_image(&copy), item_bounds(&copy));
        }
        // One copy at a time: they can be large.
        self.explorer_preview_item = None;
        if copy.state().deferred.is_none() {
            // Unloaded but still in memory: the card can have it as it is.
            return self.hold_explorer_preview(item, copy);
        }
        let generation = self.explorer_preview_generation;
        self.spawn_job_quietly(
            tr!("explorer-preview-loading"),
            vec![JobKey::ExplorerPreview(generation)],
            move |cancel| {
                if cancel.is_cancelled() {
                    anyhow::bail!(crate::app::jobs::Cancelled);
                }
                copy.materialize()
            },
            move |app, result| {
                if app.explorer_preview_generation != generation {
                    return;
                }
                match result {
                    Ok(copy) => {
                        let (status, bounds) = app.hold_explorer_preview(item, copy);
                        let preview = &mut app.editor.explorer_preview;
                        preview.status = status;
                        preview.bounds = bounds;
                    }
                    Err(error) if error.is::<crate::app::jobs::Cancelled>() => {}
                    Err(error) => app.editor.explorer_preview.status = PreviewStatus::Failed(format!("{error:#}")),
                }
                app.redraw_requested = true;
            },
        );
        (PreviewStatus::Loading, None)
    }

    /// Hold `copy`, read in from `source`, for the card: under an id of its
    /// own, loaded, so the renderer has it to draw; never among the
    /// project's items, so the scene does not.
    fn hold_explorer_preview(&mut self, source: ItemRef, mut copy: OpenItem) -> (PreviewStatus, Option<(DVec3, DVec3)>) {
        let bounds = item_bounds(&copy);
        let image = raster_image(&copy);
        set_item_id(&mut copy, preview_ref(source));
        copy.state_mut().loaded = true;
        self.explorer_preview_item = Some(PreviewCopy {
            source,
            item: copy,
            released: None,
        });
        self.shown(image, bounds)
    }

    /// What the card shows: a raster's image, flat, or the box the renderer
    /// frames anything else in.
    fn shown(&mut self, image: Option<Arc<egui::ColorImage>>, bounds: Option<(DVec3, DVec3)>) -> (PreviewStatus, Option<(DVec3, DVec3)>) {
        if let Some(image) = image {
            self.editor.explorer_preview.image = Some(image);
            return (PreviewStatus::Ready, None);
        }
        ready(bounds)
    }

    /// Load `item` from the card's copy, when the card read it in: it is in
    /// memory already, so there is nothing to wait for. Whether it did.
    pub(crate) fn hand_over_explorer_preview(&mut self, item: ItemRef) -> bool {
        let Some(current) = self.project_item_state(item).filter(|state| state.deferred.is_some()).map(|state| state.revision()) else {
            return false;
        };
        let held = self
            .explorer_preview_item
            .as_ref()
            .is_some_and(|held| held.source == item && held.item.state().revision() == current);
        let Some(PreviewCopy { mut item, source, .. }) = self.explorer_preview_item.take_if(|_| held) else {
            return false;
        };
        set_item_id(&mut item, source);
        // Its loaded flag is the project's to set, as the load goes on.
        item.state_mut().loaded = false;
        self.replace_residency(item);
        log::debug!("{source:?} loaded from the explorer preview's copy");
        true
    }

    /// A design layer's strings, points and circles as the card draws them.
    /// An unloaded one's are read in for the card, as an unloaded item is.
    fn layer_preview(&mut self, layer: LayerId) -> (PreviewStatus, Option<(DVec3, DVec3)>, Vec<PreviewLine>) {
        let Some(document) = self.workspace.active_document() else {
            return (PreviewStatus::Idle, None, Vec::new());
        };
        let Some(stored) = document.deferred_layers.get(&layer).cloned() else {
            return layer_lines(document, document.objects().iter().filter(|object| object.layer() == layer));
        };
        if let Some(held) = self
            .explorer_preview_layer
            .as_mut()
            .filter(|held| held.id == layer && held.payload_hash == stored.payload_hash)
        {
            held.released = None;
            return layer_lines(document, held.payload.objects.iter().map(|(object, _)| object));
        }
        // One copy at a time: they can be large.
        self.explorer_preview_layer = None;
        let generation = self.explorer_preview_generation;
        self.spawn_job_quietly(
            tr!("explorer-preview-loading"),
            vec![JobKey::ExplorerPreview(generation)],
            move |cancel| {
                if cancel.is_cancelled() {
                    anyhow::bail!(crate::app::jobs::Cancelled);
                }
                stored.read(layer).map(|payload| (stored.payload_hash, payload))
            },
            move |app, result| {
                if app.explorer_preview_generation != generation {
                    return;
                }
                match result {
                    Ok((payload_hash, payload)) => {
                        let Some(document) = app.workspace.active_document() else {
                            return;
                        };
                        let (status, bounds, lines) = layer_lines(document, payload.objects.iter().map(|(object, _)| object));
                        app.explorer_preview_layer = Some(HeldLayer {
                            id: layer,
                            payload_hash,
                            payload,
                            released: None,
                        });
                        let preview = &mut app.editor.explorer_preview;
                        preview.status = status;
                        preview.bounds = bounds;
                        preview.lines = Arc::new(lines);
                    }
                    Err(error) if error.is::<crate::app::jobs::Cancelled>() => {}
                    Err(error) => app.editor.explorer_preview.status = PreviewStatus::Failed(format!("{error:#}")),
                }
                app.redraw_requested = true;
            },
        );
        (PreviewStatus::Loading, None, Vec::new())
    }

    /// Load design layer `id` from the card's copy, when the card read it
    /// in. Whether it did.
    pub(crate) fn hand_over_explorer_preview_layer(&mut self, id: LayerId) -> bool {
        let Some(stored_hash) = self
            .workspace
            .active_document()
            .and_then(|document| document.deferred_layers.get(&id))
            .map(|stored| stored.payload_hash)
        else {
            return false;
        };
        let Some(held) = self.explorer_preview_layer.take_if(|held| held.id == id && held.payload_hash == stored_hash) else {
            return false;
        };
        let Some(project) = self.workspace.active_project_mut() else {
            return false;
        };
        project.project.document.restore_layer_payload(id, held.payload);
        log::debug!("layer {id:?} loaded from the explorer preview's copy");
        true
    }
}

/// The card's lines for a design layer's `objects`, coloured as `document`
/// colours them.
fn layer_lines<'object>(document: &crate::model::Document, objects: impl Iterator<Item = &'object Object>) -> (PreviewStatus, Option<(DVec3, DVec3)>, Vec<PreviewLine>) {
    let mut lines = Vec::new();
    for object in objects {
        let color = crate::rendering::color::rgba_to_color32(document.object_rgba(object));
        let mut push = |mut points: Vec<DVec3>, closed: bool| {
            if closed && points.len() > 2 {
                points.push(points[0]);
            }
            if !points.is_empty() {
                lines.push(PreviewLine { points, color });
            }
        };
        match object {
            Object::Point { pos, .. } | Object::Text { pos, .. } => push(vec![*pos], false),
            Object::Polyline { verts, closed, .. } => push(verts.iter().map(|vertex| vertex.pos).collect(), *closed),
            Object::Circle { center, radius, .. } => push(
                (0..CIRCLE_SIDES)
                    .map(|side| {
                        let angle = side as f64 * std::f64::consts::TAU / CIRCLE_SIDES as f64;
                        *center + DVec3::new(angle.cos(), angle.sin(), 0.0) * *radius
                    })
                    .collect(),
                true,
            ),
            Object::PitDesign { verts, .. } => {
                push(verts.iter().map(|vertex| vertex.pos).collect(), true);
                for string in object.pit_strings() {
                    push(string.points.clone(), string.closed);
                }
            }
        }
    }
    let bounds = lines.iter().flat_map(|line| &line.points).fold(None, |bounds: Option<(DVec3, DVec3)>, point| {
        Some(bounds.map_or((*point, *point), |(lower, upper)| (lower.min(*point), upper.max(*point))))
    });
    let Some(bounds) = bounds else {
        return (PreviewStatus::Empty, None, Vec::new());
    };
    thin_lines(&mut lines);
    (PreviewStatus::Ready, Some(bounds), lines)
}

/// What the card shows of something with `bounds`.
fn ready(bounds: Option<(DVec3, DVec3)>) -> (PreviewStatus, Option<(DVec3, DVec3)>) {
    match bounds {
        Some(bounds) => (PreviewStatus::Ready, Some(bounds)),
        None => (PreviewStatus::Empty, None),
    }
}

/// The box an item takes up, or `None` with nothing in it.
fn item_bounds(item: &OpenItem) -> Option<(DVec3, DVec3)> {
    match item {
        OpenItem::Triangulation(surface) => surface_bounds(&surface.mesh),
        OpenItem::BlockModel(model) => Some(model.world_bounds.unwrap_or((model.model.metadata.lower, model.model.metadata.upper))),
        OpenItem::DrillHole(holes) => holes.dataset.bounds,
        OpenItem::PointCloud(cloud) => (!cloud.points.is_empty()).then_some(cloud.bounds),
        OpenItem::Raster(_) => None,
    }
}

/// A raster's image as the card shows it, from the small copy every raster
/// keeps; `None` for anything else, or a raster with no pixels.
fn raster_image(item: &OpenItem) -> Option<Arc<egui::ColorImage>> {
    let OpenItem::Raster(raster) = item else {
        return None;
    };
    let [width, height] = raster.preview_size;
    let pixels = (width as usize) * (height as usize);
    (pixels > 0 && raster.rgba.len() >= pixels * 4).then(|| Arc::new(egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &raster.rgba[..pixels * 4])))
}

/// The card's copy of `source`'s kind.
fn preview_ref(source: ItemRef) -> ItemRef {
    match source {
        ItemRef::Triangulation(_) => ItemRef::Triangulation(TriangulationId(PREVIEW_ID)),
        ItemRef::BlockModel(_) => ItemRef::BlockModel(BlockModelId(PREVIEW_ID)),
        ItemRef::DrillHole(_) => ItemRef::DrillHole(DrillHoleId(PREVIEW_ID)),
        ItemRef::PointCloud(_) => ItemRef::PointCloud(PointCloudId(PREVIEW_ID)),
        ItemRef::Raster(id) => ItemRef::Raster(id),
    }
}

/// Give `item` the id `to` names, of its own kind.
pub(crate) fn set_item_id(item: &mut OpenItem, to: ItemRef) {
    match (item, to) {
        (OpenItem::Triangulation(item), ItemRef::Triangulation(id)) => item.id = id,
        (OpenItem::BlockModel(item), ItemRef::BlockModel(id)) => item.id = id,
        (OpenItem::DrillHole(item), ItemRef::DrillHole(id)) => item.id = id,
        (OpenItem::PointCloud(item), ItemRef::PointCloud(id)) => item.id = id,
        (OpenItem::Raster(item), ItemRef::Raster(id)) => item.id = id,
        _ => {}
    }
}

/// The box a surface takes up, or `None` with nothing in it.
fn surface_bounds(mesh: &crate::model::formats::mesh_data::Triangulation) -> Option<(DVec3, DVec3)> {
    (mesh.face_count() > 0).then(|| {
        let bounds = mesh.bounds();
        (DVec3::new(bounds.min.x, bounds.min.y, bounds.min.z), DVec3::new(bounds.max.x, bounds.max.y, bounds.max.z))
    })
}

/// Keep every `step`th point of each string, and its last, once the layer
/// has more than the card draws.
fn thin_lines(lines: &mut [PreviewLine]) {
    let total: usize = lines.iter().map(|line| line.points.len()).sum();
    if total <= MOST_LINE_POINTS {
        return;
    }
    let step = total.div_ceil(MOST_LINE_POINTS);
    for line in lines {
        if line.points.len() <= 2 {
            continue;
        }
        let last = line.points[line.points.len() - 1];
        let mut kept = line.points.iter().copied().step_by(step).collect::<Vec<_>>();
        if kept.last() != Some(&last) {
            kept.push(last);
        }
        line.points = kept;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crowded_layer_is_thinned_but_keeps_every_string_whole() {
        let string = |count: usize| PreviewLine {
            points: (0..count).map(|index| DVec3::new(index as f64, 0.0, 0.0)).collect(),
            color: egui::Color32::WHITE,
        };
        let mut lines = vec![string(MOST_LINE_POINTS), string(MOST_LINE_POINTS), string(2)];
        thin_lines(&mut lines);
        let total: usize = lines.iter().map(|line| line.points.len()).sum();
        assert!(total <= MOST_LINE_POINTS + lines.len(), "{total}");
        for line in &lines[..2] {
            assert_eq!(line.points.first().unwrap().x, 0.0);
            assert_eq!(line.points.last().unwrap().x, (MOST_LINE_POINTS - 1) as f64);
        }
        assert_eq!(lines[2].points.len(), 2);
    }
}
