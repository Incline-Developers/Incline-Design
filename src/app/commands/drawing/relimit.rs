use glam::DVec3;

use crate::{
    app::{App, PICK_THRESHOLD_PX},
    i18n::{tr, tr_format},
    logging::CommandReportSpec,
    model::{Command, Object, ObjectId, PolyVertex, SceneEntityId},
    ui::state::{ActiveTool, RelimitCandidate, RelimitMode, TrimEnd},
    userspace_warn,
};

impl<'a> App<'a> {
    pub(crate) fn open_relimit_dialog(&mut self) {
        // Relimiting modifies an existing object, so it isn't restricted to the
        // active layer (that restriction only matters for where *new* geometry
        // gets created).
        let object_id = self.editor.selected_handles.iter().find_map(|h| match h {
            SceneEntityId::Object(id) => {
                if matches!(self.active_document().get_object(*id), Some(Object::Polyline { closed: false, .. })) {
                    Some(*id)
                } else {
                    None
                }
            }
            _ => None,
        });
        if let Some(id) = object_id {
            self.editor.relimit_source_id = Some(id);
            self.editor.relimit_awaiting_source_pick = false;
            self.editor.relimit_dialog_open = true;
            self.editor.tool_highlight_id = None;
        } else {
            self.editor.relimit_awaiting_source_pick = true;
        }
    }

    /// Phase 0 pick: user clicked the source line while no dialog was open.
    fn pick_relimit_source(&mut self) {
        let frozen = &self.editor.frozen_handles;
        let picked = self
            .graphics
            .as_ref()
            .and_then(|g| g.pick_at_cursor(PICK_THRESHOLD_PX, &self.triangulations, &self.editor.hidden_handles, frozen, self.editor.xray_enabled));
        if let Some((SceneEntityId::Object(id), _)) = picked
            && matches!(self.active_document().get_object(id), Some(Object::Polyline { closed: false, .. }))
        {
            self.editor.relimit_source_id = Some(id);
            self.editor.relimit_awaiting_source_pick = false;
            self.editor.relimit_dialog_open = true;
            self.editor.tool_highlight_id = None;
            self.invalidate_geometry();
        }
    }

    pub(crate) fn relimit_line_click(&mut self) {
        if !self.editing_ready() {
            return;
        }

        // Phase 0: no source selected yet.
        if self.editor.relimit_awaiting_source_pick {
            self.pick_relimit_source();
            return;
        }

        // Phase 2: user clicks to confirm which end to move.
        if self.editor.relimit_confirming_end {
            self.commit_relimit_intersect();
            return;
        }

        // Phase 1: pick the target boundary (only when dialog is closed).
        if !self.editor.relimit_waiting_for_pick {
            userspace_warn!("{}", tr!(literal = "Relimit: click ignored, tool is not currently waiting for a target pick"));
            return;
        }
        let frozen = &self.editor.frozen_handles;
        let picked = self
            .graphics
            .as_ref()
            .and_then(|g| g.pick_at_cursor(PICK_THRESHOLD_PX, &self.triangulations, &self.editor.hidden_handles, frozen, self.editor.xray_enabled));
        let Some((SceneEntityId::Object(second_id), _)) = picked else {
            userspace_warn!("{}", tr!(literal = "Relimit: click did not hit any object (nothing under cursor)"));
            return;
        };
        let Some(source_id) = self.editor.relimit_source_id else {
            userspace_warn!("{}", tr!(literal = "Relimit: no source line is set, aborting pick"));
            return;
        };
        if second_id == source_id {
            userspace_warn!("{}", tr!(literal = "Relimit: clicked the source line itself, pick a different line"));
            return;
        }

        let world_candidates = self.relimit_target_candidates(second_id);
        if world_candidates.is_empty() {
            return;
        }

        let candidates: Vec<RelimitCandidate> = world_candidates
            .into_iter()
            .map(|candidate| RelimitCandidate {
                end: candidate.end,
                target: candidate.target,
                is_extension: candidate.is_extension,
            })
            .collect();

        self.editor.relimit_second_id = Some(second_id);
        self.editor.relimit_candidates = candidates;
        self.editor.relimit_waiting_for_pick = false;
        self.editor.tool_highlight_id = None;
        self.editor.relimit_confirming_end = true;
        self.invalidate_geometry();
        self.invalidate_overlay();
    }

