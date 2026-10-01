use crate::{
    app::App,
    i18n::tr,
    model::{Axis, Command, FillStyle, LayerId, Object, ObjectColor, ObjectId, SceneEntityId},
    userspace_log,
};

macro_rules! batch_property {
    ($self:expr, $ids:expr, $update_fn:expr, $log:expr) => {{
        let Some(project) = $self.workspace.active_project_mut() else {
            return;
        };
        let doc = &mut project.project.document;
        let cmds: Vec<Command> = $ids
            .iter()
            .filter_map(|&id| {
                let before = doc.get_object(id)?.clone();
                let mut after = before.clone();
                $update_fn(&mut after);
                if before != after { Some(Command::Replace { before, after }) } else { None }
            })
            .collect();
        if !cmds.is_empty() {
            $self.execute_edit(Command::Batch(cmds));
        }
        $self.log_when_gesture_ends($log);
        $self.invalidate_geometry();
    }};
}

impl<'a> App<'a> {
    pub(crate) fn batch_set_object_color(&mut self, ids: Vec<ObjectId>, new_color: ObjectColor) {
        batch_property!(
            self,
            ids,
            |obj: &mut Object| {
                match obj {
                    Object::Point { color, .. } | Object::Polyline { color, .. } | Object::Circle { color, .. } | Object::Text { color, .. } => *color = new_color,
                }
            },
            tr!("cmd-property-batch-set-color-count-object", count = ids.len().to_string())
        );
    }

    pub(crate) fn batch_set_polyline_closed(&mut self, ids: Vec<ObjectId>, closed: bool) {
        batch_property!(
            self,
            ids,
            |obj: &mut Object| {
                if let Object::Polyline { closed: c, .. } = obj {
                    *c = closed;
                }
            },
            tr!("cmd-property-batch-set-closed-count-polyline", count = ids.len().to_string())
        );
    }

    pub(crate) fn batch_set_object_fill(&mut self, ids: Vec<ObjectId>, new_fill: FillStyle) {
        batch_property!(
            self,
            ids,
            |obj: &mut Object| {
                if let Object::Polyline { fill, .. } | Object::Circle { fill, .. } = obj {
                    *fill = new_fill;
                }
            },
            tr!("cmd-property-batch-set-fill-style-count", count = ids.len().to_string())
        );
    }

    pub(crate) fn batch_set_polyline_line_weight(&mut self, ids: Vec<ObjectId>, weight: f32) {
        batch_property!(
            self,
            ids,
            |obj: &mut Object| {
                if let Object::Polyline { line_weight, .. } | Object::Circle { line_weight, .. } = obj {
                    *line_weight = weight;
                }
            },
            tr!("cmd-property-batch-set-line-weight-count", count = ids.len().to_string())
        );
    }

    pub(crate) fn batch_set_axis_value(&mut self, ids: Vec<ObjectId>, axis: Axis, value: f64) {
        batch_property!(
            self,
            ids,
            |obj: &mut Object| {
                obj.set_axis_position(axis, value);
            },
            tr!("cmd-property-batch-set-axis-value-count", count = ids.len().to_string(), axis = axis.label().to_string())
        )
    }

    pub(crate) fn move_objects_to_layer(&mut self, ids: Vec<ObjectId>, target_layer: LayerId, copy: bool) {
        let Some(project) = self.workspace.active_project_mut() else {
            return;
        };
        if project.project.document.layer(target_layer).is_none() {
            return;
        }

        let doc = &mut project.project.document;
        let mut selected_after = Vec::new();
        let cmds: Vec<Command> = if copy {
            let originals: Vec<Object> = ids.iter().filter_map(|&id| doc.get_object(id).cloned()).collect();
            originals
                .into_iter()
                .map(|object| {
                    let new_id = doc.allocate_object_id();
                    let copied = object.with_id_and_layer(new_id, target_layer);
                    selected_after.push(copied.id());
                    Command::AddObject(copied)
                })
                .collect()
        } else {
            ids.iter()
                .filter_map(|&id| {
                    let before = doc.get_object(id)?.clone();
                    let after = before.with_id_and_layer(before.id(), target_layer);
                    if before == after {
                        None
                    } else {
                        selected_after.push(after.id());
                        Some(Command::Replace { before, after })
                    }
                })
                .collect()
        };

        if cmds.is_empty() {
            return;
        }

        self.execute_edit(Command::Batch(cmds));
        self.editor.selected_handles = selected_after.into_iter().map(SceneEntityId::Object).collect();
        self.invalidate_geometry();
        let action = if copy {
            crate::i18n::tr!("cmd-property-copied")
        } else {
            crate::i18n::tr!("cmd-property-moved")
        };
        userspace_log!(
            "{}",
            crate::i18n::tr!(
                "cmd-property-action-count-object-s-layer",
                action = action.to_string(),
                count = ids.len().to_string(),
                layer = format!("{target_layer:?}")
            )
        );
    }
}
