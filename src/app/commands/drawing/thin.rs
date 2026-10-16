//! Thin Strings: drop the vertices a string does not need to keep its shape
//! within a tolerance, so a string draped on a rough surface stays easy to
//! edit. Distances are 3D, so a string on a section thins as one in plan.

use glam::DVec3;

use crate::{
    app::App,
    i18n::tr,
    logging::CommandReportSpec,
    model::{Command, Object, ObjectId, PolyVertex, kernel},
    ui::dialogs::ThinStringsDialog,
    userspace_warn,
};

/// The tolerance the dialog opens with until one is applied.
pub(crate) const DEFAULT_THIN_TOLERANCE: f64 = 0.1;

impl<'a> App<'a> {
    /// Open Thin Strings on every selected string the canvas may edit.
    pub(crate) fn open_thin_strings_dialog(&mut self) {
        let object_ids: Vec<ObjectId> = self.editable_polyline_sources(self.selected_polylines()).into_iter().map(|(id, _, _)| id).collect();
        if object_ids.is_empty() {
            userspace_warn!("{}", tr!("cmd-thin-select-strings"));
            return;
        }
        // Carried on from a dialog this one replaces, so the overlay still
        // sees a new preview.
        let preview_generation = self.editor.thin_strings_dialog.as_ref().map_or(0, |dialog| dialog.preview_generation);
        self.editor.thin_strings_dialog = Some(ThinStringsDialog {
            object_ids,
            tolerance: self.editor.thin_tolerance,
            slider_max: 0.0,
            before: 0,
            after: 0,
            preview: Vec::new(),
            preview_generation,
        });
        self.refresh_thin_preview(true);
    }

    /// Take a new tolerance from the dialog and show what it would leave.
    pub(crate) fn set_thin_tolerance(&mut self, tolerance: f64) {
        let Some(dialog) = self.editor.thin_strings_dialog.as_mut() else {
            return;
        };
        dialog.tolerance = tolerance;
        self.refresh_thin_preview(false);
    }

    /// Recompute the open dialog's preview: called when it opens, when its
    /// tolerance changes and after an undo or redo, never per frame. The
    /// slider's range is measured again only when `rescale` says the strings
    /// may have changed.
    pub(crate) fn refresh_thin_preview(&mut self, rescale: bool) {
        let Some(object_ids) = self.editor.thin_strings_dialog.as_ref().map(|dialog| dialog.object_ids.clone()) else {
            return;
        };
        let sources = self.editable_polyline_sources(object_ids);
        let Some(dialog) = self.editor.thin_strings_dialog.as_mut() else {
            return;
        };
        let tolerance = dialog.tolerance.max(0.0);
        dialog.preview = sources.iter().map(|(_, verts, closed)| (thin_vertices(verts, *closed, tolerance), *closed)).collect();
        dialog.before = sources.iter().map(|(_, verts, _)| verts.len()).sum();
        dialog.after = dialog.preview.iter().map(|(verts, _)| verts.len()).sum();
        if rescale {
            dialog.slider_max = sources.iter().map(|(_, verts, closed)| collapse_tolerance(verts, *closed)).fold(0.0, f64::max);
        }
        dialog.preview_generation = dialog.preview_generation.wrapping_add(1);
        self.redraw_requested = true;
    }

    /// Thin the strings in place as one undoable batch. Each is read again
    /// here rather than taken from the preview, so an edit made while the
    /// dialog was open is never overwritten.
    pub(crate) fn thin_strings(&mut self, object_ids: Vec<ObjectId>, tolerance: f64) {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return;
        }
        let sources = self.editable_polyline_sources(object_ids);
        let Some(document) = self.workspace.active_document() else {
            return;
        };
        let mut removed = 0usize;
        let commands: Vec<Command> = sources
            .iter()
            .filter_map(|(id, old_verts, closed)| {
                let thinned = thin_vertices(old_verts, *closed, tolerance);
                if thinned.len() == old_verts.len() {
                    return None;
                }
                let before = document.get_object(*id)?.clone();
                let mut after = before.clone();
                let Object::Polyline { verts, .. } = &mut after else {
                    return None;
                };
                removed += old_verts.len() - thinned.len();
                *verts = thinned;
                Some(Command::Replace { before, after })
            })
            .collect();
        if commands.is_empty() {
            userspace_warn!("{}", tr!("cmd-thin-nothing-removed", tolerance = tolerance.to_string()));
            return;
        }
        let count = commands.len();
        self.editor.thin_tolerance = tolerance;
        self.execute_edit(Command::Batch(commands));
        crate::logging::report_completed_action(
            CommandReportSpec::new(
                tr!("cmd-thin-thin-strings"),
                tr!("cmd-thin-count-removed", removed = removed.to_string(), count = count.to_string()),
            ),
            tr!("cmd-thin-thinned-count", count = count.to_string(), removed = removed.to_string()),
        );
        self.invalidate_geometry();
    }

    /// The strings among `object_ids` the canvas may edit, from the active
    /// project's document (the composite omits hidden objects).
    pub(super) fn editable_polyline_sources(&self, object_ids: Vec<ObjectId>) -> Vec<(ObjectId, Vec<PolyVertex>, bool)> {
        let Some(document) = self.workspace.active_document() else {
            return Vec::new();
        };
        object_ids
            .into_iter()
            .filter(|&id| self.editor.canvas_edits_object(document, id))
            .filter_map(|id| match document.get_object(id) {
                Some(Object::Polyline { verts, closed, .. }) => Some((id, verts.clone(), *closed)),
                _ => None,
            })
            .collect()
    }
}

