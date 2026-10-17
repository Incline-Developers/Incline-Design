use glam::DVec3;

use crate::{
    app::App,
    logging::CommandReportSpec,
    model::{Command, Object, ObjectId, PolyVertex, SceneEntityId},
    ui::state::ActiveTool,
};

impl<'a> App<'a> {
    pub(crate) fn explode_at_cursor(&mut self) {
        if !self.editing_ready() {
            return;
        }
        let picked = self.pick_under_cursor();
        let Some((SceneEntityId::Object(object_id), _)) = picked else {
            return;
        };
        if !self.activate_project_for_object(object_id) {
            return;
        }
        // A circle is one shape, not a ring of segments. Its geometry view
        // happens to be two semicircular arcs, but that is an encoding detail
        // and not something to hand the user as the result of a tool.
        if matches!(self.active_document().get_object(object_id), Some(Object::Circle { .. })) {
            return;
        }
        self.editor.tool_highlight_id = None;
        if self.editor.active_tool == ActiveTool::ExplodePolylineToPoints {
            self.explode_polyline_to_points(object_id);
        } else {
            self.explode_polyline(object_id);
        }
    }

    /// Update the hover highlight for the Explode tool on cursor move.
    pub(crate) fn update_explode_hover(&mut self) {
        let picked = self.pick_under_cursor();
        let hovered = picked.and_then(|(h, _)| match h {
            SceneEntityId::Object(id)
                if self
                    .workspace
                    .active_document()
                    .and_then(|document| document.get_object(id))
                    .is_some_and(|object| matches!(object, Object::Polyline { .. })) =>
            {
                Some(id)
            }
            _ => None,
        });
        if hovered != self.editor.tool_highlight_id {
            self.editor.tool_highlight_id = hovered;
            self.invalidate_geometry();
        }
    }

    pub(crate) fn explode_polyline(&mut self, object_id: ObjectId) {
        if !self.activate_project_for_object(object_id) {
            return;
        }
        let source = match self.active_document().get_object(object_id) {
            Some(obj @ Object::Polyline { .. }) => obj.clone(),
            _ => {
                return;
            }
        };
        let (verts, layer, color, line_weight, closed) = match &source {
            Object::Polyline {
                verts,
                layer,
                color,
                line_weight,
                closed,
                ..
            } => (verts.clone(), *layer, *color, *line_weight, *closed),
            _ => return,
        };

        if verts.len() < 2 {
            return;
        }

        // Build edge segments.
        let edge_count = if closed { verts.len() } else { verts.len() - 1 };
        let mut batch_cmds = vec![Command::delete_object(source)];

        if let Some(project) = self.workspace.active_project_mut() {
            let doc = &mut project.project.document;
            for i in 0..edge_count {
                let a = verts[i];
                let b = verts[(i + 1) % verts.len()];
                let id = doc.allocate_object_id();
                batch_cmds.push(Command::AddObject(Object::Polyline {
                    id,
                    layer,
                    verts: vec![a, b],
                    closed: false,
                    color,
                    fill: crate::model::FillStyle::Clear,
                    line_weight,
                }));
            }
            self.execute_edit(Command::Batch(batch_cmds));
        }

        self.editor.selected_handles.remove(&SceneEntityId::Object(object_id));
        self.editor.active_tool = ActiveTool::None;
        self.editor.tool_highlight_id = None;
        crate::logging::report_completed_action(
            CommandReportSpec::new(
                crate::i18n::tr!("cmd-explode-polyline"),
                crate::i18n::tr!("cmd-explode-count-line-s", count = edge_count.to_string()),
            ),
            crate::i18n::tr!("cmd-explode-exploded-polyline-into-count-line", count = edge_count.to_string()),
        );
        self.invalidate_geometry();
    }

    /// Explode to Points: the string goes and a point object takes each of
    /// its vertices, on its layer in its colour, as one undo; the points are
    /// left selected, ready for Build Surface. A locked or hidden string is
    /// left alone.
    pub(crate) fn explode_polyline_to_points(&mut self, object_id: ObjectId) {
        if !self.activate_project_for_object(object_id) || !self.editor.canvas_edits_object(self.active_document(), object_id) {
            return;
        }
        let source = match self.active_document().get_object(object_id) {
            Some(obj @ Object::Polyline { .. }) => obj.clone(),
            _ => return,
        };
        let (points, layer, color) = match &source {
            Object::Polyline { verts, closed, layer, color, .. } => (vertex_points(verts, *closed), *layer, *color),
            _ => return,
        };
        if points.is_empty() {
            return;
        }

        let count = points.len();
        let mut batch_cmds = vec![Command::delete_object(source)];
        let mut made = Vec::with_capacity(count);
        if let Some(project) = self.workspace.active_project_mut() {
            let doc = &mut project.project.document;
            for pos in points {
                let id = doc.allocate_object_id();
                made.push(SceneEntityId::Object(id));
                batch_cmds.push(Command::AddObject(Object::Point { id, layer, pos, color }));
            }
            self.execute_edit(Command::Batch(batch_cmds));
        }

        self.select_only(made);
        self.editor.active_tool = ActiveTool::None;
        self.editor.tool_highlight_id = None;
        crate::logging::report_completed_action(
            CommandReportSpec::new(
                crate::i18n::tr!("cmd-explode-points"),
                crate::i18n::tr!("cmd-explode-count-points", count = count.to_string()),
            ),
            crate::i18n::tr!("cmd-explode-exploded-polyline-into-count-points", count = count.to_string()),
        );
        self.invalidate_geometry();
    }
}

/// Where Explode to Points puts its points: on the string's vertices only, as
/// an arc's bulge is no vertex. An exact copy of the vertex before it gives
/// no second point, nor does a ring's last vertex copying its first; close
/// is not a copy, as a steep section string's vertices may be cm apart.
fn vertex_points(verts: &[PolyVertex], closed: bool) -> Vec<DVec3> {
    let mut points: Vec<DVec3> = Vec::with_capacity(verts.len());
    for vertex in verts {
        if points.last() != Some(&vertex.pos) {
            points.push(vertex.pos);
        }
    }
    if closed && points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    points
}
