//! The explorer's hover card's picture: one item on its own, turning slowly,
//! drawn offscreen through the scene's own pass into a texture the card
//! shows. See [`crate::ui::state::ExplorerPreview`].

use winit::dpi::PhysicalSize;

use super::{Graphics, solid_preview::SolidPreviewTarget};
use crate::{
    model::{ItemRef, OpenItem, block_model::OpenBlockModel, drill_hole::OpenDrillHoleDataset, point_cloud::OpenPointCloud, triangulation::OpenTriangulation},
    ui::state::{EditorState, ExplorerPreview, PreviewStatus, PreviewSubject, ViewportRect},
};

/// What the card's item may be found among.
pub(crate) struct PreviewItems<'frame> {
    pub(crate) triangulations: &'frame [OpenTriangulation],
    pub(crate) block_models: &'frame [OpenBlockModel],
    pub(crate) drill_holes: &'frame [OpenDrillHoleDataset],
    pub(crate) point_clouds: &'frame [OpenPointCloud],
    /// An unloaded item read in for the card alone.
    pub(crate) preview: PreviewLists<'frame>,
}

/// An unloaded item read in for the card alone, as a one-item list of its
/// kind, the others empty.
#[derive(Clone, Copy, Default)]
pub(crate) struct PreviewLists<'frame> {
    pub(crate) triangulations: &'frame [OpenTriangulation],
    pub(crate) block_models: &'frame [OpenBlockModel],
    pub(crate) drill_holes: &'frame [OpenDrillHoleDataset],
    pub(crate) point_clouds: &'frame [OpenPointCloud],
}

impl<'frame> PreviewLists<'frame> {
    pub(crate) fn of(item: Option<&'frame OpenItem>) -> Self {
        match item {
            Some(OpenItem::Triangulation(item)) => Self {
                triangulations: std::slice::from_ref(&**item),
                ..Self::default()
            },
            Some(OpenItem::BlockModel(item)) => Self {
                block_models: std::slice::from_ref(&**item),
                ..Self::default()
            },
            Some(OpenItem::DrillHole(item)) => Self {
                drill_holes: std::slice::from_ref(&**item),
                ..Self::default()
            },
            Some(OpenItem::PointCloud(item)) => Self {
                point_clouds: std::slice::from_ref(&**item),
                ..Self::default()
            },
            Some(OpenItem::Raster(_)) | None => Self::default(),
        }
    }
}

/// The lists the GPU caches are brought up to date from: the project's,
/// with the card's item among its kind's while there is one.
pub(crate) struct SyncedLists<'list> {
    pub(crate) surfaces: &'list [OpenTriangulation],
    pub(crate) block_models: &'list [OpenBlockModel],
    pub(crate) drill_holes: &'list [OpenDrillHoleDataset],
    pub(crate) point_clouds: &'list [OpenPointCloud],
}

/// Room for a kind's list with the card's item added. Only the one kind
/// the item is of is ever copied, and only while the card holds one.
#[derive(Default)]
pub(crate) struct WithPreview {
    surfaces: Vec<OpenTriangulation>,
    block_models: Vec<OpenBlockModel>,
    drill_holes: Vec<OpenDrillHoleDataset>,
    point_clouds: Vec<OpenPointCloud>,
}

impl WithPreview {
    pub(crate) fn lists<'list>(
        &'list mut self,
        preview: &PreviewLists<'_>,
        surfaces: &'list [OpenTriangulation],
        block_models: &'list [OpenBlockModel],
        drill_holes: &'list [OpenDrillHoleDataset],
        point_clouds: &'list [OpenPointCloud],
    ) -> SyncedLists<'list> {
        fn joined<'list, T: Clone>(room: &'list mut Vec<T>, list: &'list [T], extra: &[T]) -> &'list [T] {
            if extra.is_empty() {
                return list;
            }
            room.extend(list.iter().chain(extra).cloned());
            room
        }
        SyncedLists {
            surfaces: joined(&mut self.surfaces, surfaces, preview.triangulations),
            block_models: joined(&mut self.block_models, block_models, preview.block_models),
            drill_holes: joined(&mut self.drill_holes, drill_holes, preview.drill_holes),
            point_clouds: joined(&mut self.point_clouds, point_clouds, preview.point_clouds),
        }
    }
}