/// The vertices of a string that stay at `tolerance`, by Douglas and
/// Peucker's method in 3D: a run between two kept vertices loses its inner
/// vertices when none lies `tolerance` or further from the straight line
/// joining its ends. Ends of an open string and both ends of every arc span
/// always stay; a ring keeps its start, the vertex farthest from it, and at
/// least three vertices. At 0 nothing goes.
pub(crate) fn thin_vertices(verts: &[PolyVertex], closed: bool, tolerance: f64) -> Vec<PolyVertex> {
    let Some((points, mut keep)) = pinned(verts, closed) else {
        return verts.to_vec();
    };
    let pins: Vec<usize> = (0..keep.len()).filter(|&index| keep[index]).collect();
    for pair in pins.windows(2) {
        let mut stack = vec![(pair[0], pair[1])];
        while let Some((first, last)) = stack.pop() {
            let Some((index, deviation)) = farthest(&points, first, last) else {
                continue;
            };
            if deviation < tolerance {
                continue;
            }
            keep[index] = true;
            stack.push((first, index));
            stack.push((index, last));
        }
    }
    let count = verts.len();
    if closed && (0..count).filter(|&index| keep[index]).count() < 3 {
        // Only the start and the far vertex are left: the ring keeps the
        // vertex farthest from the line between them, so it still encloses.
        let far = (1..count).find(|&index| keep[index]).unwrap_or(0);
        let widest = (1..count)
            .filter(|&index| index != far)
            .map(|index| (index, deviation_from(points[index], points[0], points[far])))
            .fold(
                None,
                |best: Option<(usize, f64)>, item| if best.is_none_or(|best| item.1 > best.1) { Some(item) } else { best },
            );
        if let Some((index, _)) = widest {
            keep[index] = true;
        }
    }
    verts.iter().zip(&keep).filter(|(_, kept)| **kept).map(|(vertex, _)| *vertex).collect()
}

/// The tolerance above which [`thin_vertices`] leaves `verts` as thin as it
/// gets: the largest deviation in any run between pinned vertices.
pub(crate) fn collapse_tolerance(verts: &[PolyVertex], closed: bool) -> f64 {
    let Some((points, keep)) = pinned(verts, closed) else {
        return 0.0;
    };
    let pins: Vec<usize> = (0..keep.len()).filter(|&index| keep[index]).collect();
    pins.windows(2)
        .filter_map(|pair| farthest(&points, pair[0], pair[1]))
        .map(|(_, deviation)| deviation)
        .fold(0.0, f64::max)
}

/// The positions to thin over and which of them can never go, or `None`
/// when the string has nothing to thin. A ring's positions end with its
/// start again, so its last span is a run like any other.
fn pinned(verts: &[PolyVertex], closed: bool) -> Option<(Vec<DVec3>, Vec<bool>)> {
    let count = verts.len();
    if count < 3 || (closed && count <= 3) {
        return None;
    }
    let mut points: Vec<DVec3> = verts.iter().map(|vertex| vertex.pos).collect();
    let spans = if closed { count } else { count - 1 };
    if closed {
        points.push(points[0]);
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    for span in 0..spans {
        if verts[span].bulge != 0.0 {
            keep[span] = true;
            keep[span + 1] = true;
        }
    }
    if closed {
        let start = points[0];
        let far = (1..count).max_by(|&a, &b| points[a].distance_squared(start).total_cmp(&points[b].distance_squared(start)))?;
        if points[far].distance_squared(start) == 0.0 {
            return None;
        }
        keep[far] = true;
    }
    Some((points, keep))
}

/// The inner vertex of the run `first..=last` farthest from the segment
/// joining its ends, the first of equals; `None` for a run with none inside.
fn farthest(points: &[DVec3], first: usize, last: usize) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64)> = None;
    for (index, &point) in points.iter().enumerate().take(last).skip(first + 1) {
        let deviation = deviation_from(point, points[first], points[last]);
        if best.is_none_or(|(_, most)| deviation > most) {
            best = Some((index, deviation));
        }
    }
    best
}

fn deviation_from(point: DVec3, a: DVec3, b: DVec3) -> f64 {
    point.distance(kernel::project_onto_segment_3d(point, a, b).0)
}
