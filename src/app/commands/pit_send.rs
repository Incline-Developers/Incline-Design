//! Sending a pit design on to the rest of the project: its ramps to the
//! haul network as roads.
//!
//! What a design makes elsewhere is kept on it ([`Sent`]), so sending it
//! again replaces what it made before rather than adding to it. Each send is
//! one edit: the network and the design that remembers what it made, undone
//! together.

use anyhow::Result;

use super::pit_design::pit_parts;
use crate::{
    app::App,
    i18n::tr,
    model::{Command, Object, ObjectId, haulage::RoadId, pit_design::StringKind},
};

impl App<'_> {
    /// The pit design `id`, with its design current: what can be sent on.
    fn sendable_pit(&self, id: ObjectId) -> Result<&Object> {
        let object = self.operand_object(id).ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
        let (verts, pit) = pit_parts(object).ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
        let ground = self.pit_ground_version(&pit.settings());
        anyhow::ensure!(pit.is_fresh(verts, ground) && pit.design().is_some(), tr!("pit-design-send-not-designed"));
        Ok(object)
    }

    /// Send pit design `id`'s ramps to the haul network: each ramp a road
    /// along its centreline, joined to the network where it comes within
    /// the network's join tolerance of it, in place of the roads it was sent
    /// as before.
    pub(crate) fn send_pit_ramps_to_haulage(&mut self, id: ObjectId) -> Result<usize> {
        let object = self.sendable_pit(id)?.clone();
        let (_, pit) = pit_parts(&object).expect("checked a pit design");
        let design = pit.design().expect("checked designed");
        let centrelines: Vec<_> = design.strings.iter().filter(|string| string.kind == StringKind::RampCentreline).collect();
        anyhow::ensure!(!centrelines.is_empty(), tr!("pit-design-send-no-ramp"));
        let several = centrelines.len() > 1;
        let strings: Vec<(String, Vec<glam::DVec3>)> = centrelines
            .iter()
            .enumerate()
            .map(|(index, string)| {
                let name = if several {
                    tr!("pit-design-haul-road-numbered", design = pit.name.clone(), number = (index + 1).to_string())
                } else {
                    tr!("pit-design-haul-road", design = pit.name.clone())
                };
                (name, string.points.clone())
            })
            .collect();
        let document = self.workspace.active_document().ok_or_else(|| anyhow::anyhow!(tr!("pit-design-send-missing")))?;
        let before = document.haulage().clone();
        let mut after = before.clone();
        for road in &pit.sent.roads {
            if after.road(*road).is_some() {
                after.delete_road(*road);
            }
        }
        let existing: std::collections::HashSet<RoadId> = after.roads.iter().map(|road| road.id).collect();
        after.convert(&strings)?;
        after.validate()?;
        let made: Vec<RoadId> = after.roads.iter().map(|road| road.id).filter(|road| !existing.contains(road)).collect();
        let mut sent = object.clone();
        if let Object::PitDesign { pit, .. } = &mut sent {
            pit.sent.roads = made.clone();
        }
        self.execute_edit(Command::Batch(vec![
            Command::SetHaulNetwork {
                before: Box::new(before),
                after: Box::new(after),
            },
            Command::Replace {
                before: object.clone(),
                after: sent,
            },
        ]));
        self.editor.haul_route = None;
        self.refresh_haulage_view();
        crate::userspace_log!("{}", tr!("pit-design-sent-roads", count = made.len(), design = pit.name.clone()));
        Ok(made.len())
    }
}
