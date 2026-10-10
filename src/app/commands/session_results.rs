//! What thickness work leaves behind for the session, in one place.
//!
//! Five maps share one lifetime: held until the project changes or the layer
//! or surface they belong to is gone. They are results, not editor state, so
//! they live on `App`; the explorer reads which entries exist through
//! `UiProjectView` and asks for a table by command.

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::{
    app::commands::{thickness_points::ReferenceSource, triangulation::reference_surface::seam::ThicknessRunRecord},
    model::{LayerId, triangulation::TriangulationId},
    ui::state::{SeamTable, ThicknessTable},
};

/// Session-only results of Reference Points, Thickness Points and Thickness
/// Surfaces. Keys are a project's runtime id with a layer, or a surface id.
#[derive(Default)]
pub(crate) struct SessionResults {
    /// What each reference points layer was made from, by project and
    /// layer.
    pub(crate) reference_sources: HashMap<(u32, LayerId), ReferenceSource>,
    /// The seam each surface Build Surface made this session was picked on,
    /// to prefill its thickness points dialog.
    pub(crate) surface_seams: HashMap<TriangulationId, ReferenceSource>,
    /// The latest thickness points measured against each surface, held for
    /// its thickness grid.
    pub(crate) thickness_runs: HashMap<TriangulationId, ThicknessRunRecord>,
    /// Every thickness points layer's table made this session, by project
    /// runtime id and layer, to show again from the explorer.
    pub(crate) thickness_tables: HashMap<(u32, LayerId), Arc<ThicknessTable>>,
    /// Every surface made by Thickness Surfaces this session, with its
    /// grid's table, to show again from the explorer.
    pub(crate) seam_tables: HashMap<TriangulationId, Arc<SeamTable>>,
}

impl SessionResults {
    /// Forget everything, for ids that restart with a new project.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Drop the entries of a layer or surface that no longer exists.
    pub(crate) fn retain_live(&mut self, layer_live: impl Fn(u32, LayerId) -> bool, surface_live: impl Fn(TriangulationId) -> bool) {
        self.reference_sources.retain(|&(runtime_id, layer), _| layer_live(runtime_id, layer));
        self.thickness_tables.retain(|&(runtime_id, layer), _| layer_live(runtime_id, layer));
        self.surface_seams.retain(|&id, _| surface_live(id));
        self.thickness_runs.retain(|&id, _| surface_live(id));
        self.seam_tables.retain(|&id, _| surface_live(id));
    }

    /// Which tables can be shown, as a hash for the explorer's cached view.
    /// Order independent and allocation free.
    pub(crate) fn tables_fingerprint(&self) -> u64 {
        let one = |key: &dyn Fn(&mut std::collections::hash_map::DefaultHasher)| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            key(&mut hasher);
            hasher.finish()
        };
        let layers = self.thickness_tables.keys().fold(0u64, |sum, key| sum.wrapping_add(one(&|hasher| key.hash(hasher))));
        let surfaces = self.seam_tables.keys().fold(0u64, |sum, key| sum.wrapping_add(one(&|hasher| key.hash(hasher))));
        layers ^ surfaces.rotate_left(32)
    }
}