impl Graphics<'_> {
    /// Draw the card's item into its texture, for the card to show. The
    /// card turns, so this draws every frame it is open; nothing at all
    /// while it is not.
    pub(crate) fn render_explorer_preview(&mut self, editor: &mut EditorState, items: PreviewItems<'_>) {
        let preview = &editor.explorer_preview;
        let (Some(PreviewSubject::Item(item)), PreviewStatus::Ready, Some((lower, upper))) = (preview.subject, &preview.status, preview.bounds) else {
            editor.explorer_preview.texture = None;
            return;
        };
        let forward = preview.forward();
        // The project's own, while it is loaded; else the card's copy.
        fn one<'list, T>(list: &'list [T], found: impl Fn(&T) -> bool, copy: &'list [T]) -> &'list [T] {
            list.iter().find(|item| found(item)).map_or(copy, std::slice::from_ref)
        }
        let none = PreviewLists::default();
        let (triangulations, block_models, drill_holes, point_clouds) = match item {
            ItemRef::Triangulation(id) => (
                one(items.triangulations, |surface| surface.id == id && surface.state.loaded, items.preview.triangulations),
                none.block_models,
                none.drill_holes,
                none.point_clouds,
            ),
            ItemRef::BlockModel(id) => (
                none.triangulations,
                one(items.block_models, |model| model.id == id && model.state.loaded, items.preview.block_models),
                none.drill_holes,
                none.point_clouds,
            ),
            ItemRef::DrillHole(id) => (
                none.triangulations,
                none.block_models,
                one(items.drill_holes, |holes| holes.id == id && holes.state.loaded, items.preview.drill_holes),
                none.point_clouds,
            ),
            ItemRef::PointCloud(id) => (
                none.triangulations,
                none.block_models,
                none.drill_holes,
                one(items.point_clouds, |cloud| cloud.id == id && cloud.state.loaded, items.preview.point_clouds),
            ),
            ItemRef::Raster(_) => (none.triangulations, none.block_models, none.drill_holes, none.point_clouds),
        };
        if triangulations.is_empty() && block_models.is_empty() && drill_holes.is_empty() && point_clouds.is_empty() {
            editor.explorer_preview.texture = None;
            return;
        }

        let scale = self.scale_factor() as f32;
        let size = PhysicalSize::new(
            (ExplorerPreview::SIZE.x * scale).round().max(1.0) as u32,
            (ExplorerPreview::SIZE.y * scale).round().max(1.0) as u32,
        );
        let mut target = self.explorer_preview.take().unwrap_or_else(|| SolidPreviewTarget::new(self, size));
        if target.size != size {
            target.recreate_targets(self, size);
        }

        // An orthographic orbit, as the Solids Setup preview's: the eye on a
        // sphere round the box, far enough out to clear it at any angle, and
        // the box framed so it fits at every heading.
        let (center, radius) = super::solid_preview::framing_of(lower, upper);
        let distance = radius * 4.0;
        target.projection.zoom = ExplorerPreview::framing((lower, upper)).1;
        target.camera.look_to(center - forward * distance, forward, glam::DVec3::Z, distance);

        target.swap_into(self);
        self.projection.set_symmetric_depth_extent(distance + radius * 2.0);
        self.camera_uniform.update_view_proj(&self.camera, &self.projection, self.scene_origin, 1.0);
        self.camera_uniform.update_viewport(self.size.width, self.size.height);
        self.camera_uniform.set_viewport_origin(0, 0);
        self.camera_uniform.set_interaction_quality(1.0, 1.0);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.camera_uniform));

        // Shown though hidden in the scene: the card is a look at it, and
        // the eye is about the scene.
        let hidden = std::mem::take(&mut editor.hidden_handles);
        self.scene_pass_items_only = true;
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Explorer preview encoder"),
        });
        self.render_scene_pass(
            &mut encoder,
            &target.view,
            ViewportRect::full(self.size.width, self.size.height),
            editor,
            triangulations,
            block_models,
            drill_holes,
            point_clouds,
            &[],
            false,
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        self.scene_pass_items_only = false;
        editor.hidden_handles = hidden;
        target.swap_into(self);

        editor.explorer_preview.texture = Some(target.texture_id);
        self.explorer_preview = Some(target);
    }
}