    /// Use the same geometric eligibility for hover feedback and target clicks.
    fn relimit_target_candidates(&self, target_id: ObjectId) -> Vec<RelimitWorldCandidate> {
        let Some(source_id) = self.editor.relimit_source_id else {
            return Vec::new();
        };
        if target_id == source_id {
            return Vec::new();
        }
        let Some(Object::Polyline { verts, closed: false, .. }) = self.active_document().get_object(source_id) else {
            return Vec::new();
        };
        let target = match self.active_document().get_object(target_id) {
            Some(Object::Polyline { verts, closed, .. }) if verts.len() >= 2 => RelimitBoundary::Polyline {
                points: crate::model::geometry::tessellate_polyline_bulges(verts, *closed),
                closed: *closed,
            },
            Some(Object::Circle { center, radius, .. }) => RelimitBoundary::Circle { center: *center, radius: *radius },
            _ => return Vec::new(),
        };
        // Each end continues its own terminal segment, including its slope.
        relimit_world_candidates(verts, &target)
    }

    /// Highlight the object that the current Relimit pick phase would accept.
    pub(crate) fn update_relimit_hover_line(&mut self) {
        let picked = self.graphics.as_ref().and_then(|graphics| {
            graphics.pick_at_cursor(
                PICK_THRESHOLD_PX,
                &self.triangulations,
                &self.editor.hidden_handles,
                &self.editor.frozen_handles,
                self.editor.xray_enabled,
            )
        });
        let candidate = picked.and_then(|(entity, _)| match entity {
            SceneEntityId::Object(id)
                if self.editor.relimit_awaiting_source_pick && matches!(self.active_document().get_object(id), Some(Object::Polyline { closed: false, .. })) =>
            {
                Some(id)
            }
            SceneEntityId::Object(id) if self.editor.relimit_waiting_for_pick && !self.relimit_target_candidates(id).is_empty() => Some(id),
            _ => None,
        });
        if candidate != self.editor.tool_highlight_id {
            self.editor.tool_highlight_id = candidate;
            self.invalidate_geometry();
        }
    }

    fn commit_relimit_intersect(&mut self) {
        let Some(source_id) = self.editor.relimit_source_id else {
            return;
        };
        let before = match self.active_document().get_object(source_id) {
            Some(obj) => obj.clone(),
            None => return,
        };
        let (source_start, source_end) = match &before {
            Object::Polyline { verts, .. } if verts.len() >= 2 => (verts[0].pos, verts[verts.len() - 1].pos),
            _ => return,
        };
        let Some(cursor) = self.editor.cursor_screen_px else {
            return;
        };
        let Some(graphics) = self.graphics.as_ref() else {
            return;
        };
        let Some((candidate_index, _)) = crate::rendering::graphics::projections::projected_relimit_candidate_nearest_cursor(
            graphics.window_to_viewport_px(cursor),
            &self.editor.relimit_candidates,
            source_start,
            source_end,
            &graphics.view_proj(),
            graphics.screen_size_pub(),
        ) else {
            return;
        };
        let Some(candidate) = self.editor.relimit_candidates.get(candidate_index).copied() else {
            return;
        };

        let mut after = before.clone();
        if let Object::Polyline { verts, .. } = &mut after {
            match candidate.end {
                TrimEnd::Start => {
                    if let Some(v) = verts.first_mut() {
                        v.pos = candidate.target;
                    }
                }
                TrimEnd::End => {
                    if let Some(v) = verts.last_mut() {
                        v.pos = candidate.target;
                    }
                }
            }
        }

        let changed = if self.workspace.has_active_project() {
            self.execute_edit(Command::Replace { before, after });
            true
        } else {
            false
        };
        self.cancel_relimit();
        if changed {
            crate::logging::report_completed_action(
                CommandReportSpec::new(crate::i18n::tr!(literal = "Relimit Line"), format!("{source_id:?}")),
                tr_format!(literal = "Relimited line %source_id% to the selected target", source_id = format!("{source_id:?}")),
            );
        }
        self.invalidate_geometry();
    }

