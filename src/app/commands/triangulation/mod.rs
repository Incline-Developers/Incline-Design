use std::collections::{HashMap, HashSet};

use anyhow::Result;
use spade::Triangulation as SpadeTriangulation;

#[cfg(not(target_arch = "wasm32"))]
use crate::app::file_name;
use crate::{
    app::App,
    i18n::{tr, tr_format},
    model::{
        ItemRef, Layer, MemberKind, Object, ObjectId, SceneEntityId, SectionKind, formats,
        formats::mesh_data,
        triangulation::{LoadedTriangulation, OpenTriangulation, TriangulationId},
    },
    ui::state::{ContourOutputLayer, TriPolylineClipMode, TriSurfaceCutSide, TriSurfaceType},
    userspace_log, userspace_warn,
};

// Linear-space value; displays as sRGB 1.0 (255) white.
const DEFAULT_TRIANGULATION_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

mod contours;
mod creation;
mod cuts;
mod geometry;
mod include;
mod point_cloud_tin;
pub(crate) mod reference_surface;
pub(crate) mod session;

use geometry::*;
pub(crate) use point_cloud_tin::{TerrainBudget, TerrainSampler, TerrainTinParams, terrain_budget_target};