    pub(crate) fn relimit_resize(&mut self, source_id: ObjectId, mode: RelimitMode, value: f64) {
        let before = match self.active_document().get_object(source_id) {
            Some(obj @ Object::Polyline { .. }) => obj.clone(),
            _ => return,
        };
        let mut after = before.clone();
        let resize_end = self.editor.relimit_resize_end;
        if let Object::Polyline { verts, .. } = &mut after {
            let Some(position) = resized_terminal_position(verts, resize_end, mode, value) else {
                return;
            };
            match resize_end {
                TrimEnd::End => {
                    let last = verts.len() - 1;
                    verts[last].pos = position;
                }
                TrimEnd::Start => verts[0].pos = position,
            }
        }

        let changed = if self.workspace.has_active_project() {
            self.execute_edit(Command::Replace { before, after });
            true
        } else {
            false
        };
        self.cancel_relimit();
        if changed {
            crate::logging::report_completed_action(
                CommandReportSpec::new(crate::i18n::tr!(literal = "Relimit Line"), format!("{source_id:?}")),
                tr_format!(
                    literal = "Resized line %source_id% using %mode% value %value%",
                    source_id = format!("{source_id:?}"),
                    mode = format!("{mode:?}"),
                    value = value
                ),
            );
        }
        self.invalidate_geometry();
    }

    pub(crate) fn cancel_relimit(&mut self) {
        self.editor.relimit_confirming_end = false;
        self.editor.relimit_waiting_for_pick = false;
        self.editor.relimit_awaiting_source_pick = false;
        self.editor.relimit_dialog_open = false;
        self.editor.relimit_source_id = None;
        self.editor.relimit_second_id = None;
        self.editor.relimit_intersection_3d = None;
        self.editor.relimit_candidates.clear();
        self.editor.tool_highlight_id = None;
        self.editor.relimit_preview_from_px = None;
        self.editor.relimit_preview_to_px = None;
        self.editor.active_tool = ActiveTool::None;
        self.invalidate_geometry();
        self.invalidate_overlay();
    }
}

// -------------------------------------------------------------------------
// World-space (XY plane) line intersection helper (outside the impl)
// -------------------------------------------------------------------------

/// Intersect the *infinite* line through `a`,`b` with the *segment* `c`–`d`.
/// Returns the parameter `t` along `a`→`b` (any value), but only when the hit
/// lies on (or within `XY_TOL` metres of) the actual `c`–`d` segment - so a
/// polyline edge is never extended to an arbitrary off-edge point, while a
/// touch point that drifted off the end by floating-point noise (e.g. from a
/// prior relimit) still resolves.
fn line_line_intersect_t(a: glam::DVec2, b: glam::DVec2, c: glam::DVec2, d: glam::DVec2) -> Option<f64> {
    use crate::model::kernel;
    let (point, _) = kernel::line_segment(a, b - a, c, d, kernel::XY_TOL)?;
    let r = b - a;
    Some((point - a).dot(r) / r.length_squared())
}

#[derive(Clone, Copy, Debug)]
struct RelimitWorldCandidate {
    end: TrimEnd,
    target: DVec3,
    is_extension: bool,
}

enum RelimitBoundary {
    Polyline { points: Vec<DVec3>, closed: bool },
    Circle { center: DVec3, radius: f64 },
}

impl RelimitBoundary {
    fn crossings(&self, a: DVec3, b: DVec3) -> Vec<(f64, DVec3)> {
        let dir = (b - a).truncate();
        let length = dir.length();
        if !length.is_finite() || length * length <= f64::EPSILON {
            return Vec::new();
        }
        match self {
            Self::Polyline { points, closed } => {
                if points.len() < 2 {
                    return Vec::new();
                }
                let edge_count = if *closed { points.len() } else { points.len() - 1 };
                (0..edge_count)
                    .filter_map(|i| {
                        line_line_intersect_t(a.truncate(), b.truncate(), points[i].truncate(), points[(i + 1) % points.len()].truncate()).map(|t| (t, a + t * (b - a)))
                    })
                    .collect()
            }
            Self::Circle { center, radius } => {
                if !center.is_finite() || !radius.is_finite() || *radius <= 0.0 {
                    return Vec::new();
                }
                // Work relative to the source in XY, and retain its interpolated
                // elevation. Analytic intersections avoid tessellation error.
                let unit = dir / length;
                let offset = (*center - a).truncate();
                let along = offset.dot(unit);
                let distance = offset.perp_dot(unit).abs();
                let slack = 16.0 * f64::EPSILON * offset.length().max(*radius).max(1.0);
                if distance > radius + slack {
                    return Vec::new();
                }
                let half_chord = ((radius - distance).max(0.0) * (radius + distance)).sqrt();
                let hit = |distance: f64| {
                    let t = distance / length;
                    (t, a + t * (b - a))
                };
                if half_chord == 0.0 {
                    vec![hit(along)]
                } else {
                    vec![hit(along - half_chord), hit(along + half_chord)]
                }
            }
        }
    }
}

/// Find trim/extend candidates along the source's first and last segments.
fn relimit_world_candidates(source: &[PolyVertex], target: &RelimitBoundary) -> Vec<RelimitWorldCandidate> {
    if source.len() < 2 {
        return Vec::new();
    }
    let mut candidates = Vec::with_capacity(4);
    let start_a = source[0].pos;
    let start_b = source[1].pos;
    let start_touch = crate::model::kernel::XY_TOL / (start_b - start_a).truncate().length().max(crate::model::kernel::XY_TOL);
    let start_crossings = target.crossings(start_a, start_b);
    if let Some(&(_, target)) = start_crossings.iter().filter(|(t, _)| *t < -start_touch).max_by(|(a, _), (b, _)| a.total_cmp(b)) {
        candidates.push(RelimitWorldCandidate {
            end: TrimEnd::Start,
            target,
            is_extension: true,
        });
    }
    if let Some(&(_, target)) = start_crossings
        .iter()
        .filter(|(t, _)| *t >= -start_touch && *t < 1.0)
        .min_by(|(a, _), (b, _)| a.total_cmp(b))
    {
        candidates.push(RelimitWorldCandidate {
            end: TrimEnd::Start,
            target,
            is_extension: false,
        });
    }

    let last = source.len() - 1;
    let end_a = source[last - 1].pos;
    let end_b = source[last].pos;
    let end_touch = crate::model::kernel::XY_TOL / (end_b - end_a).truncate().length().max(crate::model::kernel::XY_TOL);
    let end_crossings = target.crossings(end_a, end_b);
    if let Some(&(_, target)) = end_crossings
        .iter()
        .filter(|(t, _)| *t > 0.0 && *t <= 1.0 + end_touch)
        .max_by(|(a, _), (b, _)| a.total_cmp(b))
    {
        candidates.push(RelimitWorldCandidate {
            end: TrimEnd::End,
            target,
            is_extension: false,
        });
    }
    if let Some(&(_, target)) = end_crossings.iter().filter(|(t, _)| *t > 1.0 + end_touch).min_by(|(a, _), (b, _)| a.total_cmp(b)) {
        candidates.push(RelimitWorldCandidate {
            end: TrimEnd::End,
            target,
            is_extension: true,
        });
    }

    candidates
}

fn resized_terminal_position(verts: &[PolyVertex], end: TrimEnd, mode: RelimitMode, value: f64) -> Option<DVec3> {
    if verts.len() < 2 || !value.is_finite() {
        return None;
    }
    let lengths: Vec<f64> = verts.windows(2).map(|pair| pair[0].pos.distance(pair[1].pos)).collect();
    if lengths.iter().any(|length| !length.is_finite()) {
        return None;
    }
    let current_length: f64 = lengths.iter().sum();
    let requested_length = match mode {
        RelimitMode::AbsoluteLength => value,
        RelimitMode::RelativeLength => current_length + value,
        RelimitMode::Intersect => return None,
    };
    let terminal_index = match end {
        TrimEnd::Start => 0,
        TrimEnd::End => lengths.len() - 1,
    };
    let fixed_length = current_length - lengths[terminal_index];
    let new_terminal_length = requested_length - fixed_length;
    if !new_terminal_length.is_finite() || new_terminal_length <= 1e-9 {
        return None;
    }

    match end {
        TrimEnd::Start => {
            let next = verts[1].pos;
            let outward = verts[0].pos - next;
            (outward.length() > 1e-9).then(|| next + outward.normalize() * new_terminal_length)
        }
        TrimEnd::End => {
            let previous = verts[verts.len() - 2].pos;
            let outward = verts[verts.len() - 1].pos - previous;
            (outward.length() > 1e-9).then(|| previous + outward.normalize() * new_terminal_length)
        }
    }
}
