//! Whole-project Open Mining Format (OMF) interchange.
//!
//! OMF is a container for mining data, not a triangle-mesh encoding.  This
//! module therefore translates a snapshot of every project-owned data family
//! to one project and decodes all matching OMF element types on import.
//! Native OMF geometry and attributes keep the file useful in other programs;
//! `incline:*` metadata preserves application-specific semantics for lossless
//! round trips (bulged design strings, text, drill intervals, and styling).

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Seek, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result, bail};
use glam::{DMat3, DVec3};
use omf as omf_crate;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    i18n::{tr, tr_format},
    model::{
        Document, FillStyle, FolderId, FolderRegistry, Layer, MemberKind, Object, ObjectColor, PolyVertex, SectionKind,
        block_model::{
            BlockBounds, BlockBoundsSource, Boundary, ColorTransferFunction, LoadedBlockModel, OpenBlockModel, RenderableBlockIndices, StoredColorTransferFunction,
            compute_world_bounds, opaque_irregular_surface_block_count, opaque_surface_block_count,
        },
        drill_hole::{DrillHole, DrillHoleDataset, DrillHoleSource, DrillValue, LoadedDrillHoleDataset, OpenDrillHoleDataset},
        formats::{
            block_model_data::{BlockModelColumn, BlockModelData},
            mesh_data::{Triangulation, Vertex},
        },
        point_cloud::{LoadedPointCloud, OpenPointCloud, prepare_for_render},
        progress::Phase,
        project::{self, ProjectFile, ProjectMetadata},
        raster::{LoadedRasterTexture, OpenRasterTexture},
        triangulation::{LoadedTriangulation, OpenTriangulation, spatial_surface_face_order, unique_edges},
    },
    rendering::color::{linear_to_srgb_byte, rgb_bytes_to_linear_rgba},
};

const META_KIND: &str = "incline:kind";
const META_NAME: &str = "incline:name";
/// A design layer's [`DesignRecord`]s.
const META_OBJECTS: &str = "incline:objects";
/// The per-row column naming the design object a segment or point belongs to.
const DESIGN_OBJECT_ATTRIBUTE: &str = "Object";
/// Per-segment DXF bulge of a design polyline; absent when every segment is straight.
const DESIGN_BULGE_ATTRIBUTE: &str = "Bulge";
/// Ring resolution for a circle's native OMF geometry, which has no arcs. Only
/// readers that ignore `incline:objects` metadata ever see this approximation.
const CIRCLE_EXPORT_SEGMENTS: u32 = 64;
const META_LAYER: &str = "incline:layer";
/// Every section's explorer folder names, keyed by [`SectionKind::key`].
/// Written on the OMF project record; omitted entirely when the project
/// has no folders, so a project without folders writes byte for byte as
/// before.
const META_FOLDERS: &str = "incline:folders";
/// A layer's or item's folder membership: the name of the folder in its
/// own section, absent when the member is at the section root.
const META_FOLDER: &str = "incline:folder";
/// The explorer section an item's element is shown under, written only when
/// it differs from the section that kind of item naturally sits in - so a
/// file whose items are all where their kind puts them is byte for byte what
/// it was before.
const META_SECTION: &str = "incline:section";
const META_SOURCE: &str = "incline:source";
const META_STYLE: &str = "incline:style";
const META_ID: &str = "incline:id";
/// Depth ranges each hole is drawn over, where not the whole trace, keyed by
/// the hole's position in its dataset.
const META_RENDER_RANGES: &str = "incline:render_ranges";
/// The category on every drillhole row naming the hole it belongs to.
const DRILL_HOLE_ATTRIBUTE: &str = "Hole";
/// A dataset's tie-in: its surface connectors and where the round starts,
/// both keyed by hole name. Carried on the dataset's own element, because
/// they are what joins its holes rather than anything one hole holds.
const META_TIE_INS: &str = "incline:tie_ins";
const MAX_ARRAY_ITEMS: u64 = 200_000_000;

/// Owned, cheaply-cloned state captured before OMF encoding moves to a worker.
#[derive(Clone, Default)]
pub(crate) struct ProjectSnapshot {
    pub(crate) name: String,
    pub(crate) designs: Option<ProjectFile>,
    pub(crate) triangulations: Vec<OpenTriangulation>,
    pub(crate) block_models: Vec<OpenBlockModel>,
    pub(crate) drill_holes: Vec<OpenDrillHoleDataset>,
    pub(crate) point_clouds: Vec<OpenPointCloud>,
    pub(crate) rasters: Vec<OpenRasterTexture>,
    /// Every explorer folder in the project, for all six sections. The single
    /// source of truth for export: `designs`'s own `ProjectFile::folders` is
    /// not consulted, so there is exactly one registry to keep in sync.
    pub(crate) folders: FolderRegistry,
}

impl std::fmt::Debug for ProjectSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectSnapshot")
            .field("name", &self.name)
            .field("designs", &usize::from(self.designs.is_some()))
            .field("triangulations", &self.triangulations.len())
            .field("block_models", &self.block_models.len())
            .field("drill_holes", &self.drill_holes.len())
            .field("point_clouds", &self.point_clouds.len())
            .field("rasters", &self.rasters.len())
            .field("folders", &self.folders)
            .finish()
    }
}

impl ProjectSnapshot {
    /// Whether there is nothing here worth writing at all.
    ///
    /// Folders count here, unlike in [`Self::item_count`]: a project whose
    /// only unsaved work is an empty folder still has to be able to save it.
    pub(crate) fn is_empty(&self) -> bool {
        self.designs.is_none()
            && self.triangulations.is_empty()
            && self.block_models.is_empty()
            && self.drill_holes.is_empty()
            && self.point_clouds.is_empty()
            && self.rasters.is_empty()
            && self.folders.is_empty()
    }

    pub(crate) fn item_count(&self) -> usize {
        usize::from(self.designs.is_some()) + self.triangulations.len() + self.block_models.len() + self.drill_holes.len() + self.point_clouds.len() + self.rasters.len()
    }
}

pub(crate) struct ImportedTriangulation {
    pub(crate) preferred_id: Option<u64>,
    pub(crate) source_name: Option<String>,
    pub(crate) source_format: Option<String>,
    pub(crate) loaded: LoadedTriangulation,
    pub(crate) is_loaded: bool,
    pub(crate) deferred: Option<(DeferredAsset, crate::model::asset_residency::AssetSummary)>,
    /// The archive element the payload was read from; see [`PayloadSource`].
    pub(crate) payload_source: Option<DeferredAsset>,
    pub(crate) color: [f32; 4],
    pub(crate) line_color: [f32; 4],
    pub(crate) line_weight: Option<f32>,
    pub(crate) raster_opacity: f32,
    pub(crate) raster_texture_id: Option<u64>,
    pub(crate) folder: Option<FolderId>,
    /// The section this item is shown under, as its element recorded it.
    pub(crate) section: SectionKind,
}

pub(crate) struct ImportedBlockModel {
    pub(crate) preferred_id: Option<u64>,
    pub(crate) source_name: Option<String>,
    pub(crate) source_format: Option<String>,
    pub(crate) loaded: LoadedBlockModel,
    pub(crate) is_loaded: bool,
    pub(crate) deferred: Option<(DeferredAsset, crate::model::asset_residency::AssetSummary)>,
    pub(crate) color: [f32; 4],
    pub(crate) slice: Option<crate::model::block_model::BlockModelSlice>,
    /// Ramps keyed by variable name, resolved from Incline Design's own style
    /// metadata where the source carried it and otherwise from the OMF
    /// colormaps on the attributes themselves. A variable absent here has its
    /// ramp invented at load from the data.
    pub(crate) color_transfers: BTreeMap<String, ColorTransferFunction>,
    pub(crate) hide_empty_color_values: bool,
    pub(crate) folder: Option<FolderId>,
    /// The section this item is shown under, as its element recorded it.
    pub(crate) section: SectionKind,
}

pub(crate) struct ImportedPointCloud {
    pub(crate) preferred_id: Option<u64>,
    pub(crate) source_name: Option<String>,
    pub(crate) source_format: Option<String>,
    pub(crate) loaded: LoadedPointCloud,
    pub(crate) is_loaded: bool,
    pub(crate) deferred: Option<(DeferredAsset, crate::model::asset_residency::AssetSummary)>,
    /// The archive element the payload was read from; see [`PayloadSource`].
    pub(crate) payload_source: Option<DeferredAsset>,
    pub(crate) color: [f32; 4],
    pub(crate) point_size: f32,
    pub(crate) folder: Option<FolderId>,
    /// The section this item is shown under, as its element recorded it.
    pub(crate) section: SectionKind,
}

pub(crate) struct ImportedDrillHoles {
    pub(crate) preferred_id: Option<u64>,
    pub(crate) source_name: Option<String>,
    pub(crate) source_format: Option<String>,
    pub(crate) loaded: LoadedDrillHoleDataset,
    pub(crate) is_loaded: bool,
    pub(crate) deferred: Option<(DeferredAsset, crate::model::asset_residency::AssetSummary)>,
    pub(crate) color: crate::model::drill_hole::DrillColorState,
    pub(crate) folder: Option<FolderId>,
    /// The section this item is shown under, as its element recorded it.
    pub(crate) section: SectionKind,
}

pub(crate) struct ImportedRaster {
    pub(crate) preferred_id: Option<u64>,
    pub(crate) source_name: Option<String>,
    pub(crate) source_format: Option<String>,
    pub(crate) loaded: LoadedRasterTexture,
    pub(crate) is_loaded: bool,
    pub(crate) deferred: Option<(DeferredAsset, crate::model::asset_residency::AssetSummary)>,
    /// The archive element the image was read from; see [`PayloadSource`].
    pub(crate) payload_source: Option<DeferredAsset>,
    pub(crate) folder: Option<FolderId>,
    /// The section this item is shown under, as its element recorded it.
    pub(crate) section: SectionKind,
}

#[derive(Default)]
pub(crate) struct ImportBundle {
    pub(crate) project_name: String,
    pub(crate) coordinate_reference_system: String,
    pub(crate) units: String,
    pub(crate) origin: [f64; 3],
    pub(crate) designs: Vec<ProjectFile>,
    pub(crate) triangulations: Vec<ImportedTriangulation>,
    pub(crate) block_models: Vec<ImportedBlockModel>,
    pub(crate) drill_holes: Vec<ImportedDrillHoles>,
    pub(crate) point_clouds: Vec<ImportedPointCloud>,
    pub(crate) rasters: Vec<ImportedRaster>,
    /// Every explorer folder decoded so far, for all six sections. Populated
    /// from the OMF project record before any element is walked, so
    /// per-element membership below always resolves against it, and grown by
    /// `ensure` for a name the project record did not list.
    pub(crate) folders: FolderRegistry,
    pub(crate) warnings: Vec<String>,
}

impl ImportBundle {
    pub(crate) fn item_count(&self) -> usize {
        self.designs.len() + self.triangulations.len() + self.block_models.len() + self.drill_holes.len() + self.point_clouds.len() + self.rasters.len()
    }
}

/// How hard to compress an OMF container. Deflate's cost per level is steep and
/// badly non-linear, so the level is worth choosing per destination instead of
/// taking the library default of 6. Measured on a 7.8M-point survey cloud:
///
/// | level | time | size |
/// |-------|-------|---------|
/// | 1     | 0.50s | 83.3 MB |
/// | 2     | 0.77s | 54.6 MB |
/// | 3     | 0.97s | 51.8 MB |
/// | 5     | 1.80s | 48.6 MB |
/// | 6     | 4.34s | 48.1 MB |
///
/// Level 6 is a cliff - 2.4x the time of level 5 to shave a further 1% - and
/// it was costing more CPU than every other part of a save put together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Compression {
    /// A file the user keeps, syncs, or hands to another program. Level 5 is
    /// the last point before the cliff: within about 1% of what level 6 would
    /// have produced, so saved projects are no larger in any way that matters.
    Archive,
    /// A scratch spill this process wrote and only this process reads back,
    /// deleted when the last handle to it drops. Level 2 gives up about a
    /// tenth of the size for well over five times the speed. Going lower
    /// (level 1 is another 1.5x faster) would inflate the spill by three
    /// quarters, which defeats the point of unloading the data at all.
    Scratch,
}

impl From<Compression> for omf_crate::file::Compression {
    fn from(value: Compression) -> Self {
        match value {
            Compression::Archive => Self::new(5),
            Compression::Scratch => Self::new(2),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_path(snapshot: ProjectSnapshot, path: &Path, progress: &Phase) -> Result<()> {
    crate::model::atomic_file::write_atomic(path, |file| {
        write_to(snapshot, file, Compression::Archive, progress)?;
        Ok(())
    })
}

pub(crate) fn to_bytes(snapshot: ProjectSnapshot, compression: Compression, progress: &Phase) -> Result<Vec<u8>> {
    let cursor = write_to(snapshot, Cursor::new(Vec::new()), compression, progress)?;
    Ok(cursor.into_inner())
}

fn write_to<W: Write + Seek + Send>(snapshot: ProjectSnapshot, output: W, compression: Compression, progress: &Phase) -> Result<W> {
    if snapshot.is_empty() {
        bail!("There is no open Incline Design data to export");
    }
    let mut writer = omf_crate::file::Writer::new(output).context("create OMF writer")?;
    writer.set_compression(compression.into());
    let total = snapshot.item_count().max(1) as u64;
    let mut complete = 0u64;
    let mut elements = Vec::with_capacity(snapshot.item_count());
    let coordinate_reference_systems = snapshot
        .rasters
        .iter()
        .map(|raster| raster.projection.trim())
        .filter(|projection| !projection.is_empty())
        .collect::<BTreeSet<_>>();

    if let Some(design) = &snapshot.designs {
        elements.push(write_design(&mut writer, design, &snapshot.folders)?);
        complete += 1;
        progress.set_items(complete, total);
    }
    // Unchanged payloads are copied out of the archive they were read from,
    // still compressed. Compression is nearly all the cost of a save, and an
    // unloaded payload would otherwise be read back in just to encode it.
    let mut sources = SourceArchives::default();
    for triangulation in &snapshot.triangulations {
        let copied = match sources.unchanged(&triangulation.state, &PayloadIdentity::triangulation(triangulation)) {
            Some((reader, element)) => copy_surface(&mut writer, reader, element)?,
            None => None,
        };
        let mut element = match copied {
            Some(geometry) => triangulation_element(triangulation, geometry),
            None if triangulation.state.deferred.is_some() => {
                let restored = crate::model::OpenItem::Triangulation(Box::new(triangulation.clone())).materialize()?;
                let crate::model::OpenItem::Triangulation(item) = &restored else { unreachable!() };
                write_triangulation(&mut writer, item)?
            }
            None => write_triangulation(&mut writer, triangulation)?,
        };
        tag_folder(&mut element, &snapshot.folders, triangulation.state.section, triangulation.state.folder);
        tag_section(&mut element, MemberKind::Triangulation, triangulation.state.section);
        elements.push(element);
        complete += 1;
        progress.set_items(complete, total);
    }
    for block_model in &snapshot.block_models {
        let restored;
        let block_model = if block_model.state.deferred.is_some() {
            restored = crate::model::OpenItem::BlockModel(Box::new(block_model.clone())).materialize()?;
            let crate::model::OpenItem::BlockModel(item) = &restored else { unreachable!() };
            item.as_ref()
        } else {
            block_model
        };
        let mut element = write_block_model(&mut writer, block_model)?;
        tag_folder(&mut element, &snapshot.folders, block_model.state.section, block_model.state.folder);
        tag_section(&mut element, MemberKind::BlockModel, block_model.state.section);
        elements.push(element);
        complete += 1;
        progress.set_items(complete, total);
    }
    for drill_holes in &snapshot.drill_holes {
        let restored;
        let drill_holes = if drill_holes.state.deferred.is_some() {
            restored = crate::model::OpenItem::DrillHole(Box::new(drill_holes.clone())).materialize()?;
            let crate::model::OpenItem::DrillHole(item) = &restored else { unreachable!() };
            item.as_ref()
        } else {
            drill_holes
        };
        if let Some(mut element) = write_drill_holes(&mut writer, drill_holes)? {
            tag_folder(&mut element, &snapshot.folders, drill_holes.state.section, drill_holes.state.folder);
            tag_section(&mut element, MemberKind::DrillHole, drill_holes.state.section);
            elements.push(element);
        }
        complete += 1;
        progress.set_items(complete, total);
    }
    for point_cloud in &snapshot.point_clouds {
        let copied = match sources.unchanged(&point_cloud.state, &PayloadIdentity::point_cloud(point_cloud)) {
            Some((reader, element)) => copy_point_set(&mut writer, reader, element)?,
            None => None,
        };
        let mut element = match copied {
            Some((geometry, attributes)) => point_cloud_element(point_cloud, geometry, attributes),
            None if point_cloud.state.deferred.is_some() => {
                let restored = crate::model::OpenItem::PointCloud(Box::new(point_cloud.clone())).materialize()?;
                let crate::model::OpenItem::PointCloud(item) = &restored else { unreachable!() };
                write_point_cloud(&mut writer, item)?
            }
            None => write_point_cloud(&mut writer, point_cloud)?,
        };
        tag_folder(&mut element, &snapshot.folders, point_cloud.state.section, point_cloud.state.folder);
        tag_section(&mut element, MemberKind::PointCloud, point_cloud.state.section);
        elements.push(element);
        complete += 1;
        progress.set_items(complete, total);
    }
    for raster in &snapshot.rasters {
        let copied = match sources.unchanged(&raster.state, &PayloadIdentity::raster(raster)) {
            Some((reader, element)) => copy_raster_image(&mut writer, reader, element)?,
            None => None,
        };
        let mut element = match copied {
            Some(image) => write_raster(&mut writer, raster, image)?,
            None if raster.state.deferred.is_some() => {
                let restored = crate::model::OpenItem::Raster(Box::new(raster.clone())).materialize()?;
                let crate::model::OpenItem::Raster(item) = &restored else { unreachable!() };
                let image = writer.image_bytes(&encode_png(item.source_size, &item.full_rgba)?)?;
                write_raster(&mut writer, item, image)?
            }
            None => {
                let image = writer.image_bytes(&encode_png(raster.source_size, &raster.full_rgba)?)?;
                write_raster(&mut writer, raster, image)?
            }
        };
        tag_folder(&mut element, &snapshot.folders, raster.state.section, raster.state.folder);
        tag_section(&mut element, MemberKind::Raster, raster.state.section);
        elements.push(element);
        complete += 1;
        progress.set_items(complete, total);
    }

    make_element_names_unique(&mut elements);
    let fallback_name = tr!(literal = "Incline Design project");
    let mut project = omf_crate::Project::new(if snapshot.name.trim().is_empty() { fallback_name.as_str() } else { &snapshot.name });
    project.application = format!("Incline {}", env!("CARGO_PKG_VERSION"));
    project.description = tr!(literal = "Mining data exported by Incline");
    if let Some(design) = snapshot.designs.as_ref()
        && !design.metadata.coordinate_reference_system.trim().is_empty()
    {
        project.coordinate_reference_system = design.metadata.coordinate_reference_system.clone();
    } else if coordinate_reference_systems.len() == 1 {
        project.coordinate_reference_system = coordinate_reference_systems.into_iter().next().unwrap_or_default().to_owned();
    }
    if let Some(design) = snapshot.designs.as_ref() {
        project.units = design.metadata.units.clone();
    }
    project.elements = elements;
    if !snapshot.folders.is_empty() {
        let mut sections = serde_json::Map::new();
        for section in SectionKind::ALL {
            let names = snapshot.folders.names(section);
            if !names.is_empty() {
                sections.insert(section.key().to_owned(), json!(names));
            }
        }
        project.metadata.insert(META_FOLDERS.to_owned(), Value::Object(sections));
    }
    let (output, _warnings) = writer.finish(project).context("finish project")?;
    progress.finish();
    Ok(output)
}

fn put(element: &mut omf_crate::Element, key: &str, value: impl Into<Value>) {
    element.metadata.insert(key.to_owned(), value.into());
}

/// Tag `element` with the name its member resolves to in `section`, when it
/// resolves at all. Absent when `id` is `None` or does not resolve.
fn tag_folder(element: &mut omf_crate::Element, folders: &FolderRegistry, section: SectionKind, id: Option<FolderId>) {
    if let Some(name) = id.and_then(|id| folders.name(section, id)) {
        put(element, META_FOLDER, name);
    }
}

/// Tag `element` with `section` when it is not where `kind` naturally sits -
/// mirrors [`tag_folder`]'s absent-means-default convention, but for the
/// section rather than the folder within it.
fn tag_section(element: &mut omf_crate::Element, kind: MemberKind, section: SectionKind) {
    if section != SectionKind::natural_for(kind) {
        put(element, META_SECTION, section.key());
    }
}

fn kind(element: &omf_crate::Element) -> Option<&str> {
    element.metadata.get(META_KIND).and_then(Value::as_str)
}

fn element_name(element: &omf_crate::Element) -> &str {
    element.metadata.get(META_NAME).and_then(Value::as_str).unwrap_or(&element.name)
}

fn element_id(element: &omf_crate::Element) -> Option<u64> {
    element
        .metadata
        .get(META_ID)
        .and_then(|value| value.as_str().and_then(|value| value.parse().ok()).or_else(|| value.as_u64()))
}

fn element_source_name(element: &omf_crate::Element) -> Option<String> {
    element
        .metadata
        .get(META_SOURCE)
        .and_then(|value| value.get("filename"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn element_source_format(element: &omf_crate::Element) -> Option<String> {
    element
        .metadata
        .get(META_SOURCE)
        .and_then(|value| value.get("format"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn put_item_identity(element: &mut omf_crate::Element, id: u64, source_name: Option<&str>, source_format: Option<&str>, fallback_format: &str) {
    put(element, META_ID, id.to_string());
    if source_name.is_some() || source_format.is_some() {
        put(element, META_SOURCE, json!({ "filename": source_name, "format": source_format.unwrap_or(fallback_format) }));
    }
}

fn make_element_names_unique(elements: &mut [omf_crate::Element]) {
    let mut used = BTreeSet::new();
    for element in elements {
        let original = element.name.clone();
        let mut unique = original.clone();
        let mut suffix = 2usize;
        while !used.insert(unique.clone()) {
            unique = format!("{original} ({suffix})");
            suffix += 1;
        }
        if unique != original {
            put(element, META_NAME, original);
            element.name = unique;
        }
        if let omf_crate::Geometry::Composite(composite) = &mut element.geometry {
            make_element_names_unique(&mut composite.elements);
        }
    }
}

fn rgba8(color: [f32; 4]) -> [u8; 4] {
    [
        linear_to_srgb_byte(color[0]),
        linear_to_srgb_byte(color[1]),
        linear_to_srgb_byte(color[2]),
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn linear_rgba(color: [u8; 4]) -> [f32; 4] {
    let mut rgba = rgb_bytes_to_linear_rgba([color[0], color[1], color[2]]);
    rgba[3] = f32::from(color[3]) / 255.0;
    rgba
}

fn element_color(element: &omf_crate::Element, fallback: [f32; 4]) -> [f32; 4] {
    element.color.map(linear_rgba).unwrap_or(fallback)
}

fn write_design<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, design: &ProjectFile, folders: &FolderRegistry) -> Result<omf_crate::Element> {
    const LOCAL_MASK: u64 = u32::MAX as u64;
    let document = &design.document;
    let mut layers = Vec::with_capacity(document.layers().len());
    for layer in document.layers() {
        // Resolved before the swap below shadows `document`: the stand-in it
        // builds for an unloaded layer holds that layer alone, no folder list.
        let folder_name = layer.folder.and_then(|folder| folders.name(layer.section, folder));
        let restored;
        let document = if let Some(stored) = document.deferred_layers.get(&layer.id) {
            let mut resident = Document::new();
            resident.append_layer_snapshot(layer, std::iter::empty());
            resident.restore_layer_payload(layer.id, stored.read(layer.id)?);
            restored = resident;
            &restored
        } else {
            document
        };

        let (parts, records) = write_design_layer(writer, document, layer)?;
        let mut element = omf_crate::Element::new(layer.name.clone(), omf_crate::Composite::new(parts));
        element.color = Some(rgba8(layer.color));
        put(&mut element, META_KIND, "design_layer");
        put(&mut element, META_OBJECTS, serde_json::to_value(records)?);
        let mut portable_layer = layer.clone();
        portable_layer.id = crate::model::LayerId(layer.id.0 & LOCAL_MASK);
        // Folder and section travel in META_FOLDER/META_SECTION instead of
        // the blob, so a build that predates them still decodes it - the
        // blob denies unknown fields.
        portable_layer.folder = None;
        portable_layer.section = SectionKind::natural_layer();
        put(&mut element, META_LAYER, serde_json::to_value(portable_layer)?);
        if let Some(folder) = folder_name {
            put(&mut element, META_FOLDER, folder);
        }
        tag_section(&mut element, MemberKind::Layer, layer.section);
        layers.push(element);
    }
    // The layers still sit inside this "Designs" composite element regardless
    // of section - that is the file's container, not the tag.
    let mut element = omf_crate::Element::new("Designs", omf_crate::Composite::new(layers));
    put(&mut element, META_KIND, "designs");
    Ok(element)
}

/// A design object's settings, in document order on its layer's element.
/// Geometry is not repeated here: it lives in the layer's `Lines` and `Points`
/// sets, whose [`DESIGN_OBJECT_ATTRIBUTE`] rows name the object they belong to.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DesignRecord {
    Point {
        id: u64,
        color: ObjectColor,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        hidden: bool,
    },
    /// Vertices, bulges and closure come from the object's run of segments.
    Polyline {
        id: u64,
        color: ObjectColor,
        fill: FillStyle,
        line_weight: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        hidden: bool,
    },
    /// The segments are only a tessellated ring for other readers; the exact
    /// centre and radius are here.
    Circle {
        id: u64,
        color: ObjectColor,
        fill: FillStyle,
        line_weight: f32,
        center: DVec3,
        radius: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        hidden: bool,
    },
    Text {
        id: u64,
        color: ObjectColor,
        content: String,
        height: f64,
        rotation: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        hidden: bool,
    },
}

/// One layer's objects as a composite of at most two elements: every polyline
/// and circle in one `Lines` set, every point and text in one `Points` set.
/// Polyline `i` owns vertices `b..b+n` and the segments `[b+k, b+k+1]`, plus
/// `[b+n-1, b]` when closed - so closure is read back from the segments. A
/// segment's `Bulge` (written only when some bulge is non-zero) is the DXF
/// bulge of the vertex it starts at.
fn write_design_layer<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, document: &Document, layer: &Layer) -> Result<(Vec<omf_crate::Element>, Vec<DesignRecord>)> {
    use omf_crate::{Attribute, Element, LineSet, Location, PointSet};
    const LOCAL_MASK: u64 = u32::MAX as u64;

    let mut records = Vec::new();
    let (mut line_vertices, mut segments, mut segment_objects, mut bulges) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut point_vertices, mut point_objects) = (Vec::new(), Vec::new());
    for object in document.objects().iter().filter(|object| object.layer() == layer.id) {
        let id = object.id().0 & LOCAL_MASK;
        let color = object.color();
        let hidden = document.is_object_hidden(object.id());
        match object {
            Object::Point { pos, .. } => {
                point_vertices.push(pos.to_array());
                point_objects.push(Some(id as i64));
                records.push(DesignRecord::Point { id, color, hidden });
            }
            Object::Text {
                pos, content, height, rotation, ..
            } => {
                point_vertices.push(pos.to_array());
                point_objects.push(Some(id as i64));
                records.push(DesignRecord::Text {
                    id,
                    color,
                    content: content.clone(),
                    height: *height,
                    rotation: *rotation,
                    hidden,
                });
            }
            Object::Polyline {
                verts, closed, fill, line_weight, ..
            } => {
                if verts.len() < 2 {
                    bail!("polyline {id} on layer '{}' has fewer than two vertices", layer.name);
                }
                let first = line_vertices.len() as u32;
                let count = verts.len() as u32;
                line_vertices.extend(verts.iter().map(|vertex| vertex.pos.to_array()));
                segments.extend((0..count - 1).map(|index| [first + index, first + index + 1]));
                if *closed {
                    segments.push([first + count - 1, first]);
                }
                let segment_count = if *closed { verts.len() } else { verts.len() - 1 };
                bulges.extend(verts[..segment_count].iter().map(|vertex| Some(vertex.bulge)));
                segment_objects.extend(std::iter::repeat_n(Some(id as i64), segment_count));
                records.push(DesignRecord::Polyline {
                    id,
                    color,
                    fill: *fill,
                    line_weight: *line_weight,
                    hidden,
                });
            }
            // OMF has no arc primitive, so other readers see a tessellated ring.
            Object::Circle {
                center,
                radius,
                fill,
                line_weight,
                ..
            } => {
                let first = line_vertices.len() as u32;
                line_vertices.extend((0..CIRCLE_EXPORT_SEGMENTS).map(|step| {
                    let angle = std::f64::consts::TAU * (f64::from(step) / f64::from(CIRCLE_EXPORT_SEGMENTS));
                    [center.x + radius * angle.cos(), center.y + radius * angle.sin(), center.z]
                }));
                segments.extend((0..CIRCLE_EXPORT_SEGMENTS).map(|step| [first + step, first + (step + 1) % CIRCLE_EXPORT_SEGMENTS]));
                bulges.extend(std::iter::repeat_n(None, CIRCLE_EXPORT_SEGMENTS as usize));
                segment_objects.extend(std::iter::repeat_n(Some(id as i64), CIRCLE_EXPORT_SEGMENTS as usize));
                records.push(DesignRecord::Circle {
                    id,
                    color,
                    fill: *fill,
                    line_weight: *line_weight,
                    center: *center,
                    radius: *radius,
                    hidden,
                });
            }
        }
    }

    let mut elements = Vec::new();
    if !segments.is_empty() {
        let mut lines = Element::new("Lines", LineSet::new(writer.array_vertices(line_vertices)?, writer.array_segments(segments)?));
        lines.color = Some(rgba8(layer.color));
        lines.attributes.push(Attribute::from_numbers(
            DESIGN_OBJECT_ATTRIBUTE,
            Location::Primitives,
            writer.array_numbers(segment_objects)?,
        ));
        if bulges.iter().flatten().any(|bulge| *bulge != 0.0) {
            lines
                .attributes
                .push(Attribute::from_numbers(DESIGN_BULGE_ATTRIBUTE, Location::Primitives, writer.array_numbers(bulges)?));
        }
        put(&mut lines, META_KIND, "design_lines");
        elements.push(lines);
    }
    if !point_vertices.is_empty() {
        let mut points = Element::new("Points", PointSet::new(writer.array_vertices(point_vertices)?));
        points.color = Some(rgba8(layer.color));
        points
            .attributes
            .push(Attribute::from_numbers(DESIGN_OBJECT_ATTRIBUTE, Location::Vertices, writer.array_numbers(point_objects)?));
        put(&mut points, META_KIND, "design_points");
        elements.push(points);
    }
    Ok((elements, records))
}

fn write_triangulation<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, triangulation: &OpenTriangulation) -> Result<omf_crate::Element> {
    let mesh = &triangulation.mesh;
    let vertices = mesh.vertices().iter().map(|vertex| vertex.as_array());
    let triangles = mesh.face_vertex_indices_iter().map(|face| face.map(|index| index as u32));
    let geometry = omf_crate::Surface::new(writer.array_vertices(vertices)?, writer.array_triangles(triangles)?);
    Ok(triangulation_element(triangulation, geometry.into()))
}

/// Everything about a triangulation's element except its arrays.
fn triangulation_element(triangulation: &OpenTriangulation, geometry: omf_crate::Geometry) -> omf_crate::Element {
    let mut element = omf_crate::Element::new(triangulation.name.clone(), geometry);
    element.color = Some(rgba8(triangulation.color));
    put(&mut element, META_KIND, "triangulation");
    put_item_identity(
        &mut element,
        triangulation.id.0,
        triangulation.state.source_name.as_deref(),
        triangulation.state.source_format.as_deref(),
        "surface",
    );
    put(
        &mut element,
        META_STYLE,
        json!({
            "loaded": triangulation.state.loaded,
            "color": triangulation.color,
            "line_color": triangulation.line_color,
            "line_weight": triangulation.line_weight,
            "raster_opacity": triangulation.raster_opacity,
            "raster_texture_id": triangulation.raster_texture.map(|id| id.0),
        }),
    );
    element
}

fn write_point_cloud<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, cloud: &OpenPointCloud) -> Result<omf_crate::Element> {
    let geometry = omf_crate::PointSet::new(writer.array_vertices(cloud.points.iter().map(|point| point.to_array()))?);
    let mut attributes = Vec::new();
    if let Some(colors) = cloud.colors.as_ref().filter(|colors| colors.len() == cloud.points.len()) {
        attributes.push(omf_crate::Attribute::from_colors(
            "Color",
            omf_crate::Location::Vertices,
            writer.array_colors(colors.iter().map(|color| Some(color.to_le_bytes())))?,
        ));
    }
    if let Some(codes) = cloud.classifications.as_ref().filter(|codes| codes.len() == cloud.points.len()) {
        attributes.push(write_point_classification(writer, codes)?);
    }
    Ok(point_cloud_element(cloud, geometry.into(), attributes))
}

/// Everything about a point cloud's element except its arrays.
fn point_cloud_element(cloud: &OpenPointCloud, geometry: omf_crate::Geometry, attributes: Vec<omf_crate::Attribute>) -> omf_crate::Element {
    let mut element = omf_crate::Element::new(cloud.name.clone(), geometry);
    element.color = Some(rgba8(cloud.color));
    element.attributes = attributes;
    put(&mut element, META_KIND, "point_cloud");
    put_item_identity(
        &mut element,
        cloud.id.0,
        cloud.state.source_name.as_deref(),
        cloud.state.source_format.as_deref(),
        "point-set",
    );
    put(
        &mut element,
        META_STYLE,
        json!({ "loaded": cloud.state.loaded, "color": cloud.color, "point_size": cloud.point_size }),
    );
    element
}

/// Write ASPRS point classifications as an OMF category attribute.
///
/// Only the codes the cloud actually uses become categories, so the indices
/// stay narrow and the names list stays short. The ASPRS code itself travels in
/// the same `Incline category code` sidecar the block-model writer uses, which
/// is what lets a round-trip recover codes rather than category positions, and
/// the gradient carries the colours the classification view draws so other
/// applications show the same cloud.
fn write_point_classification<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, codes: &[u8]) -> Result<omf_crate::Attribute> {
    use crate::model::point_cloud::{classification_color, classification_name};

    let mut seen = [false; 256];
    for code in codes {
        seen[usize::from(*code)] = true;
    }
    let used: Vec<u8> = (0..=u8::MAX).filter(|code| seen[usize::from(*code)]).collect();
    let mut lookup = [0u32; 256];
    for (index, code) in used.iter().enumerate() {
        lookup[usize::from(*code)] = index as u32;
    }
    let names = used
        .iter()
        .map(|code| classification_name(*code).map_or_else(|| format!("Class {code}"), str::to_owned))
        .collect::<Vec<_>>();
    let original_codes = omf_crate::Attribute::from_numbers(
        "Incline category code",
        omf_crate::Location::Categories,
        writer.array_numbers(used.iter().map(|code| Some(i64::from(*code))))?,
    );
    let gradient = writer.array_gradient(used.iter().map(|code| classification_color(*code).to_le_bytes()))?;
    let indices = writer.array_indices(codes.iter().map(|code| Some(lookup[usize::from(*code)])))?;
    let names = writer.array_names(names)?;
    Ok(omf_crate::Attribute::from_categories(
        POINT_CLASSIFICATION_ATTRIBUTE,
        omf_crate::Location::Vertices,
        indices,
        names,
        Some(gradient),
        [original_codes],
    ))
}

/// Attribute name the classification column round-trips under.
const POINT_CLASSIFICATION_ATTRIBUTE: &str = "Classification";

/// Write a numeric attribute, carrying its colormap when it has one.
///
/// Both colormap kinds are stored in exactly OMF's shape, so this is a copy
/// rather than a conversion. Boundaries and the range are written as `f64` to
/// match the `f64` values alongside them, which the standard requires the types
/// to agree on - [`BlockModelData`] decodes every numeric column to `f64`, so
/// that is the only honest choice.
fn write_number_attribute<W: Write + Seek + Send>(
    writer: &mut omf_crate::file::Writer<W>,
    name: &str,
    location: omf_crate::Location,
    values: omf_crate::Array<omf_crate::array_type::Number>,
    transfer: Option<&ColorTransferFunction>,
) -> Result<omf_crate::Attribute> {
    match transfer {
        Some(ColorTransferFunction::Discrete { boundaries, gradient }) if !boundaries.is_empty() && gradient.len() == boundaries.len() + 1 => {
            let boundaries = writer.array_boundaries(boundaries.iter().map(|boundary| {
                if boundary.inclusive {
                    omf_crate::data::Boundary::LessEqual(boundary.value)
                } else {
                    omf_crate::data::Boundary::Less(boundary.value)
                }
            }))?;
            let gradient = writer.array_gradient(gradient.iter().map(|color| rgba8(*color)))?;
            Ok(omf_crate::Attribute::from_numbers_discrete_colormap(
                name.to_owned(),
                location,
                values,
                boundaries,
                gradient,
            ))
        }
        Some(ColorTransferFunction::Continuous { range, gradient }) if !gradient.is_empty() => {
            let gradient = writer.array_gradient(gradient.iter().map(|color| rgba8(*color)))?;
            Ok(omf_crate::Attribute::from_numbers_continuous_colormap(name.to_owned(), location, values, *range, gradient))
        }
        // A `Category` colormap on a numeric column, or a malformed one, has
        // nothing valid to say here.
        _ => Ok(omf_crate::Attribute::from_numbers(name.to_owned(), location, values)),
    }
}

/// One RGBA per category, in `codes` order, ready for `Category.gradient`.
fn category_gradient(variable: &crate::model::formats::block_model_data::BlockVariable, codes: &[u32], transfer: Option<&ColorTransferFunction>) -> Vec<[u8; 4]> {
    codes
        .iter()
        .map(|code| {
            let color = match transfer {
                Some(ColorTransferFunction::Category { gradient }) => gradient.get(code).copied(),
                _ => None,
            }
            .or_else(|| variable.category_colors.get(code).copied())
            .unwrap_or([0.72, 0.72, 0.75, 1.0]);
            rgba8(color)
        })
        .collect()
}

/// An OMF `NumberColormap` as an Incline Design colormap. A move, not a translation:
/// gradients are kept whole (the render side caps its own working copy) and
/// boundary inclusiveness is preserved.
fn read_number_colormap<R: omf_crate::file::ReadAt>(reader: &omf_crate::file::Reader<R>, colormap: &omf_crate::NumberColormap) -> Result<ColorTransferFunction> {
    match colormap {
        omf_crate::NumberColormap::Continuous { range, gradient } => {
            let gradient = collect_results(reader.array_gradient(gradient)?)?.into_iter().map(linear_rgba).collect::<Vec<_>>();
            if gradient.is_empty() {
                bail!("continuous colormap has no colours");
            }
            Ok(ColorTransferFunction::Continuous {
                range: number_range_bounds(range),
                gradient,
            })
        }
        omf_crate::NumberColormap::Discrete { boundaries, gradient } => {
            let gradient = collect_results(reader.array_gradient(gradient)?)?.into_iter().map(linear_rgba).collect::<Vec<_>>();
            let boundaries = read_boundaries(reader, boundaries)?;
            if boundaries.is_empty() {
                bail!("discrete colormap has no boundaries");
            }
            if gradient.len() != boundaries.len() + 1 {
                bail!("discrete colormap has {} colours for {} boundaries", gradient.len(), boundaries.len());
            }
            Ok(ColorTransferFunction::Discrete { boundaries, gradient })
        }
    }
}

/// A colormap's range as `f64`. Dates and date-times become their numeric epoch
/// offsets, matching how `array_numbers().try_into_f64()` decodes the values the
/// range describes.
fn number_range_bounds(range: &omf_crate::NumberRange) -> (f64, f64) {
    match range {
        omf_crate::NumberRange::Float { min, max } => (*min, *max),
        omf_crate::NumberRange::Integer { min, max } => (*min as f64, *max as f64),
        omf_crate::NumberRange::Date { min, max } => (omf_crate::date_time::date_to_f64(*min), omf_crate::date_time::date_to_f64(*max)),
        omf_crate::NumberRange::DateTime { min, max } => (min.timestamp() as f64, max.timestamp() as f64),
    }
}

fn read_boundaries<R: omf_crate::file::ReadAt>(reader: &omf_crate::file::Reader<R>, array: &omf_crate::Array<omf_crate::array_type::Boundary>) -> Result<Vec<Boundary>> {
    let boundaries = reader.array_boundaries(array)?;
    // Integer boundaries refuse the f64 cast, so fall back rather than losing
    // the colormap entirely.
    let decoded = match boundaries.try_into_f64() {
        Ok(iter) => collect_results(iter)?,
        Err(_) => collect_results(reader.array_boundaries(array)?.try_into_i64()?)?
            .into_iter()
            .map(|boundary| boundary.map(|value| value as f64))
            .collect(),
    };
    Ok(decoded
        .into_iter()
        .enumerate()
        .map(|(index, boundary)| Boundary {
            id: index as u64 + 1,
            value: boundary.value(),
            inclusive: boundary.is_inclusive(),
        })
        .collect())
}

fn write_block_model<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, open: &OpenBlockModel) -> Result<omf_crate::Element> {
    let model = &open.model;
    let lower = model.metadata.lower;
    let upper = model.metadata.upper;
    let extent = upper - lower;
    if !extent.is_finite() || extent.min_element() <= 0.0 {
        bail!("Block model '{}' has invalid bounds", open.name);
    }
    let selected = open.renderable_block_indices.iter().collect::<Vec<_>>();
    if selected.is_empty() {
        bail!("Block model '{}' has no renderable blocks", open.name);
    }
    let subblocks = selected
        .iter()
        .map(|&index| {
            let block = open.blocks.get(index).with_context(|| format!("Block model '{}' is missing block {index}", open.name))?;
            // Normalizing by `extent` should land boundary blocks exactly on
            // 0.0/1.0, but float rounding can push a corner a hair outside
            // that range, which OMF's strict validation rejects.
            let min = ((block.lower - lower) / extent).clamp(DVec3::ZERO, DVec3::ONE);
            let max = ((block.upper - lower) / extent).clamp(DVec3::ZERO, DVec3::ONE);
            Ok(([0, 0, 0], [min.x, min.y, min.z, max.x, max.y, max.z]))
        })
        .collect::<Result<Vec<_>>>()?;
    let rotation = model.rotation();
    let geometry = omf_crate::BlockModel::with_freeform_subblocks(
        omf_crate::Orient3::new(
            model.local_to_world(lower).to_array(),
            rotation.x_axis.to_array(),
            rotation.y_axis.to_array(),
            rotation.z_axis.to_array(),
        ),
        omf_crate::Grid3::from_size_and_count(extent.to_array(), [1, 1, 1]),
        writer.array_freeform_subblocks(subblocks)?,
    );
    let mut element = omf_crate::Element::new(open.name.clone(), geometry);
    element.color = Some(rgba8(open.color));
    for variable in &model.metadata.variables {
        let Some(values) = model.shared_numeric_values(&variable.name) else {
            continue;
        };
        if variable.strings.is_empty() {
            let selected_values = selected.iter().map(|&index| values.get(index).copied().filter(|value| value.is_finite()));
            let numbers = writer.array_numbers(selected_values)?;
            element.attributes.push(write_number_attribute(
                writer,
                &variable.name,
                omf_crate::Location::Subblocks,
                numbers,
                open.color_transfers.get(&variable.name),
            )?);
        } else {
            let codes = variable.strings.keys().copied().collect::<Vec<_>>();
            let code_to_index = codes.iter().enumerate().map(|(index, code)| (*code, index as u32)).collect::<BTreeMap<_, _>>();
            let indices = selected.iter().map(|&index| {
                values
                    .get(index)
                    .copied()
                    .filter(|value| value.is_finite() && value.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(value))
                    .and_then(|value| code_to_index.get(&(value as u32)).copied())
            });
            let names = codes.iter().map(|code| variable.strings.get(code).cloned().unwrap_or_else(|| code.to_string()));
            let original_codes = omf_crate::Attribute::from_numbers(
                "Incline category code",
                omf_crate::Location::Categories,
                writer.array_numbers(codes.iter().map(|code| Some(i64::from(*code))))?,
            );
            // One colour per name, in the same order - OMF's `gradient` is
            // exactly Incline Design's per-category ramp, so other applications see
            // the colours the user chose rather than inventing their own.
            let gradient = category_gradient(variable, &codes, open.color_transfers.get(&variable.name));
            let indices = writer.array_indices(indices)?;
            let names = writer.array_names(names)?;
            element.attributes.push(omf_crate::Attribute::from_categories(
                variable.name.clone(),
                omf_crate::Location::Subblocks,
                indices,
                names,
                Some(writer.array_gradient(gradient)?),
                [original_codes],
            ));
        }
    }
    put(&mut element, META_KIND, "block_model");
    put_item_identity(
        &mut element,
        open.id.0,
        open.state.source_name.as_deref(),
        open.state.source_format.as_deref(),
        "block-model",
    );
    put(
        &mut element,
        META_STYLE,
        json!({
            "loaded": open.state.loaded,
            "color": open.color,
            "slice": open.slice,
            "active_color_variable": open.active_color_variable,
            "hide_empty_color_values": open.hide_empty_color_values,
        }),
    );
    Ok(element)
}

/// Write a drillhole dataset as three standard OMF elements under one
/// composite, each holding every hole: collars as a point set, survey traces
/// as one line set, and intervals as one line set whose segments are the
/// intervals themselves. Every row carries a `Hole` category naming its hole,
/// so the dataset regroups exactly, and the arrays are the only copy of the
/// data - nothing is repeated in metadata.
fn write_drill_holes<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, open: &OpenDrillHoleDataset) -> Result<Option<omf_crate::Element>> {
    use omf_crate::{Attribute, Element, LineSet, Location, PointSet};

    let holes = &open.dataset.holes;
    if holes.is_empty() {
        return Ok(None);
    }
    let hole_names = holes.iter().map(|hole| hole.dhid.clone()).collect::<Vec<_>>();
    let hole_category = |writer: &mut omf_crate::file::Writer<W>, location, rows: &dyn Fn(&DrillHole) -> usize| -> Result<Attribute> {
        let indices = holes.iter().enumerate().flat_map(|(index, hole)| std::iter::repeat_n(Some(index as u32), rows(hole)));
        Ok(Attribute::from_categories(
            DRILL_HOLE_ATTRIBUTE,
            location,
            writer.array_indices(indices)?,
            writer.array_names(hole_names.iter().cloned())?,
            None,
            [],
        ))
    };

    let mut collars = Element::new("Collars", PointSet::new(writer.array_vertices(holes.iter().map(|hole| hole.collar.to_array()))?));
    collars.attributes.push(hole_category(writer, Location::Vertices, &|_| 1)?);
    collars.attributes.push(Attribute::from_numbers(
        "Diameter",
        Location::Vertices,
        writer.array_numbers(holes.iter().map(|hole| hole.diameter.filter(|diameter| diameter.is_finite())))?,
    ));
    put(&mut collars, META_KIND, "drillhole_collars");

    let mut segments = Vec::new();
    let mut first = 0u32;
    for hole in holes {
        let count = hole.trace.len() as u32;
        segments.extend((1..count).map(|index| [first + index - 1, first + index]));
        first += count;
    }
    let mut traces = Element::new(
        "Traces",
        LineSet::new(
            writer.array_vertices(holes.iter().flat_map(|hole| hole.trace.iter().map(|station| station.position.to_array())))?,
            writer.array_segments(segments)?,
        ),
    );
    traces.attributes.push(hole_category(writer, Location::Vertices, &|hole| hole.trace.len())?);
    traces.attributes.push(Attribute::from_numbers(
        "Measured depth",
        Location::Vertices,
        writer.array_numbers(holes.iter().flat_map(|hole| hole.trace.iter().map(|station| Some(station.depth))))?,
    ));
    put(&mut traces, META_KIND, "drillhole_traces");

    // Interval ends are placed on the trace so other applications draw each
    // interval where it lies; `From` and `To` remain the authority.
    let interval_count = holes.iter().map(|hole| hole.intervals.len()).sum::<usize>();
    let ends = holes.iter().flat_map(|hole| {
        hole.intervals
            .iter()
            .flat_map(|interval| [interval.from, interval.to].map(|depth| hole.position_at_depth(depth).filter(|position| position.is_finite()).unwrap_or(hole.collar).to_array()))
    });
    let mut intervals = Element::new(
        "Intervals",
        LineSet::new(
            writer.array_vertices(ends)?,
            writer.array_segments((0..interval_count as u32).map(|index| [2 * index, 2 * index + 1]))?,
        ),
    );
    intervals.attributes.push(hole_category(writer, Location::Primitives, &|hole| hole.intervals.len())?);
    let all_intervals = || holes.iter().flat_map(|hole| hole.intervals.iter());
    intervals.attributes.push(Attribute::from_numbers(
        "From",
        Location::Primitives,
        writer.array_numbers(all_intervals().map(|interval| Some(interval.from)))?,
    ));
    intervals.attributes.push(Attribute::from_numbers(
        "To",
        Location::Primitives,
        writer.array_numbers(all_intervals().map(|interval| Some(interval.to)))?,
    ));
    let keys = all_intervals().flat_map(|interval| interval.values.keys()).collect::<BTreeSet<_>>();
    for key in keys {
        let values = || all_intervals().map(|interval| interval.values.get(key));
        let numeric = values().all(|value| !matches!(value, Some(DrillValue::Category(_))));
        if numeric {
            let numbers = writer.array_numbers(values().map(|value| match value {
                Some(DrillValue::Numeric(value)) if value.is_finite() => Some(*value),
                _ => None,
            }))?;
            intervals.attributes.push(Attribute::from_numbers(key.clone(), Location::Primitives, numbers));
        } else {
            // A column holding any text is categorical; a number that shares
            // one with text is kept as its text.
            let text = |value: &DrillValue| match value {
                DrillValue::Category(text) => text.clone(),
                DrillValue::Numeric(number) => number.to_string(),
            };
            let names = values().flatten().map(text).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
            let lookup = names.iter().enumerate().map(|(index, name)| (name.as_str(), index as u32)).collect::<BTreeMap<_, _>>();
            let indices = writer.array_indices(values().map(|value| value.and_then(|value| lookup.get(text(value).as_str()).copied())))?;
            intervals.attributes.push(Attribute::from_categories(
                key.clone(),
                Location::Primitives,
                indices,
                writer.array_names(names.iter().cloned())?,
                None,
                [],
            ));
        }
    }
    put(&mut intervals, META_KIND, "drillhole_intervals");

    let mut element = Element::new(open.name.clone(), omf_crate::Composite::new(vec![collars, traces, intervals]));
    put(&mut element, META_KIND, "drillhole_dataset");
    put_item_identity(
        &mut element,
        open.id.0,
        open.state.source_name.as_deref(),
        open.state.source_format.as_deref(),
        "drill-holes",
    );
    put(&mut element, META_STYLE, json!({ "loaded": open.state.loaded, "color": open.color }));
    let ties = open.dataset.stored_ties();
    if !ties.is_empty() {
        put(&mut element, META_TIE_INS, serde_json::to_value(&ties)?);
    }
    // Keyed by position in the collars, which is the dataset's hole order.
    let render_ranges = holes
        .iter()
        .enumerate()
        .filter(|(_, hole)| !hole.render_ranges.is_empty())
        .map(|(index, hole)| (index.to_string(), json!(hole.render_ranges)))
        .collect::<serde_json::Map<_, _>>();
    if !render_ranges.is_empty() {
        put(&mut element, META_RENDER_RANGES, Value::Object(render_ranges));
    }
    Ok(Some(element))
}

/// `image` is the raster's pixels already in the archive, encoded afresh or
/// copied; everything else is written here.
fn write_raster<W: Write + Seek + Send>(
    writer: &mut omf_crate::file::Writer<W>,
    raster: &OpenRasterTexture,
    image: omf_crate::Array<omf_crate::array_type::Image>,
) -> Result<omf_crate::Element> {
    let [a, b, c, d, e, f] = raster.world_to_uv;
    let determinant = a * e - b * d;
    if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
        bail!("Raster '{}' has a singular world transform", raster.name);
    }
    let world = |u: f64, v: f64| {
        let x = (e * (u - c) - b * (v - f)) / determinant;
        let y = (-d * (u - c) + a * (v - f)) / determinant;
        DVec3::new(x, y, 0.0)
    };
    let origin = world(0.0, 0.0);
    let u_vector = world(1.0, 0.0) - origin;
    let v_vector = world(0.0, 1.0) - origin;
    let corners = [origin, origin + u_vector, origin + u_vector + v_vector, origin + v_vector];
    let mut element = omf_crate::Element::new(
        raster.name.clone(),
        omf_crate::Surface::new(
            writer.array_vertices(corners.map(|corner| corner.to_array()))?,
            writer.array_triangles([[0, 1, 2], [0, 2, 3]])?,
        ),
    );
    element.attributes.push(omf_crate::Attribute::from_texture_map(
        "Raster",
        image,
        omf_crate::Location::Vertices,
        writer.array_texcoords([[0.0_f64, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])?,
    ));
    put(&mut element, META_KIND, "raster");
    put_item_identity(
        &mut element,
        raster.id.0,
        raster.state.source_name.as_deref(),
        raster.state.source_format.as_deref(),
        "raster",
    );
    put(
        &mut element,
        META_STYLE,
        json!({
            "loaded": raster.state.loaded,
            "world_to_uv": raster.world_to_uv,
            "source_size": raster.source_size,
            "preview_size": raster.preview_size,
            "projection": raster.projection,
            "driver_name": raster.driver_name,
        }),
    );
    Ok(element)
}

fn encode_png(size: [u32; 2], rgba: &[u8]) -> Result<Vec<u8>> {
    let expected = usize::try_from(u64::from(size[0]) * u64::from(size[1]) * 4).context("raster dimensions exceed addressable memory")?;
    if rgba.len() != expected {
        bail!("Raster pixel buffer has {} bytes; expected {expected}", rgba.len());
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, size[0], size[1]);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // fdeflate's PNG-tuned mode. On an 8000x8333 orthophoto the default
        // (Balanced) took 4.9s for a file only 3% smaller, and the result also
        // decodes slower.
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
    }
    Ok(bytes)
}

/// A locator into a temporary, immutable OMF archive. No geometry or image
/// bytes are retained by an unloaded explorer entry.
#[derive(Clone, Debug)]
pub(crate) struct DeferredAsset {
    pub(crate) backing: crate::model::asset_storage::Backing,
    pub(crate) element_path: Vec<usize>,
    /// The element itself, moved out of the index parse that located it, so
    /// loading the payload or copying it into a save need not parse the whole
    /// project index again. Empty until that parse finishes, and for elements
    /// not worth retaining; either way `element_path` still finds it.
    indexed: Arc<OnceLock<IndexedElement>>,
}

/// An archive element, with the project fields its decode depends on.
#[derive(Debug)]
struct IndexedElement {
    element: omf_crate::Element,
    origin: [f64; 3],
    coordinate_reference_system: String,
    units: String,
}

impl DeferredAsset {
    pub(crate) fn new(backing: crate::model::asset_storage::Backing, element_path: Vec<usize>) -> Self {
        Self {
            backing,
            element_path,
            indexed: Arc::default(),
        }
    }

    pub(crate) fn read(&self) -> Result<ImportBundle> {
        // Natively this opens the file for positioned reads, so only the
        // element's own arrays come off disk rather than the whole archive.
        let mut reader = omf_crate::file::Reader::new(self.backing.open()?)?;
        reader.set_limits(reader_limits());
        let parsed;
        let (element, origin, crs, units) = match self.indexed.get() {
            Some(indexed) => (&indexed.element, indexed.origin, &indexed.coordinate_reference_system, &indexed.units),
            None => {
                parsed = reader.project()?.0;
                let element = locate_element(&parsed.elements, &self.element_path).context("unloaded asset is missing from its backing archive")?;
                (element, parsed.origin, &parsed.coordinate_reference_system, &parsed.units)
            }
        };
        let mut decoder = Decoder {
            reader: &reader,
            project_origin: DVec3::from_array(origin),
            project_crs: crs.clone(),
            project_units: units.clone(),
            source_name: "asset.omf",
            bundle: ImportBundle::default(),
            generic_design: Document::new(),
            backing: None,
            element_path: Vec::new(),
            indexed: Vec::new(),
        };
        decoder.walk(element)?;
        decoder.finish()
    }
}

/// Follow `path` through nested composite elements, mutably.
fn locate_element_mut<'a>(elements: &'a mut [omf_crate::Element], path: &[usize]) -> Option<&'a mut omf_crate::Element> {
    let (&first, rest) = path.split_first()?;
    let mut element = elements.get_mut(first)?;
    for &index in rest {
        let omf_crate::Geometry::Composite(group) = &mut element.geometry else {
            return None;
        };
        element = group.elements.get_mut(index)?;
    }
    Some(element)
}

/// Follow `path` through nested composite elements.
fn locate_element<'a>(elements: &'a [omf_crate::Element], path: &[usize]) -> Option<&'a omf_crate::Element> {
    let (&first, rest) = path.split_first()?;
    let mut element = elements.get(first)?;
    for &index in rest {
        let omf_crate::Geometry::Composite(group) = &element.geometry else {
            return None;
        };
        element = group.elements.get(index)?;
    }
    Some(element)
}

/// The archive element an item's payload was read from, and which payload
/// that was, so a save can copy the element's already-compressed arrays
/// rather than compress an unchanged payload all over again.
///
/// Payloads are identified by allocation, not content: an edit replaces the
/// payload `Arc`s rather than mutating them. Holding a `Weak` to each keeps
/// the allocation reserved, so a later payload can never land at the same
/// address and pass for the original, and it makes `Arc::make_mut` copy
/// instead of writing in place.
#[derive(Clone, Debug)]
pub(crate) struct PayloadSource {
    asset: DeferredAsset,
    /// The payload decoded from `asset`, or `None` while the item is
    /// unloaded - a payload that only exists in a backing cannot change.
    resident: Option<PayloadIdentity>,
}

impl PayloadSource {
    /// Record `asset` as the source of `item`'s payload as it stands now.
    pub(crate) fn for_triangulation(asset: Option<DeferredAsset>, item: &OpenTriangulation) -> Option<Self> {
        Some(Self {
            asset: asset?,
            resident: item.state.deferred.is_none().then(|| PayloadIdentity::triangulation(item)),
        })
    }

    /// Record `asset` as the source of `item`'s payload as it stands now.
    pub(crate) fn for_point_cloud(asset: Option<DeferredAsset>, item: &OpenPointCloud) -> Option<Self> {
        Some(Self {
            asset: asset?,
            resident: item.state.deferred.is_none().then(|| PayloadIdentity::point_cloud(item)),
        })
    }

    /// Record `asset` as the source of `item`'s payload as it stands now.
    pub(crate) fn for_raster(asset: Option<DeferredAsset>, item: &OpenRasterTexture) -> Option<Self> {
        Some(Self {
            asset: asset?,
            resident: item.state.deferred.is_none().then(|| PayloadIdentity::raster(item)),
        })
    }

    /// The archive element that still holds exactly the payload `current`
    /// names, if there is one.
    fn unchanged(&self, deferred: bool, current: &PayloadIdentity) -> Option<&DeferredAsset> {
        let unchanged = match &self.resident {
            Some(resident) => !deferred && resident.same(current),
            None => deferred,
        };
        unchanged.then_some(&self.asset)
    }

    /// Carry the record across the payload being unloaded. It survives only
    /// if the payload being dropped is still the one read from the archive.
    pub(crate) fn released(self, current: &PayloadIdentity) -> Option<Self> {
        match &self.resident {
            Some(resident) if resident.same(current) => Some(Self { resident: None, ..self }),
            _ => None,
        }
    }

    /// Carry the record across an unloaded payload being read back in.
    pub(crate) fn restored(self, current: PayloadIdentity) -> Option<Self> {
        self.resident.is_none().then_some(Self { resident: Some(current), ..self })
    }
}

/// Which payload allocations an item holds; see [`PayloadSource`].
#[derive(Clone, Debug)]
pub(crate) enum PayloadIdentity {
    Triangulation(std::sync::Weak<Triangulation>),
    PointCloud {
        points: std::sync::Weak<Vec<DVec3>>,
        colors: Option<std::sync::Weak<Vec<u32>>>,
        classifications: Option<std::sync::Weak<Vec<u8>>>,
    },
    Raster(std::sync::Weak<Vec<u8>>),
}

impl PayloadIdentity {
    pub(crate) fn triangulation(item: &OpenTriangulation) -> Self {
        Self::Triangulation(Arc::downgrade(&item.mesh))
    }

    pub(crate) fn point_cloud(item: &OpenPointCloud) -> Self {
        Self::PointCloud {
            points: Arc::downgrade(&item.points),
            colors: item.colors.as_ref().map(Arc::downgrade),
            classifications: item.classifications.as_ref().map(Arc::downgrade),
        }
    }

    pub(crate) fn raster(item: &OpenRasterTexture) -> Self {
        Self::Raster(Arc::downgrade(&item.full_rgba))
    }

    fn same(&self, other: &Self) -> bool {
        fn same_optional<T>(a: &Option<std::sync::Weak<T>>, b: &Option<std::sync::Weak<T>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => a.ptr_eq(b),
                _ => false,
            }
        }
        match (self, other) {
            (Self::Triangulation(a), Self::Triangulation(b)) => a.ptr_eq(b),
            (
                Self::PointCloud {
                    points: a,
                    colors: a_colors,
                    classifications: a_classes,
                },
                Self::PointCloud {
                    points: b,
                    colors: b_colors,
                    classifications: b_classes,
                },
            ) => a.ptr_eq(b) && same_optional(a_colors, b_colors) && same_optional(a_classes, b_classes),
            (Self::Raster(a), Self::Raster(b)) => a.ptr_eq(b),
            _ => false,
        }
    }
}

type SourceReader = omf_crate::file::Reader<crate::model::asset_storage::BackingData>;

/// Archives a save copies unchanged payloads out of, each opened once however
/// many items it holds. The index is parsed only for an item whose locator did
/// not retain its element.
#[derive(Default)]
struct SourceArchives(Vec<(crate::model::asset_storage::Backing, SourceReader, OnceLock<omf_crate::Project>)>);

impl SourceArchives {
    /// The archive element holding `state`'s payload, when `current` is still
    /// exactly the payload read from it.
    ///
    /// A backing that cannot be read is logged and treated as no source: the
    /// payload can still be encoded afresh, so it must not fail the save.
    fn unchanged<'a>(&'a mut self, state: &'a project::ProjectItemState, current: &PayloadIdentity) -> Option<(&'a SourceReader, &'a omf_crate::Element)> {
        let asset = state.payload_source.as_ref()?.unchanged(state.deferred.is_some(), current)?;
        let index = match self.0.iter().position(|(backing, ..)| backing.same(&asset.backing)) {
            Some(index) => index,
            None => {
                let opened = asset.backing.open().and_then(|data| {
                    let mut reader = omf_crate::file::Reader::new(data)?;
                    reader.set_limits(reader_limits());
                    Ok(reader)
                });
                match opened {
                    Ok(reader) => self.0.push((asset.backing.clone(), reader, OnceLock::new())),
                    Err(error) => {
                        log::warn!("Encoding an unchanged item afresh: its source archive could not be read: {error:#}");
                        return None;
                    }
                }
                self.0.len() - 1
            }
        };
        let (_, reader, project) = &self.0[index];
        let (origin, element) = match asset.indexed.get() {
            Some(indexed) => (indexed.origin, &indexed.element),
            None => {
                if project.get().is_none() {
                    match reader.project() {
                        Ok((parsed, _)) => {
                            let _ = project.set(parsed);
                        }
                        Err(error) => {
                            log::warn!("Encoding an unchanged item afresh: its source archive index could not be read: {error:#}");
                            return None;
                        }
                    }
                }
                let project = project.get()?;
                (project.origin, locate_element(&project.elements, &asset.element_path)?)
            }
        };
        // Saved coordinates are relative to a zero project origin; arrays
        // stored against any other origin would move.
        if origin != [0.0; 3] {
            return None;
        }
        Some((reader, element))
    }
}

/// Copy an unchanged raster's image from the element it was read from, the
/// one array worth copying: re-encoding a large orthophoto costs far more than
/// the rest of a save. `None` when the element is not in the shape
/// [`write_raster`] produces.
fn copy_raster_image<W: Write + Seek + Send>(
    writer: &mut omf_crate::file::Writer<W>,
    reader: &SourceReader,
    element: &omf_crate::Element,
) -> Result<Option<omf_crate::Array<omf_crate::array_type::Image>>> {
    if kind(element) != Some("raster") {
        return Ok(None);
    }
    let [attribute] = element.attributes.as_slice() else {
        return Ok(None);
    };
    let omf_crate::AttributeData::MappedTexture { image, .. } = &attribute.data else {
        return Ok(None);
    };
    Ok(Some(writer.array_copy(reader, image)?))
}

/// Copy an unchanged triangulation's arrays from the element it was read
/// from. `None` when that element is not exactly what [`write_triangulation`]
/// would produce, so the caller encodes the mesh instead.
fn copy_surface<W: Write + Seek + Send>(writer: &mut omf_crate::file::Writer<W>, reader: &SourceReader, element: &omf_crate::Element) -> Result<Option<omf_crate::Geometry>> {
    let omf_crate::Geometry::Surface(surface) = &element.geometry else {
        return Ok(None);
    };
    if kind(element) != Some("triangulation") || surface.origin != [0.0; 3] || !element.attributes.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        omf_crate::Surface::new(writer.array_copy(reader, &surface.vertices)?, writer.array_copy(reader, &surface.triangles)?).into(),
    ))
}

/// Copy an unchanged point cloud's arrays from the element it was read from.
/// `None` when that element is not in the shape [`write_point_cloud`]
/// produces, so the caller encodes the cloud instead.
///
/// The payload itself is not consulted - an unloaded one is not there to
/// consult - and needn't be: it is whatever the decoder made of this element,
/// and will be again when the copy is opened.
fn copy_point_set<W: Write + Seek + Send>(
    writer: &mut omf_crate::file::Writer<W>,
    reader: &SourceReader,
    element: &omf_crate::Element,
) -> Result<Option<(omf_crate::Geometry, Vec<omf_crate::Attribute>)>> {
    let omf_crate::Geometry::PointSet(points) = &element.geometry else {
        return Ok(None);
    };
    let written_by_incline = |attribute: &omf_crate::Attribute| {
        attribute.location == omf_crate::Location::Vertices
            && match &attribute.data {
                omf_crate::AttributeData::Color { .. } => attribute.name == "Color",
                omf_crate::AttributeData::Category { attributes, .. } => {
                    attribute.name == POINT_CLASSIFICATION_ATTRIBUTE && attributes.iter().all(|code| matches!(code.data, omf_crate::AttributeData::Number { colormap: None, .. }))
                }
                _ => false,
            }
    };
    if kind(element) != Some("point_cloud") || points.origin != [0.0; 3] || !element.attributes.iter().all(written_by_incline) {
        return Ok(None);
    }
    let mut attributes = Vec::with_capacity(element.attributes.len());
    for attribute in &element.attributes {
        let data = match &attribute.data {
            omf_crate::AttributeData::Color { values } => omf_crate::AttributeData::Color {
                values: writer.array_copy(reader, values)?,
            },
            omf_crate::AttributeData::Category {
                values,
                names,
                gradient,
                attributes: codes,
            } => {
                let mut copied_codes = Vec::with_capacity(codes.len());
                for code in codes {
                    let omf_crate::AttributeData::Number { values, .. } = &code.data else {
                        unreachable!("checked above");
                    };
                    let mut copied = code.clone();
                    copied.data = omf_crate::AttributeData::Number {
                        values: writer.array_copy(reader, values)?,
                        colormap: None,
                    };
                    copied_codes.push(copied);
                }
                omf_crate::AttributeData::Category {
                    values: writer.array_copy(reader, values)?,
                    names: writer.array_copy(reader, names)?,
                    gradient: gradient.as_ref().map(|gradient| writer.array_copy(reader, gradient)).transpose()?,
                    attributes: copied_codes,
                }
            }
            _ => unreachable!("checked above"),
        };
        let mut copied = attribute.clone();
        copied.data = data;
        attributes.push(copied);
    }
    Ok(Some((omf_crate::PointSet::new(writer.array_copy(reader, &points.vertices)?).into(), attributes)))
}

fn reader_limits() -> omf_crate::file::Limits {
    omf_crate::file::Limits {
        json_bytes: Some(64 * 1024 * 1024),
        image_bytes: Some(4 * 1024 * 1024 * 1024),
        image_dim: Some(65_536),
        validation: Some(1_000),
    }
}

/// Read an OMF 2 container.
pub(crate) fn from_bytes(source_name: &str, bytes: Vec<u8>, progress: &Phase) -> Result<ImportBundle> {
    progress.set_fraction(0.0);
    reject_omf1(&bytes).with_context(|| format!("read OMF container {source_name}"))?;
    let backing = crate::model::asset_storage::Backing::write(&bytes)?;
    let mut reader = omf_crate::file::Reader::new(bytes).context("open OMF archive")?;
    reader.set_limits(reader_limits());
    let (project, problems) = reader.project().context("read project index")?;
    let mut bundle = ImportBundle {
        project_name: project.name.clone(),
        coordinate_reference_system: project.coordinate_reference_system.clone(),
        units: project.units.clone(),
        origin: project.origin,
        ..Default::default()
    };
    let written_by_incline = project.application.starts_with("Incline ");
    if !project.description.trim().is_empty() && !written_by_incline {
        bundle.warnings.push(tr!(literal = "Project description is not retained"));
    }
    if !project.author.trim().is_empty() {
        bundle.warnings.push(tr!(literal = "Project author is not retained"));
    }
    if !project.application.trim().is_empty() && !project.application.starts_with("Incline ") {
        bundle.warnings.push(tr_format!(
            literal = "Project application metadata '%application%' is not retained",
            application = &project.application
        ));
    }
    // Parsed before any element is walked, so a name on an element's own
    // META_FOLDER below always has something to resolve against.
    if let Some(sections) = project.metadata.get(META_FOLDERS).and_then(Value::as_object) {
        for (key, names) in sections {
            let Some(section) = SectionKind::from_key(key) else { continue };
            for name in names.as_array().into_iter().flatten().filter_map(Value::as_str) {
                bundle.folders.ensure(section, name);
            }
        }
    }
    let unsupported_project_metadata = project.metadata.keys().filter(|key| key.as_str() != META_FOLDERS).cloned().collect::<Vec<_>>();
    if !unsupported_project_metadata.is_empty() {
        bundle.warnings.push(tr_format!(
            literal = "Project has unsupported metadata keys: %keys%",
            keys = unsupported_project_metadata.join(", ")
        ));
    }
    if !problems.is_empty() {
        bundle
            .warnings
            .push(tr_format!(literal = "OMF validation warnings: %warnings%", warnings = format!("{problems:?}")));
    }
    let mut decoder = Decoder {
        reader: &reader,
        project_origin: DVec3::from_array(project.origin),
        project_crs: project.coordinate_reference_system,
        project_units: project.units,
        source_name,
        bundle,
        generic_design: Document::new(),
        backing: None,
        element_path: Vec::new(),
        indexed: Vec::new(),
    };
    decoder.backing = Some(backing);
    let total = project.elements.len().max(1) as u64;
    for (index, element) in project.elements.iter().enumerate() {
        decoder.element_path = vec![index];
        decoder.walk(element)?;
        progress.set_items(index as u64 + 1, total);
    }
    let mut elements = project.elements;
    for (path, slot) in std::mem::take(&mut decoder.indexed) {
        if let Some(element) = locate_element_mut(&mut elements, &path) {
            let element = std::mem::replace(element, omf_crate::Element::new(String::new(), omf_crate::Composite::new(Vec::new())));
            let _ = slot.set(IndexedElement {
                element,
                origin: project.origin,
                coordinate_reference_system: decoder.project_crs.clone(),
                units: decoder.project_units.clone(),
            });
        }
    }
    decoder.finish()
}

/// OMF 1 containers open with this magic and an `OMF-v...` version string.
/// Incline Design reads OMF 2 only, so they are named in the error rather than left
/// to fail later as an unrecognised archive.
fn reject_omf1(bytes: &[u8]) -> Result<()> {
    const OMF1_MAGIC: [u8; 4] = [0x84, 0x83, 0x82, 0x81];
    if !bytes.starts_with(&OMF1_MAGIC) {
        return Ok(());
    }
    let version = String::from_utf8_lossy(bytes.get(4..36).unwrap_or_default());
    let version = version.trim_end_matches('\0');
    let detail = if version.starts_with("OMF-") { format!(" ({version})") } else { String::new() };
    bail!("This is an OMF 1 file{detail}. Incline Design reads OMF 2 only; re-export it as OMF 2 from the application that wrote it.");
}

struct Decoder<'a, R: omf_crate::file::ReadAt> {
    reader: &'a omf_crate::file::Reader<R>,
    project_origin: DVec3,
    project_crs: String,
    project_units: String,
    source_name: &'a str,
    bundle: ImportBundle,
    generic_design: Document,
    backing: Option<crate::model::asset_storage::Backing>,
    element_path: Vec<usize>,
    /// Slots of the locators handed out, by element path, to be filled with
    /// their elements once the walk no longer borrows the project.
    indexed: Vec<(Vec<usize>, Arc<OnceLock<IndexedElement>>)>,
}

impl<R: omf_crate::file::ReadAt> Decoder<'_, R> {
    fn finish(mut self) -> Result<ImportBundle> {
        if !self.generic_design.layers().is_empty() {
            self.generic_design.validate().context("validate OMF line-set designs")?;
            self.bundle.designs.push(ProjectFile {
                format_version: project::PROJECT_FORMAT_VERSION,
                document: self.generic_design,
                metadata: ProjectMetadata {
                    name: file_stem(self.source_name),
                    coordinate_reference_system: self.project_crs.clone(),
                    units: self.project_units.clone(),
                },
                // The bundle's registry is the single source of truth for
                // membership; a decoded `ProjectFile` never carries its own.
                folders: FolderRegistry::default(),
            });
        }
        Ok(self.bundle)
    }

    fn walk(&mut self, element: &omf_crate::Element) -> Result<()> {
        self.record_unsupported_content(element);
        if self.backing.is_some() && !style_loaded(element.metadata.get(META_STYLE)) && self.defer_element(element)? {
            return Ok(());
        }
        match kind(element) {
            Some("designs" | "design_database") => {
                let design = self.read_design(element)?;
                self.bundle.designs.push(design);
                return Ok(());
            }
            Some("drillhole_dataset") => {
                if let Some(drill_holes) = self.read_drill_dataset(element)? {
                    self.bundle.drill_holes.push(drill_holes);
                }
                return Ok(());
            }
            Some("raster") => {
                self.read_textures(element)?;
                return Ok(());
            }
            _ => {}
        }

        match &element.geometry {
            omf_crate::Geometry::Surface(surface) => self.read_surface(element, surface)?,
            omf_crate::Geometry::GridSurface(surface) => self.read_grid_surface(element, surface)?,
            omf_crate::Geometry::PointSet(points) => self.read_point_cloud(element, points)?,
            omf_crate::Geometry::LineSet(lines) => self.read_generic_lines(element, lines)?,
            omf_crate::Geometry::BlockModel(model) => self.read_block_model(element, model)?,
            omf_crate::Geometry::Composite(composite) => {
                for (index, child) in composite.elements.iter().enumerate() {
                    self.element_path.push(index);
                    self.walk(child)?;
                    self.element_path.pop();
                }
            }
        }
        if !element.attributes.is_empty() {
            self.read_textures(element)?;
        }
        Ok(())
    }

    /// The folder `element`'s [`META_FOLDER`] name resolves to in `section`,
    /// minting the folder if the project record's list did not already carry
    /// it - so membership can never dangle past the name a file recorded for
    /// it. `None` when the element is at the section root.
    fn element_folder(&mut self, element: &omf_crate::Element, section: SectionKind) -> Option<FolderId> {
        element
            .metadata
            .get(META_FOLDER)
            .and_then(Value::as_str)
            .and_then(|name| self.bundle.folders.ensure(section, name))
    }

    /// The section `element` says its item is shown under, or the one its kind
    /// naturally sits in when it says nothing.
    fn element_section(&mut self, element: &omf_crate::Element, kind: MemberKind) -> SectionKind {
        let natural = SectionKind::natural_for(kind);
        let Some(key) = element.metadata.get(META_SECTION).and_then(Value::as_str) else {
            return natural;
        };
        let section = SectionKind::from_key(key).unwrap_or_else(|| {
            self.bundle.warnings.push(tr_format!(
                literal = "Element '%name%' names an unknown section '%section%'",
                name = &element.name,
                section = key
            ));
            natural
        });
        if section.admits(kind) {
            return section;
        }
        self.bundle.warnings.push(tr_format!(
            literal = "Element '%name%' names section '%section%' which cannot show this kind of item in this build",
            name = &element.name,
            section = key
        ));
        section.healed_for(kind)
    }

    /// Where the element being walked sits in the opened archive, when there
    /// is a backing copy of it to find it in again.
    fn payload_locator(&mut self) -> Option<DeferredAsset> {
        let locator = DeferredAsset::new(self.backing.clone()?, self.element_path.clone());
        self.indexed.push((locator.element_path.clone(), locator.indexed.clone()));
        Some(locator)
    }

    /// Construct only explorer metadata. In particular, do not read any
    /// Parquet arrays, mesh accelerators, drill traces or raster pixels here.
    fn defer_element(&mut self, element: &omf_crate::Element) -> Result<bool> {
        use crate::model::{
            asset_residency::AssetSummary,
            block_model::{BlockBoundsSource, BlockModelSource},
            formats::block_model_data::{BlockModelMetadata, BlockVariable},
        };
        let style = element.metadata.get(META_STYLE);
        let name = element_name(element).to_owned();
        let preferred_id = element_id(element);
        let source_name = element_source_name(element);
        let source_format = element_source_format(element);
        let locator = self.payload_locator().context("missing asset backing")?;
        let path = virtual_path(self.source_name, &name, "omf");
        if kind(element) == Some("raster") {
            let section = self.element_section(element, MemberKind::Raster);
            let folder = self.element_folder(element, section);
            self.bundle.rasters.push(ImportedRaster {
                preferred_id,
                source_name,
                source_format,
                is_loaded: false,
                payload_source: Some(locator.clone()),
                deferred: Some((locator, AssetSummary::default())),
                folder,
                section,
                loaded: LoadedRasterTexture {
                    name,
                    path,
                    source_size: style_value(style, "source_size").unwrap_or([0; 2]),
                    preview_size: style_value(style, "preview_size").unwrap_or([0; 2]),
                    full_rgba: Arc::new(Vec::new()),
                    rgba: Arc::new(Vec::new()),
                    world_to_uv: style_value(style, "world_to_uv").unwrap_or([0.0; 6]),
                    projection: style_value(style, "projection").unwrap_or_else(|| self.project_crs.clone()),
                    driver_name: tr!(literal = "OMF texture"),
                },
            });
            return Ok(true);
        }
        if kind(element) == Some("drillhole_dataset") {
            let count = match &element.geometry {
                omf_crate::Geometry::Composite(group) => group
                    .elements
                    .iter()
                    .find(|child| kind(child) == Some("drillhole_collars"))
                    .and_then(|collars| match &collars.geometry {
                        omf_crate::Geometry::PointSet(points) => Some(points.vertices.item_count() as usize),
                        _ => None,
                    })
                    .unwrap_or_default(),
                _ => return Ok(false),
            };
            let section = self.element_section(element, MemberKind::DrillHole);
            let folder = self.element_folder(element, section);
            self.bundle.drill_holes.push(ImportedDrillHoles {
                preferred_id,
                source_name,
                source_format,
                is_loaded: false,
                deferred: Some((
                    locator,
                    AssetSummary {
                        primary_count: count,
                        ..Default::default()
                    },
                )),
                loaded: LoadedDrillHoleDataset {
                    source: DrillHoleSource::Omf { name: name.clone(), path },
                    name,
                    dataset: Arc::new(DrillHoleDataset::new(Vec::new())),
                },
                color: style_value(style, "color").unwrap_or_default(),
                folder,
                section,
            });
            return Ok(true);
        }
        match &element.geometry {
            omf_crate::Geometry::Surface(_) | omf_crate::Geometry::GridSurface(_) => {
                let (vertices, faces) = match &element.geometry {
                    omf_crate::Geometry::Surface(surface) => (surface.vertices.item_count() as usize, surface.triangles.item_count() as usize),
                    _ => (0, 0),
                };
                let mesh = Arc::new(Triangulation::empty());
                let spatial = Arc::new(crate::model::spatial::TriangleBvh::build(&mesh));
                let section = self.element_section(element, MemberKind::Triangulation);
                let folder = self.element_folder(element, section);
                self.bundle.triangulations.push(ImportedTriangulation {
                    preferred_id,
                    source_name,
                    source_format,
                    is_loaded: false,
                    payload_source: Some(locator.clone()),
                    deferred: Some((
                        locator,
                        AssetSummary {
                            primary_count: vertices,
                            secondary_count: faces,
                        },
                    )),
                    loaded: LoadedTriangulation {
                        name,
                        path,
                        mesh,
                        spatial,
                        edges: Vec::new(),
                        surface_face_order: Arc::new(Vec::new()),
                    },
                    color: style_value(style, "color").unwrap_or_else(|| element_color(element, [0.65, 0.68, 0.72, 1.0])),
                    line_color: style_value(style, "line_color").unwrap_or([0.05, 0.08, 0.10, 1.0]),
                    line_weight: style_value(style, "line_weight").unwrap_or(Some(1.0)),
                    raster_opacity: style_f32(style, "raster_opacity").unwrap_or(1.0),
                    raster_texture_id: style_value(style, "raster_texture_id"),
                    folder,
                    section,
                });
            }
            omf_crate::Geometry::PointSet(points) => {
                let bounds = (DVec3::ZERO, DVec3::ZERO);
                let section = self.element_section(element, MemberKind::PointCloud);
                let folder = self.element_folder(element, section);
                self.bundle.point_clouds.push(ImportedPointCloud {
                    preferred_id,
                    source_name,
                    source_format,
                    is_loaded: false,
                    payload_source: Some(locator.clone()),
                    deferred: Some((
                        locator,
                        AssetSummary {
                            primary_count: points.vertices.item_count() as usize,
                            ..Default::default()
                        },
                    )),
                    loaded: LoadedPointCloud {
                        name,
                        path,
                        points: Arc::new(Vec::new()),
                        colors: None,
                        classifications: None,
                        prepared: Arc::new(prepare_for_render(&[], None, None, bounds)),
                        bounds,
                    },
                    color: style_value(style, "color").unwrap_or_else(|| element_color(element, [0.85, 0.87, 0.9, 1.0])),
                    point_size: style_f32(style, "point_size").unwrap_or(0.1),
                    folder,
                    section,
                });
            }
            omf_crate::Geometry::BlockModel(geometry) => {
                let count = match &geometry.subblocks {
                    Some(omf_crate::Subblocks::Freeform { subblocks }) => subblocks.item_count() as usize,
                    Some(omf_crate::Subblocks::Regular { subblocks, .. }) => subblocks.item_count() as usize,
                    None => geometry
                        .grid
                        .count()
                        .into_iter()
                        .try_fold(1usize, |n, axis| n.checked_mul(axis as usize))
                        .context("block count overflow")?,
                };
                let variables: Vec<_> = element
                    .attributes
                    .iter()
                    .filter(|attribute| matches!(attribute.data, omf_crate::AttributeData::Number { .. } | omf_crate::AttributeData::Category { .. }))
                    .map(|attribute| BlockVariable {
                        name: attribute.name.clone(),
                        ..Default::default()
                    })
                    .collect();
                let variable_count = variables.len();
                let active_color_variable = style_value::<Option<String>>(style, "active_color_variable")
                    .flatten()
                    .or_else(|| variables.first().map(|variable| variable.name.clone()));
                let section = self.element_section(element, MemberKind::BlockModel);
                let folder = self.element_folder(element, section);
                self.bundle.block_models.push(ImportedBlockModel {
                    preferred_id,
                    source_name,
                    source_format,
                    is_loaded: false,
                    deferred: Some((
                        locator,
                        AssetSummary {
                            primary_count: count,
                            secondary_count: variable_count,
                        },
                    )),
                    loaded: LoadedBlockModel {
                        name,
                        source: BlockModelSource { path, csv_columns: None },
                        model: BlockModelData::unloaded(BlockModelMetadata {
                            n_blocks: count,
                            variables,
                            dims: geometry.grid.count().map(|n| n as usize),
                            ..Default::default()
                        }),
                        blocks: Arc::new(BlockBoundsSource::Explicit(Vec::new())),
                        renderable_block_indices: Arc::new(RenderableBlockIndices::All(0)),
                        uniform_grid: None,
                        opaque_surface_blocks: None,
                        world_bounds: None,
                        active_color_variable,
                        active_values_cache: Default::default(),
                    },
                    color: style_value(style, "color").unwrap_or_else(|| element_color(element, [0.72, 0.72, 0.75, 1.0])),
                    slice: style_value(style, "slice"),
                    color_transfers: BTreeMap::new(),
                    hide_empty_color_values: style_bool(style, "hide_empty_color_values").unwrap_or(true),
                    folder,
                    section,
                });
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn record_unsupported_content(&mut self, element: &omf_crate::Element) {
        const KNOWN_METADATA: &[&str] = &[
            META_KIND,
            META_NAME,
            META_OBJECTS,
            META_LAYER,
            META_FOLDERS,
            META_FOLDER,
            META_SECTION,
            META_SOURCE,
            META_STYLE,
            META_ID,
            META_RENDER_RANGES,
            META_TIE_INS,
        ];
        let unknown_metadata = element.metadata.keys().filter(|key| !KNOWN_METADATA.contains(&key.as_str())).cloned().collect::<Vec<_>>();
        if !unknown_metadata.is_empty() {
            self.bundle
                .warnings
                .push(format!("Element '{}' has unsupported metadata keys: {}", element.name, unknown_metadata.join(", ")));
        }
        if !element.description.trim().is_empty() {
            self.bundle
                .warnings
                .push(format!("Element '{}' has a description that Incline Design does not retain", element.name));
        }

        let incline_design_kind = kind(element);
        let unsupported_attributes = element
            .attributes
            .iter()
            .filter(|attribute| {
                if matches!(
                    incline_design_kind,
                    Some("designs" | "design_database" | "design_lines" | "design_points" | "drillhole_dataset" | "raster")
                ) {
                    return false;
                }
                match &element.geometry {
                    omf_crate::Geometry::Surface(_) | omf_crate::Geometry::GridSurface(_) => !matches!(
                        attribute.data,
                        omf_crate::AttributeData::MappedTexture { .. } | omf_crate::AttributeData::ProjectedTexture { .. }
                    ),
                    omf_crate::Geometry::PointSet(_) => match attribute.data {
                        omf_crate::AttributeData::Color { .. } => false,
                        // Decoded by `read_point_classification`.
                        omf_crate::AttributeData::Category { .. } => attribute.name != POINT_CLASSIFICATION_ATTRIBUTE || attribute.location != omf_crate::Location::Vertices,
                        _ => true,
                    },
                    omf_crate::Geometry::BlockModel(_) => !matches!(attribute.data, omf_crate::AttributeData::Number { .. } | omf_crate::AttributeData::Category { .. }),
                    omf_crate::Geometry::LineSet(_) | omf_crate::Geometry::Composite(_) => true,
                }
            })
            .map(|attribute| attribute.name.clone())
            .collect::<Vec<_>>();
        if !unsupported_attributes.is_empty() {
            self.bundle.warnings.push(format!(
                "Element '{}' has unsupported attributes that will be omitted: {}",
                element.name,
                unsupported_attributes.join(", ")
            ));
        }
        for attribute in &element.attributes {
            if !attribute.description.trim().is_empty() || !attribute.metadata.is_empty() {
                self.bundle.warnings.push(format!(
                    "Attribute '{}' on element '{}' has descriptive metadata that Incline Design does not retain",
                    attribute.name, element.name
                ));
            }
        }
    }

    fn read_design(&mut self, element: &omf_crate::Element) -> Result<ProjectFile> {
        let omf_crate::Geometry::Composite(composite) = &element.geometry else {
            bail!("Incline Design designs element '{}' is not an OMF composite", element.name);
        };
        let mut document = Document::new();
        // Legacy migration path: a file written before folders covered every
        // section carried the Designs list here instead of the project
        // record. `ensure` is idempotent, so carrying both keys is harmless.
        if let Some(names) = element.metadata.get(META_FOLDERS).and_then(|value| Vec::<String>::deserialize(value).ok()) {
            for name in names {
                self.bundle.folders.ensure(SectionKind::Designs, &name);
            }
        }
        for layer_element in &composite.elements {
            self.record_unsupported_content(layer_element);
            // Resolved first: `set_layer_section` below clears the layer's
            // folder when the section actually changes, so the folder must be
            // resolved against the section the layer ends up in, not Designs.
            let section = self.element_section(layer_element, MemberKind::Layer);
            let mut layer_template = layer_element.metadata.get(META_LAYER).and_then(|value| Layer::deserialize(value).ok());
            if let Some(layer) = layer_template.as_mut() {
                layer.elevation += self.project_origin.z as f32;
            }
            let color = layer_template
                .as_ref()
                .map(|layer| layer.color)
                .unwrap_or_else(|| element_color(layer_element, [1.0, 1.0, 1.0, 1.0]));
            let layer_id = if let Some(layer) = layer_template.as_ref()
                && layer.id.0 <= u32::MAX as u64
                && document.layer(layer.id).is_none()
            {
                document.append_layer_snapshot(layer, std::iter::empty());
                layer.id
            } else {
                document.add_layer(
                    layer_template
                        .as_ref()
                        .map(|layer| layer.name.clone())
                        .unwrap_or_else(|| element_name(layer_element).to_owned()),
                    layer_template.as_ref().and_then(|layer| layer.color_index),
                    color,
                    layer_template.as_ref().is_none_or(|layer| layer.loaded),
                    layer_template.as_ref().map_or(0.0, |layer| layer.elevation),
                )
            };
            // An absent META_SECTION means the layer sits where its kind
            // naturally does - a file written before sections were tagged
            // loads every layer at Designs, same as before.
            document.set_layer_section(layer_id, section);
            // An absent key means root; a file written before folders existed
            // must load with every layer there. A name the project record did
            // not list is minted here so membership can never dangle.
            if let Some(folder) = layer_element.metadata.get(META_FOLDER).and_then(Value::as_str)
                && let Some(folder) = self.bundle.folders.ensure(section, folder)
            {
                document.set_layer_folder(layer_id, Some(folder));
            }
            let children = match &layer_element.geometry {
                omf_crate::Geometry::Composite(layer) => layer.elements.as_slice(),
                _ => std::slice::from_ref(layer_element),
            };
            let mut lines = None;
            let mut points = None;
            for child in children {
                if !std::ptr::eq(layer_element, child) {
                    self.record_unsupported_content(child);
                }
                match (kind(child), &child.geometry) {
                    (Some("design_lines"), omf_crate::Geometry::LineSet(set)) => lines = Some((child, set)),
                    (Some("design_points"), omf_crate::Geometry::PointSet(set)) => points = Some((child, set)),
                    _ => self.append_design_geometry(&mut document, layer_id, child)?,
                }
            }
            let Some(records) = layer_element.metadata.get(META_OBJECTS) else {
                continue;
            };
            let records = Vec::<DesignRecord>::deserialize(records).with_context(|| format!("read the objects of design layer '{}'", layer_element.name))?;
            self.read_design_layer(&mut document, layer_id, records, lines, points)
                .with_context(|| format!("read design layer '{}'", layer_element.name))?;
        }
        // Files written before circles were their own variant store them as
        // closed two-vertex bulged polylines. Upgrade on load so no tool
        // downstream has to recognise the old encoding.
        document.promote_compact_circles();
        document.validate().with_context(|| format!("validate designs '{}'", element.name))?;
        let unloaded: Vec<_> = document.layers().iter().filter(|layer| !layer.loaded).map(|layer| layer.id).collect();
        for id in unloaded {
            if self.backing.is_some() {
                let stored = document.layer_payload(id).store()?;
                document.install_deferred_layer(id, stored);
            }
        }

        Ok(ProjectFile {
            format_version: project::PROJECT_FORMAT_VERSION,
            document,
            metadata: ProjectMetadata {
                name: element_name(element).to_owned(),
                coordinate_reference_system: self.project_crs.clone(),
                units: self.project_units.clone(),
            },
            // The bundle's registry is the single source of truth for
            // membership; a decoded `ProjectFile` never carries its own.
            folders: FolderRegistry::default(),
        })
    }

    /// Rebuild a layer written by [`write_design_layer`]: each record, in
    /// order, takes the next run of `Lines` segments or the next `Points` row.
    fn read_design_layer(
        &mut self,
        document: &mut Document,
        layer: crate::model::LayerId,
        records: Vec<DesignRecord>,
        lines: Option<(&omf_crate::Element, &omf_crate::LineSet)>,
        points: Option<(&omf_crate::Element, &omf_crate::PointSet)>,
    ) -> Result<()> {
        let object_column = |element: &omf_crate::Element, name: &str| -> Result<Option<Vec<Option<f64>>>> {
            element
                .attributes
                .iter()
                .find(|attribute| attribute.name == name)
                .and_then(|attribute| match &attribute.data {
                    omf_crate::AttributeData::Number { values, .. } => Some(values),
                    _ => None,
                })
                .map(|values| read_numbers(self.reader, values))
                .transpose()
        };

        let (mut line_vertices, mut segments, mut segment_objects, mut bulges) = (Vec::new(), Vec::new(), Vec::new(), None);
        if let Some((element, set)) = lines {
            let offset = self.project_origin + DVec3::from_array(set.origin);
            line_vertices = self.read_vertices(&set.vertices)?.into_iter().map(|point| point + offset).collect();
            segments = self.reader.array_segments_vec(&set.segments)?;
            segment_objects = object_column(element, DESIGN_OBJECT_ATTRIBUTE)?.context("design lines have no object column")?;
            bulges = object_column(element, DESIGN_BULGE_ATTRIBUTE)?;
            if segment_objects.len() != segments.len() || bulges.as_ref().is_some_and(|bulges| bulges.len() != segments.len()) {
                bail!("design lines have mismatched column lengths");
            }
        }
        let (mut point_vertices, mut point_objects) = (Vec::new(), Vec::new());
        if let Some((element, set)) = points {
            let offset = self.project_origin + DVec3::from_array(set.origin);
            point_vertices = self.read_vertices(&set.vertices)?.into_iter().map(|point| point + offset).collect();
            point_objects = object_column(element, DESIGN_OBJECT_ATTRIBUTE)?.context("design points have no object column")?;
            if point_objects.len() != point_vertices.len() {
                bail!("design points have mismatched column lengths");
            }
        }

        let mut next_segment = 0;
        let mut next_point = 0;
        for record in records {
            let (id, hidden) = match &record {
                DesignRecord::Point { id, hidden, .. }
                | DesignRecord::Polyline { id, hidden, .. }
                | DesignRecord::Circle { id, hidden, .. }
                | DesignRecord::Text { id, hidden, .. } => (*id, *hidden),
            };
            let belongs = |value: Option<f64>| value == Some(id as f64);
            let mut take_point = || -> Result<DVec3> {
                if !point_objects.get(next_point).copied().is_some_and(belongs) {
                    bail!("design object {id} has no point");
                }
                next_point += 1;
                Ok(point_vertices[next_point - 1])
            };
            let run_start = next_segment;
            let mut take_run = || -> Result<std::ops::Range<usize>> {
                while segment_objects.get(next_segment).copied().is_some_and(belongs) {
                    next_segment += 1;
                }
                if next_segment == run_start {
                    bail!("design object {id} has no segments");
                }
                Ok(run_start..next_segment)
            };
            let object_id = crate::model::ObjectId(id);
            let object = match record {
                DesignRecord::Point { color, .. } => Object::Point {
                    id: object_id,
                    layer,
                    pos: take_point()?,
                    color,
                },
                DesignRecord::Text {
                    color, content, height, rotation, ..
                } => Object::Text {
                    id: object_id,
                    layer,
                    pos: take_point()?,
                    content,
                    height,
                    rotation,
                    color,
                },
                DesignRecord::Circle {
                    color,
                    fill,
                    line_weight,
                    center,
                    radius,
                    ..
                } => {
                    take_run()?;
                    Object::Circle {
                        id: object_id,
                        layer,
                        center: center + self.project_origin,
                        radius,
                        color,
                        fill,
                        line_weight,
                    }
                }
                DesignRecord::Polyline { color, fill, line_weight, .. } => {
                    let run = take_run()?;
                    let run_segments = &segments[run.clone()];
                    let first = run_segments[0][0];
                    let closed = run_segments.len() >= 2 && run_segments[run_segments.len() - 1][1] == first;
                    let count = if closed { run_segments.len() } else { run_segments.len() + 1 };
                    let sequential = run_segments[..count - 1]
                        .iter()
                        .enumerate()
                        .all(|(index, segment)| *segment == [first + index as u32, first + index as u32 + 1]);
                    let vertices = line_vertices.get(first as usize..first as usize + count);
                    let (true, Some(vertices)) = (sequential, vertices) else {
                        bail!("design polyline {id} has segments that are not one string");
                    };
                    let bulge = |index: usize| bulges.as_ref().and_then(|bulges| bulges.get(run.start + index).copied().flatten()).unwrap_or(0.0);
                    Object::Polyline {
                        id: object_id,
                        layer,
                        verts: vertices
                            .iter()
                            .enumerate()
                            .map(|(index, pos)| PolyVertex {
                                pos: *pos,
                                bulge: if index < run_segments.len() { bulge(index) } else { 0.0 },
                            })
                            .collect(),
                        closed,
                        color,
                        fill,
                        line_weight,
                    }
                }
            };
            let id = if id <= u32::MAX as u64 && document.get_object(object_id).is_none() {
                object_id
            } else {
                document.allocate_object_id()
            };
            document.insert_object(object.with_id_and_layer(id, layer));
            if hidden {
                document.set_object_hidden(id, true);
            }
        }
        if next_segment != segments.len() || next_point != point_vertices.len() {
            bail!("design layer has geometry that no object record claims");
        }
        Ok(())
    }

    fn append_design_geometry(&mut self, document: &mut Document, layer: crate::model::LayerId, element: &omf_crate::Element) -> Result<()> {
        match &element.geometry {
            omf_crate::Geometry::PointSet(points) => {
                let offset = self.project_origin + DVec3::from_array(points.origin);
                for point in self.read_vertices(&points.vertices)? {
                    let pos = point + offset;
                    let color = ObjectColor::Fixed(element_color(element, [1.0; 4]));
                    document.add_object(|id| Object::Point { id, layer, pos, color });
                }
            }
            omf_crate::Geometry::LineSet(lines) => {
                let offset = self.project_origin + DVec3::from_array(lines.origin);
                let vertices = self.read_vertices(&lines.vertices)?.into_iter().map(|point| point + offset).collect::<Vec<_>>();
                let segments = self.reader.array_segments_vec(&lines.segments)?;
                for (points, closed) in line_strings(&vertices, &segments) {
                    let color = ObjectColor::Fixed(element_color(element, [1.0; 4]));
                    document.add_object(|id| Object::Polyline {
                        id,
                        layer,
                        verts: points.into_iter().map(PolyVertex::straight).collect(),
                        closed,
                        color,
                        fill: FillStyle::Clear,
                        line_weight: 1.0,
                    });
                }
            }
            _ => self
                .bundle
                .warnings
                .push(format!("Skipped unsupported '{}' geometry nested inside the Designs composite", element.name)),
        }
        Ok(())
    }

    fn read_generic_lines(&mut self, element: &omf_crate::Element, lines: &omf_crate::LineSet) -> Result<()> {
        let color = element_color(element, [1.0, 1.0, 1.0, 1.0]);
        let layer = self.generic_design.add_layer(element_name(element).to_owned(), None, color, true, 0.0);
        // Borrow splitting through a temporary avoids borrowing `self` and
        // `generic_design` mutably at the same time.
        let offset = self.project_origin + DVec3::from_array(lines.origin);
        let vertices = self.read_vertices(&lines.vertices)?.into_iter().map(|point| point + offset).collect::<Vec<_>>();
        let segments = self.reader.array_segments_vec(&lines.segments)?;
        for (points, closed) in line_strings(&vertices, &segments) {
            self.generic_design.add_object(|id| Object::Polyline {
                id,
                layer,
                verts: points.into_iter().map(PolyVertex::straight).collect(),
                closed,
                color: ObjectColor::ByLayer,
                fill: FillStyle::Clear,
                line_weight: 1.0,
            });
        }
        Ok(())
    }

    fn read_surface(&mut self, element: &omf_crate::Element, surface: &omf_crate::Surface) -> Result<()> {
        let offset = self.project_origin + DVec3::from_array(surface.origin);
        let vertices = self
            .read_vertices(&surface.vertices)?
            .into_iter()
            .map(|point| Vertex::new(point.x + offset.x, point.y + offset.y, point.z + offset.z))
            .collect();
        let faces = self.reader.array_triangles_vec(&surface.triangles)?;
        self.push_triangulation(element, vertices, faces)
    }

    fn read_grid_surface(&mut self, element: &omf_crate::Element, surface: &omf_crate::GridSurface) -> Result<()> {
        let [u_sizes, v_sizes] = self.grid2_sizes(&surface.grid)?;
        let u_edges = cumulative_edges(&u_sizes);
        let v_edges = cumulative_edges(&v_sizes);
        let width = u_edges.len();
        let height = v_edges.len();
        let vertex_count = width.checked_mul(height).context("OMF grid-surface vertex count overflows")?;
        ensure_items(vertex_count as u64, "grid-surface vertices")?;
        let heights = if let Some(values) = &surface.heights {
            let values = collect_results(self.reader.array_scalars(values)?)?;
            if values.len() != vertex_count {
                bail!("Grid surface '{}' has {} heights for {vertex_count} vertices", element.name, values.len());
            }
            values
        } else {
            vec![0.0; vertex_count]
        };
        let origin = self.project_origin + DVec3::from_array(surface.orient.origin);
        let u = DVec3::from_array(surface.orient.u);
        let v = DVec3::from_array(surface.orient.v);
        let normal = u.cross(v).normalize_or_zero();
        let mut vertices = Vec::with_capacity(vertex_count);
        for (v_index, v_offset) in v_edges.iter().copied().enumerate() {
            for (u_index, u_offset) in u_edges.iter().copied().enumerate() {
                let index = v_index * width + u_index;
                let point = origin + u * u_offset + v * v_offset + normal * heights[index];
                vertices.push(Vertex::new(point.x, point.y, point.z));
            }
        }
        let face_count = u_sizes
            .len()
            .checked_mul(v_sizes.len())
            .and_then(|cells| cells.checked_mul(2))
            .context("OMF grid-surface face count overflows")?;
        ensure_items(face_count as u64, "grid-surface faces")?;
        let mut faces = Vec::with_capacity(face_count);
        for row in 0..v_sizes.len() {
            for column in 0..u_sizes.len() {
                let a = (row * width + column) as u32;
                let b = a + 1;
                let d = ((row + 1) * width + column) as u32;
                let c = d + 1;
                faces.extend([[a, b, c], [a, c, d]]);
            }
        }
        self.push_triangulation(element, vertices, faces)
    }

    fn push_triangulation(&mut self, element: &omf_crate::Element, vertices: Vec<Vertex>, faces: Vec<[u32; 3]>) -> Result<()> {
        let mesh = Triangulation::from_vertices_and_faces(vertices, faces).with_context(|| format!("build OMF surface '{}'", element.name))?;
        let spatial = Arc::new(crate::model::spatial::TriangleBvh::build(&mesh));
        let edges = unique_edges(&mesh);
        let surface_face_order = Arc::new(spatial_surface_face_order(&mesh));
        let style = element.metadata.get(META_STYLE);
        let color = style_value(style, "color").unwrap_or_else(|| element_color(element, [0.65, 0.68, 0.72, 1.0]));
        let section = self.element_section(element, MemberKind::Triangulation);
        let folder = self.element_folder(element, section);
        let payload_source = self.payload_locator();
        self.bundle.triangulations.push(ImportedTriangulation {
            preferred_id: element_id(element),
            source_name: element_source_name(element),
            source_format: element_source_format(element),
            loaded: LoadedTriangulation {
                name: element_name(element).to_owned(),
                path: virtual_path(self.source_name, element_name(element), "obj"),
                mesh: Arc::new(mesh),
                spatial,
                edges,
                surface_face_order,
            },
            deferred: None,
            payload_source,
            is_loaded: style_loaded(style),
            color,
            line_color: style_value(style, "line_color").unwrap_or([0.05, 0.08, 0.10, 1.0]),
            line_weight: style
                .and_then(|value| value.get("line_weight"))
                .and_then(|value| Deserialize::deserialize(value).ok())
                .unwrap_or(Some(1.0)),
            raster_opacity: style_f32(style, "raster_opacity").unwrap_or(1.0),
            raster_texture_id: style_value(style, "raster_texture_id"),
            folder,
            section,
        });
        Ok(())
    }

    fn read_point_cloud(&mut self, element: &omf_crate::Element, points: &omf_crate::PointSet) -> Result<()> {
        let offset = self.project_origin + DVec3::from_array(points.origin);
        let mut positions = self.read_vertices(&points.vertices)?;
        // Applying the offset and measuring the bounds are both full sweeps of
        // an array that can run to gigabytes, so they share one pass.
        let bounds = shift_and_measure(&mut positions, offset);
        if positions.is_empty() {
            self.bundle.warnings.push(format!("Skipped empty OMF point set '{}'", element.name));
            return Ok(());
        }
        let colors = element
            .attributes
            .iter()
            .find_map(|attribute| match (&attribute.location, &attribute.data) {
                (omf_crate::Location::Vertices, omf_crate::AttributeData::Color { values }) => Some(values),
                _ => None,
            })
            .map(|values| {
                ensure_items(values.item_count(), "colours")?;
                anyhow::Ok(
                    self.reader
                        .array_colors_vec(values)?
                        .into_iter()
                        .map(|color| u32::from_le_bytes(color.unwrap_or([255; 4])))
                        .collect::<Vec<_>>(),
                )
            })
            .transpose()?;
        let classifications = self.read_point_classification(element, positions.len())?;
        if classifications.is_none() && element.attributes.iter().any(|attribute| attribute.name == POINT_CLASSIFICATION_ATTRIBUTE) {
            self.bundle.warnings.push(format!(
                "Element '{}' has a classification attribute that could not be decoded and will be omitted",
                element.name
            ));
        }
        let bounds = bounds.with_context(|| format!("OMF point set '{}' contains no finite points", element.name))?;
        let prepared = prepare_for_render(&positions, colors.as_deref(), classifications.as_deref(), bounds);
        let style = element.metadata.get(META_STYLE);
        let section = self.element_section(element, MemberKind::PointCloud);
        let folder = self.element_folder(element, section);
        let payload_source = self.payload_locator();
        self.bundle.point_clouds.push(ImportedPointCloud {
            preferred_id: element_id(element),
            source_name: element_source_name(element),
            source_format: element_source_format(element),
            loaded: LoadedPointCloud {
                name: element_name(element).to_owned(),
                path: virtual_path(self.source_name, element_name(element), "pcd"),
                points: Arc::new(positions),
                colors: colors.map(Arc::new),
                classifications: classifications.map(Arc::new),
                prepared: Arc::new(prepared),
                bounds,
            },
            deferred: None,
            payload_source,
            is_loaded: style_loaded(style),
            color: style_value(style, "color").unwrap_or_else(|| element_color(element, [0.85, 0.87, 0.9, 1.0])),
            point_size: style_f32(style, "point_size").unwrap_or(0.1),
            folder,
            section,
        });
        Ok(())
    }

    /// Recover the ASPRS classification column written by
    /// [`write_point_classification`].
    ///
    /// Anything that does not line up - a foreign file's own `Classification`
    /// attribute, a missing code sidecar, a length that disagrees with the
    /// point count - yields `None` rather than a guess: a wrong classification
    /// silently changes which points a bare-earth surface is built from.
    fn read_point_classification(&self, element: &omf_crate::Element, point_count: usize) -> Result<Option<Vec<u8>>> {
        let Some((values, names, attributes)) = element.attributes.iter().find_map(|attribute| match (&attribute.location, &attribute.data) {
            (omf_crate::Location::Vertices, omf_crate::AttributeData::Category { values, names, attributes, .. }) if attribute.name == POINT_CLASSIFICATION_ATTRIBUTE => {
                Some((values, names, attributes))
            }
            _ => None,
        }) else {
            return Ok(None);
        };
        let category_count = collect_results(self.reader.array_names(names)?)?.len();
        let codes = attributes
            .iter()
            .find(|attribute| attribute.name == "Incline category code" && attribute.location == omf_crate::Location::Categories)
            .and_then(|attribute| match &attribute.data {
                omf_crate::AttributeData::Number { values, .. } => Some(values),
                _ => None,
            })
            .map(|values| read_numbers(self.reader, values))
            .transpose()?
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| {
                        value
                            .filter(|value| value.is_finite() && value.fract() == 0.0 && (0.0..=255.0).contains(value))
                            .map(|value| value as u8)
                    })
                    .collect::<Option<Vec<u8>>>()
            });
        let Some(Some(codes)) = codes else {
            return Ok(None);
        };
        if codes.len() != category_count {
            return Ok(None);
        }
        ensure_items(values.item_count(), "classifications")?;
        let values = self.reader.array_indices_vec(values)?;
        if values.len() != point_count {
            return Ok(None);
        }
        Ok(Some(
            values
                .into_par_iter()
                .map(|value| {
                    value
                        .and_then(|index| codes.get(index as usize).copied())
                        .unwrap_or(crate::model::point_cloud::CLASS_UNCLASSIFIED)
                })
                .collect(),
        ))
    }

    fn read_block_model(&mut self, element: &omf_crate::Element, geometry: &omf_crate::BlockModel) -> Result<()> {
        let sizes = self.grid3_sizes(&geometry.grid)?;
        let edges = sizes.each_ref().map(|axis| cumulative_edges(axis));
        let count = geometry.grid.count().map(|count| count as usize);
        let mut blocks = Vec::new();
        let mut parent_indices = Vec::new();
        let location = match &geometry.subblocks {
            None => {
                let total = count
                    .into_iter()
                    .try_fold(1usize, |total, count| total.checked_mul(count))
                    .context("OMF block count overflows")?;
                ensure_items(total as u64, "blocks")?;
                blocks.reserve(total);
                for k in 0..count[2] {
                    for j in 0..count[1] {
                        for i in 0..count[0] {
                            blocks.push(BlockBounds {
                                lower: DVec3::new(edges[0][i], edges[1][j], edges[2][k]),
                                upper: DVec3::new(edges[0][i + 1], edges[1][j + 1], edges[2][k + 1]),
                            });
                        }
                    }
                }
                omf_crate::Location::Primitives
            }
            Some(omf_crate::Subblocks::Freeform { subblocks }) => {
                ensure_items(subblocks.item_count(), "free-form sub-blocks")?;
                for (parent, corners) in collect_results(self.reader.array_freeform_subblocks(subblocks)?)? {
                    blocks.push(subblock_bounds(parent, corners, &edges)?);
                    parent_indices.push(block_parent_index(parent, count)?);
                }
                omf_crate::Location::Subblocks
            }
            Some(omf_crate::Subblocks::Regular { count: sub_count, subblocks, .. }) => {
                ensure_items(subblocks.item_count(), "regular sub-blocks")?;
                for (parent, corners) in collect_results(self.reader.array_regular_subblocks(subblocks)?)? {
                    let fractions: [f64; 3] = std::array::from_fn(|axis| f64::from(corners[axis]) / f64::from(sub_count[axis]));
                    let max_fractions: [f64; 3] = std::array::from_fn(|axis| f64::from(corners[axis + 3]) / f64::from(sub_count[axis]));
                    blocks.push(subblock_bounds(
                        parent,
                        [fractions[0], fractions[1], fractions[2], max_fractions[0], max_fractions[1], max_fractions[2]],
                        &edges,
                    )?);
                    parent_indices.push(block_parent_index(parent, count)?);
                }
                omf_crate::Location::Subblocks
            }
        };
        if blocks.is_empty() {
            self.bundle.warnings.push(format!("Skipped empty OMF block model '{}'", element.name));
            return Ok(());
        }
        // Each variable is its own array, so they decode side by side; a
        // block model's variables are usually far more numerous than its row groups.
        let reader = self.reader;
        let columns = element
            .attributes
            .par_iter()
            .filter_map(|attribute| {
                if location == omf_crate::Location::Subblocks && attribute.location == omf_crate::Location::Primitives {
                    Some((attribute, Some(parent_indices.as_slice())))
                } else if attribute.location == location {
                    Some((attribute, None))
                } else {
                    None
                }
            })
            .map(|(attribute, expansion)| read_block_column(reader, attribute, expansion))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let upper = DVec3::new(*edges[0].last().unwrap_or(&0.0), *edges[1].last().unwrap_or(&0.0), *edges[2].last().unwrap_or(&0.0));
        let rotation = DMat3::from_cols(
            DVec3::from_array(geometry.orient.u),
            DVec3::from_array(geometry.orient.v),
            DVec3::from_array(geometry.orient.w),
        );
        let origin = self.project_origin + DVec3::from_array(geometry.orient.origin);
        let retained_geometry_bytes = blocks.len().checked_mul(std::mem::size_of::<BlockBounds>()).context("block geometry size overflows")?;
        let model = BlockModelData::from_columns(blocks.len(), DVec3::ZERO, upper, columns, retained_geometry_bytes)?.with_transform(origin, rotation)?;
        let blocks = Arc::new(BlockBoundsSource::Explicit(blocks));
        let renderable = Arc::new(RenderableBlockIndices::All(blocks.len()));
        let uniform_grid = blocks.uniform_grid();
        let opaque_surface_blocks = uniform_grid.as_ref().map_or_else(
            || opaque_irregular_surface_block_count(&blocks, &renderable),
            |grid| opaque_surface_block_count(&blocks, &renderable, grid),
        );
        let world_bounds = compute_world_bounds(&model, &blocks, &renderable);
        let style = element.metadata.get(META_STYLE);
        let active_color_variable = style_value::<Option<String>>(style, "active_color_variable")
            .flatten()
            .filter(|name| model.variable(name).is_some())
            .or_else(|| model.color_variables().into_iter().find(|variable| !variable.special).map(|variable| variable.name.clone()));
        let active_values_cache = OpenBlockModel::prepare_active_values_cache(&model, &renderable, active_color_variable.as_deref());
        let color_transfers = self.read_block_color_transfers(element, &model, &renderable, style, active_color_variable.as_deref());
        let section = self.element_section(element, MemberKind::BlockModel);
        let folder = self.element_folder(element, section);
        self.bundle.block_models.push(ImportedBlockModel {
            preferred_id: element_id(element),
            source_name: element_source_name(element),
            source_format: element_source_format(element),
            loaded: LoadedBlockModel {
                name: element_name(element).to_owned(),
                source: crate::model::block_model::BlockModelSource {
                    path: virtual_path(self.source_name, element_name(element), "csv"),
                    csv_columns: None,
                },
                model,
                blocks,
                renderable_block_indices: renderable,
                uniform_grid,
                opaque_surface_blocks,
                world_bounds,
                active_color_variable,
                active_values_cache,
            },
            deferred: None,
            is_loaded: style_loaded(style),
            color: style_value(style, "color").unwrap_or_else(|| element_color(element, [0.72, 0.72, 0.75, 1.0])),
            slice: style_value(style, "slice"),
            color_transfers,
            hide_empty_color_values: style_bool(style, "hide_empty_color_values").unwrap_or(true),
            folder,
            section,
        });
        Ok(())
    }

    /// Colormaps for this element's variables, taken from the OMF attributes
    /// themselves.
    ///
    /// The attribute is the authority: a number attribute's `colormap` and a
    /// category attribute's `gradient` are where a colormap lives, so that is
    /// where Incline Design reads it from and writes it back to. `incline:style` is
    /// consulted only to migrate projects written before that was true, and is
    /// never written any more - one representation, so nothing can drift.
    ///
    /// A colormap that fails to decode is skipped with a warning rather than
    /// failing the import: colours are not worth losing a model over.
    fn read_block_color_transfers(
        &self,
        element: &omf_crate::Element,
        model: &BlockModelData,
        renderable: &RenderableBlockIndices,
        style: Option<&Value>,
        active_color_variable: Option<&str>,
    ) -> BTreeMap<String, ColorTransferFunction> {
        let variable_range = |name: &str| -> Option<(f64, f64)> {
            let variable = model.variable(name)?;
            let default = crate::model::block_model::color_variable_default(variable);
            let values = model.color_values(name).ok()?;
            crate::model::block_model::render_value_range(&values, renderable, default)
        };

        let mut transfers = BTreeMap::new();
        for attribute in &element.attributes {
            let Some(variable) = model.variable(&attribute.name) else {
                continue;
            };
            match &attribute.data {
                omf_crate::AttributeData::Number { colormap: Some(colormap), .. } => match read_number_colormap(self.reader, colormap) {
                    Ok(transfer) => {
                        transfers.insert(attribute.name.clone(), transfer);
                    }
                    Err(error) => crate::userspace_warn!(
                        "{}",
                        crate::i18n::tr_format!(
                            literal = "Ignoring the colour map on OMF attribute '%attribute%': %error%",
                            attribute = &attribute.name,
                            error = format!("{error:#}")
                        )
                    ),
                },
                // Category colours arrive on the column as `category_colors`
                // (see `read_block_column`) and are already merged into the
                // variable, so the colormap is built from there.
                omf_crate::AttributeData::Category { gradient: Some(_), .. } => {
                    transfers.insert(attribute.name.clone(), crate::model::block_model::categorical_color_transfer(variable));
                }
                _ => {}
            }
        }

        // Projects written before colormaps lived in the attribute.
        if let Some(stored) = style_value::<BTreeMap<String, StoredColorTransferFunction>>(style, "color_transfers") {
            for (name, transfer) in stored {
                let range = variable_range(&name);
                transfers.insert(name, transfer.resolve(range));
            }
        } else if let Some(stored) = style_value::<StoredColorTransferFunction>(style, "color_transfer") {
            // Older still: one ramp per model, belonging to whichever variable
            // was active.
            if let Some(name) = active_color_variable {
                let range = variable_range(name);
                transfers.insert(name.to_owned(), stored.resolve(range));
            }
        }
        for (name, transfer) in &mut transfers {
            transfer.sanitise(variable_range(name));
        }
        transfers
    }

    /// Rebuild a dataset written by [`write_drill_holes`].
    fn read_drill_dataset(&mut self, element: &omf_crate::Element) -> Result<Option<ImportedDrillHoles>> {
        use crate::model::drill_hole::{DrillInterval, TraceStation};

        let omf_crate::Geometry::Composite(composite) = &element.geometry else {
            return Ok(None);
        };
        let part = |part_kind: &str| composite.elements.iter().find(|child| kind(child) == Some(part_kind));
        let (Some(collars), Some(traces), Some(intervals)) = (part("drillhole_collars"), part("drillhole_traces"), part("drillhole_intervals")) else {
            bail!("Drillhole dataset '{}' is missing its collars, traces or intervals", element.name);
        };
        let context = |part: &str| format!("read {part} of drillhole dataset '{}'", element.name);

        let omf_crate::Geometry::PointSet(collar_points) = &collars.geometry else {
            bail!("Drillhole collars of '{}' are not a point set", element.name);
        };
        let offset = self.project_origin + DVec3::from_array(collar_points.origin);
        let positions = self.read_vertices(&collar_points.vertices).with_context(|| context("collars"))?;
        let (collar_holes, names) = self.read_drill_hole_category(collars).with_context(|| context("collars"))?;
        let diameters = self.read_drill_numbers(collars, "Diameter")?.unwrap_or_default();
        let mut holes = positions
            .iter()
            .enumerate()
            .map(|(index, position)| DrillHole {
                dhid: collar_holes
                    .get(index)
                    .copied()
                    .flatten()
                    .and_then(|hole| names.get(hole as usize).cloned())
                    .unwrap_or_else(|| format!("Hole {}", index + 1)),
                collar: *position + offset,
                diameter: diameters.get(index).copied().flatten(),
                trace: Vec::new(),
                render_ranges: Vec::new(),
                intervals: Vec::new(),
            })
            .collect::<Vec<_>>();

        let omf_crate::Geometry::LineSet(trace_lines) = &traces.geometry else {
            bail!("Drillhole traces of '{}' are not a line set", element.name);
        };
        let offset = self.project_origin + DVec3::from_array(trace_lines.origin);
        let positions = self.read_vertices(&trace_lines.vertices).with_context(|| context("traces"))?;
        let (station_holes, _) = self.read_drill_hole_category(traces).with_context(|| context("traces"))?;
        let depths = self.read_drill_numbers(traces, "Measured depth")?.context("drillhole traces have no measured depths")?;
        if station_holes.len() != positions.len() || depths.len() != positions.len() {
            bail!("Drillhole traces of '{}' have mismatched column lengths", element.name);
        }
        for ((position, hole), depth) in positions.into_iter().zip(station_holes).zip(depths) {
            if let (Some(hole), Some(depth)) = (hole.and_then(|hole| holes.get_mut(hole as usize)), depth) {
                hole.trace.push(TraceStation {
                    depth,
                    position: position + offset,
                });
            }
        }

        let (interval_holes, _) = self.read_drill_hole_category(intervals).with_context(|| context("intervals"))?;
        let from = self.read_drill_numbers(intervals, "From")?.context("drillhole intervals have no From depths")?;
        let to = self.read_drill_numbers(intervals, "To")?.context("drillhole intervals have no To depths")?;
        if from.len() != interval_holes.len() || to.len() != interval_holes.len() {
            bail!("Drillhole intervals of '{}' have mismatched column lengths", element.name);
        }
        let mut rows = from
            .into_iter()
            .zip(to)
            .map(|(from, to)| DrillInterval {
                from: from.unwrap_or(f64::NAN),
                to: to.unwrap_or(f64::NAN),
                values: BTreeMap::new(),
            })
            .collect::<Vec<_>>();
        for attribute in intervals
            .attributes
            .iter()
            .filter(|attribute| attribute.location == omf_crate::Location::Primitives && !matches!(attribute.name.as_str(), DRILL_HOLE_ATTRIBUTE | "From" | "To"))
        {
            let values: Vec<Option<DrillValue>> = match &attribute.data {
                omf_crate::AttributeData::Number { values, .. } => read_numbers(self.reader, values)?.into_iter().map(|value| value.map(DrillValue::Numeric)).collect(),
                omf_crate::AttributeData::Category { values, names, .. } => {
                    let names = collect_results(self.reader.array_names(names)?)?;
                    collect_results(self.reader.array_indices(values)?)?
                        .into_iter()
                        .map(|index| index.and_then(|index| names.get(index as usize)).map(|name| DrillValue::Category(name.clone())))
                        .collect()
                }
                _ => continue,
            };
            if values.len() != rows.len() {
                bail!("Drillhole interval field '{}' of '{}' has the wrong length", attribute.name, element.name);
            }
            for (row, value) in rows.iter_mut().zip(values) {
                if let Some(value) = value {
                    row.values.insert(attribute.name.clone(), value);
                }
            }
        }
        for (row, hole) in rows.into_iter().zip(interval_holes) {
            if let Some(hole) = hole.and_then(|hole| holes.get_mut(hole as usize)) {
                hole.intervals.push(row);
            }
        }

        if let Some(ranges) = element.metadata.get(META_RENDER_RANGES).and_then(Value::as_object) {
            for (index, ranges) in ranges {
                if let (Some(hole), Ok(ranges)) = (index.parse::<usize>().ok().and_then(|index| holes.get_mut(index)), Vec::<(f64, f64)>::deserialize(ranges)) {
                    hole.render_ranges = ranges;
                }
            }
        }
        if holes.is_empty() {
            return Ok(None);
        }
        let mut dataset = DrillHoleDataset::new(holes);
        // Resolved after construction: it is `new` that fixes the hole order
        // the stored names are looked up against.
        if let Some(value) = element.metadata.get(META_TIE_INS)
            && let Ok(stored) = crate::model::drill_hole::StoredTieIns::deserialize(value)
        {
            let dropped = dataset.apply_stored_ties(stored);
            if dropped > 0 {
                self.bundle.warnings.push(tr_format!(
                    literal = "Element '%name%' has %count% tie-in(s) naming holes it no longer contains",
                    name = &element.name,
                    count = dropped
                ));
            }
        }
        let dataset = Arc::new(dataset);
        let path = virtual_path(self.source_name, element_name(element), "omf");
        let section = self.element_section(element, MemberKind::DrillHole);
        let folder = self.element_folder(element, section);
        Ok(Some(ImportedDrillHoles {
            preferred_id: element_id(element),
            source_name: element_source_name(element),
            source_format: element_source_format(element),
            loaded: LoadedDrillHoleDataset {
                name: element_name(element).to_owned(),
                source: DrillHoleSource::Omf {
                    name: element_name(element).to_owned(),
                    path,
                },
                dataset,
            },
            deferred: None,
            is_loaded: style_loaded(element.metadata.get(META_STYLE)),
            color: style_value(element.metadata.get(META_STYLE), "color").unwrap_or_default(),
            folder,
            section,
        }))
    }

    /// Each row's hole, as an index into the returned hole names.
    fn read_drill_hole_category(&self, element: &omf_crate::Element) -> Result<(Vec<Option<u32>>, Vec<String>)> {
        let Some(omf_crate::AttributeData::Category { values, names, .. }) = element
            .attributes
            .iter()
            .find(|attribute| attribute.name == DRILL_HOLE_ATTRIBUTE)
            .map(|attribute| &attribute.data)
        else {
            bail!("missing the '{DRILL_HOLE_ATTRIBUTE}' category");
        };
        Ok((collect_results(self.reader.array_indices(values)?)?, collect_results(self.reader.array_names(names)?)?))
    }

    fn read_drill_numbers(&self, element: &omf_crate::Element, name: &str) -> Result<Option<Vec<Option<f64>>>> {
        element
            .attributes
            .iter()
            .find(|attribute| attribute.name == name)
            .and_then(|attribute| match &attribute.data {
                omf_crate::AttributeData::Number { values, .. } => Some(values),
                _ => None,
            })
            .map(|values| read_numbers(self.reader, values))
            .transpose()
    }

    fn read_textures(&mut self, element: &omf_crate::Element) -> Result<()> {
        // One element can carry several textures (one push per attribute
        // below); all share the carrying element's section and folder.
        let section = self.element_section(element, MemberKind::Raster);
        let folder = self.element_folder(element, section);
        for attribute in &element.attributes {
            let (image, derived_world_to_uv) = match &attribute.data {
                omf_crate::AttributeData::ProjectedTexture { image, orient, width, height } => {
                    let origin = self.project_origin + DVec3::from_array(orient.origin);
                    let u = DVec3::from_array(orient.u);
                    let v = DVec3::from_array(orient.v);
                    (
                        image,
                        Some([u.x / *width, u.y / *width, -u.dot(origin) / *width, v.x / *height, v.y / *height, -v.dot(origin) / *height]),
                    )
                }
                omf_crate::AttributeData::MappedTexture { image, texcoords } => {
                    let omf_crate::Geometry::Surface(surface) = &element.geometry else {
                        self.bundle.warnings.push(format!(
                            "Skipped mapped texture '{}' on '{}': only surface textures can become Incline Design rasters",
                            attribute.name, element.name
                        ));
                        continue;
                    };
                    let offset = self.project_origin + DVec3::from_array(surface.origin);
                    let vertices = self.read_vertices(&surface.vertices)?.into_iter().map(|point| point + offset).collect::<Vec<_>>();
                    ensure_items(texcoords.item_count(), "texture coordinates")?;
                    let texcoords = collect_results(self.reader.array_texcoords(texcoords)?)?;
                    (image, affine_from_texcoords(&vertices, &texcoords))
                }
                _ => continue,
            };
            let decoded = self.reader.image(image).with_context(|| format!("decode texture '{}'", attribute.name))?.into_rgba8();
            let size = [decoded.width(), decoded.height()];
            let style = element.metadata.get(META_STYLE);
            let world_to_uv = style
                .and_then(|style| style.get("world_to_uv"))
                .and_then(|value| <[f64; 6]>::deserialize(value).ok())
                .map(|[a, b, c, d, e, f]| {
                    [
                        a,
                        b,
                        c - a * self.project_origin.x - b * self.project_origin.y,
                        d,
                        e,
                        f - d * self.project_origin.x - e * self.project_origin.y,
                    ]
                })
                .or(derived_world_to_uv);
            let Some(world_to_uv) = world_to_uv else {
                self.bundle.warnings.push(format!(
                    "Skipped texture '{}' on '{}': its UV mapping is not affine in world XY",
                    attribute.name, element.name
                ));
                continue;
            };
            let declared_source_size = style
                .and_then(|style| style.get("source_size"))
                .and_then(|value| <[u32; 2]>::deserialize(value).ok())
                .unwrap_or(size);
            if declared_source_size != size {
                self.bundle.warnings.push(format!(
                    "Raster '{}' declared source size {} × {}, but its embedded full-resolution image is {} × {}; the image dimensions were used",
                    element_name(element),
                    declared_source_size[0],
                    declared_source_size[1],
                    size[0],
                    size[1]
                ));
            }
            let preview_size = style
                .and_then(|style| style.get("preview_size"))
                .and_then(|value| <[u32; 2]>::deserialize(value).ok())
                .filter(|[width, height]| *width > 0 && *height > 0 && *width <= size[0] && *height <= size[1])
                .unwrap_or(size);
            let projection = style
                .and_then(|style| style.get("projection"))
                .and_then(Value::as_str)
                .unwrap_or(&self.project_crs)
                .to_owned();
            let full_rgba = Arc::new(decoded.into_raw());
            let rgba = if preview_size == size {
                Arc::clone(&full_rgba)
            } else {
                Arc::new(crate::model::raster::downscale_rgba(&full_rgba, size, preview_size)?)
            };
            // Only an element this app wrote holds exactly `full_rgba` in the
            // layout a save would give it; see `copy_raster_image`.
            let payload_source = if kind(element) == Some("raster") { self.payload_locator() } else { None };
            self.bundle.rasters.push(ImportedRaster {
                preferred_id: element_id(element),
                source_name: element_source_name(element),
                source_format: element_source_format(element),
                deferred: None,
                payload_source,
                is_loaded: style_loaded(style),
                loaded: LoadedRasterTexture {
                    name: if attribute.name == "Raster" {
                        element_name(element).to_owned()
                    } else {
                        format!("{} – {}", element_name(element), attribute.name)
                    },
                    path: virtual_path(self.source_name, element_name(element), "tif"),
                    source_size: size,
                    preview_size,
                    full_rgba,
                    rgba,
                    world_to_uv,
                    projection,
                    driver_name: tr!(literal = "OMF texture"),
                },
                folder,
                section,
            });
        }
        Ok(())
    }

    /// Vertex arrays are the bulk of an OMF file, so they go through the
    /// reader's bulk path: whole Parquet column blocks instead of one
    /// `Option<Result<f64>>` per coordinate, with row groups decoded in
    /// parallel. `[f64; 3]` and `DVec3` share a layout, so the cast below
    /// reuses the allocation rather than copying it.
    fn read_vertices(&self, array: &omf_crate::Array<omf_crate::array_type::Vertex>) -> Result<Vec<DVec3>> {
        ensure_items(array.item_count(), "vertices")?;
        Ok(self.reader.array_vertices_vec(array)?.into_iter().map(DVec3::from_array).collect())
    }

    fn grid2_sizes(&self, grid: &omf_crate::Grid2) -> Result<[Vec<f64>; 2]> {
        match grid {
            omf_crate::Grid2::Regular { size, count } => Ok([vec![size[0]; count[0] as usize], vec![size[1]; count[1] as usize]]),
            omf_crate::Grid2::Tensor { u, v } => Ok([collect_results(self.reader.array_scalars(u)?)?, collect_results(self.reader.array_scalars(v)?)?]),
        }
    }

    fn grid3_sizes(&self, grid: &omf_crate::Grid3) -> Result<[Vec<f64>; 3]> {
        match grid {
            omf_crate::Grid3::Regular { size, count } => Ok([vec![size[0]; count[0] as usize], vec![size[1]; count[1] as usize], vec![size[2]; count[2] as usize]]),
            omf_crate::Grid3::Tensor { u, v, w } => Ok([
                collect_results(self.reader.array_scalars(u)?)?,
                collect_results(self.reader.array_scalars(v)?)?,
                collect_results(self.reader.array_scalars(w)?)?,
            ]),
        }
    }
}

fn affine_from_texcoords(vertices: &[DVec3], texcoords: &[[f64; 2]]) -> Option<[f64; 6]> {
    let count = vertices.len().min(texcoords.len());
    for first in 0..count {
        for second in first + 1..count {
            for third in second + 1..count {
                let p0 = vertices[first];
                let p1 = vertices[second];
                let p2 = vertices[third];
                let t0 = texcoords[first];
                let t1 = texcoords[second];
                let t2 = texcoords[third];
                let dx1 = p1.x - p0.x;
                let dy1 = p1.y - p0.y;
                let dx2 = p2.x - p0.x;
                let dy2 = p2.y - p0.y;
                let determinant = dx1 * dy2 - dx2 * dy1;
                if !determinant.is_finite() || determinant.abs() <= f64::EPSILON {
                    continue;
                }
                let solve = |v0: f64, v1: f64, v2: f64| {
                    let dv1 = v1 - v0;
                    let dv2 = v2 - v0;
                    let x = (dv1 * dy2 - dv2 * dy1) / determinant;
                    let y = (-dv1 * dx2 + dv2 * dx1) / determinant;
                    [x, y, v0 - x * p0.x - y * p0.y]
                };
                let [a, b, c] = solve(t0[0], t1[0], t2[0]);
                let [d, e, f] = solve(t0[1], t1[1], t2[1]);
                let transform = [a, b, c, d, e, f];
                if transform.iter().all(|value| value.is_finite()) {
                    return Some(transform);
                }
            }
        }
    }
    None
}

fn read_block_column<R: omf_crate::file::ReadAt>(
    reader: &omf_crate::file::Reader<R>,
    attribute: &omf_crate::Attribute,
    expansion: Option<&[usize]>,
) -> Result<Option<BlockModelColumn>> {
    match &attribute.data {
        omf_crate::AttributeData::Number { values, .. } => {
            let values = read_numbers(reader, values)?.into_iter().map(|value| value.unwrap_or(f64::NAN)).collect::<Vec<_>>();
            let values = expand_block_values(values, expansion)?;
            Ok(Some(BlockModelColumn {
                name: attribute.name.clone(),
                values: Arc::new(values),
                categories: None,
                category_colors: BTreeMap::new(),
            }))
        }
        omf_crate::AttributeData::Category {
            values,
            names,
            gradient,
            attributes,
        } => {
            let names = collect_results(reader.array_names(names)?)?;
            let original_codes = attributes
                .iter()
                .find(|attribute| attribute.name == "Incline category code" && attribute.location == omf_crate::Location::Categories)
                .and_then(|attribute| match &attribute.data {
                    omf_crate::AttributeData::Number { values, .. } => Some(values),
                    _ => None,
                })
                .map(|values| read_numbers(reader, values))
                .transpose()?
                .and_then(|values| {
                    (values.len() == names.len())
                        .then(|| {
                            values
                                .into_iter()
                                .map(|value| {
                                    value
                                        .filter(|value| value.is_finite() && value.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(value))
                                        .map(|value| value as u32)
                                })
                                .collect::<Option<Vec<_>>>()
                        })
                        .flatten()
                });
            let codes = original_codes.unwrap_or_else(|| (0..names.len() as u32).collect());
            // The file's own palette, when it ships one, beats Incline Design's
            // generic categorical colours.
            let category_colors = gradient
                .as_ref()
                .map(|gradient| collect_results(reader.array_gradient(gradient)?))
                .transpose()?
                .map(|colors| codes.iter().copied().zip(colors).map(|(code, color)| (code, linear_rgba(color))).collect())
                .unwrap_or_default();
            let categories = names.into_iter().zip(codes.iter().copied()).map(|(name, code)| (code, name)).collect();
            let values = collect_results(reader.array_indices(values)?)?
                .into_iter()
                .map(|value| value.and_then(|index| codes.get(index as usize).copied()).map_or(f64::NAN, f64::from))
                .collect::<Vec<_>>();
            let values = expand_block_values(values, expansion)?;
            Ok(Some(BlockModelColumn {
                name: attribute.name.clone(),
                values: Arc::new(values),
                categories: Some(categories),
                category_colors,
            }))
        }
        _ => Ok(None),
    }
}

fn ensure_items(count: u64, description: &str) -> Result<usize> {
    if count > MAX_ARRAY_ITEMS {
        bail!("OMF {description} count {count} exceeds Incline Design's import limit of {MAX_ARRAY_ITEMS}");
    }
    usize::try_from(count).with_context(|| format!("OMF {description} count exceeds addressable memory"))
}

/// Shift every point by `offset` in place and return the bounds of the finite
/// ones, in a single parallel pass.
fn shift_and_measure(positions: &mut [DVec3], offset: DVec3) -> Option<(DVec3, DVec3)> {
    let (min, max) = positions
        .par_iter_mut()
        .map(|point| {
            *point += offset;
            *point
        })
        .filter(|point| point.is_finite())
        .fold(
            || (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(min, max), point| (min.min(point), max.max(point)),
        )
        .reduce(
            || (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
            |(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)),
        );
    (min.is_finite() && max.is_finite()).then_some((min, max))
}

fn collect_results<T>(items: impl IntoIterator<Item = std::result::Result<T, omf_crate::error::Error>>) -> Result<Vec<T>> {
    items.into_iter().collect::<std::result::Result<Vec<_>, _>>().map_err(anyhow::Error::new)
}

fn read_numbers<R: omf_crate::file::ReadAt>(reader: &omf_crate::file::Reader<R>, array: &omf_crate::Array<omf_crate::array_type::Number>) -> Result<Vec<Option<f64>>> {
    ensure_items(array.item_count(), "number values")?;
    let numbers = reader.array_numbers(array)?;
    if let Ok(numbers) = numbers.try_into_f64() {
        return collect_results(numbers);
    }
    let integers = reader.array_numbers(array)?.try_into_i64()?;
    Ok(collect_results(integers)?.into_iter().map(|value| value.map(|value| value as f64)).collect())
}

fn cumulative_edges(sizes: &[f64]) -> Vec<f64> {
    let mut edges = Vec::with_capacity(sizes.len() + 1);
    edges.push(0.0);
    for size in sizes {
        edges.push(edges.last().copied().unwrap_or(0.0) + size);
    }
    edges
}

fn block_parent_index(parent: [u32; 3], count: [usize; 3]) -> Result<usize> {
    let [i, j, k] = parent.map(|value| value as usize);
    if i >= count[0] || j >= count[1] || k >= count[2] {
        bail!("OMF sub-block parent {parent:?} is outside grid {count:?}");
    }
    k.checked_mul(count[1])
        .and_then(|value| value.checked_add(j))
        .and_then(|value| value.checked_mul(count[0]))
        .and_then(|value| value.checked_add(i))
        .context("OMF block parent index overflows")
}

fn expand_block_values(values: Vec<f64>, expansion: Option<&[usize]>) -> Result<Vec<f64>> {
    let Some(expansion) = expansion else {
        return Ok(values);
    };
    expansion
        .iter()
        .map(|&parent| values.get(parent).copied().with_context(|| format!("OMF parent-block attribute is missing value {parent}")))
        .collect()
}

fn subblock_bounds(parent: [u32; 3], corners: [f64; 6], edges: &[Vec<f64>; 3]) -> Result<BlockBounds> {
    let mut lower = [0.0; 3];
    let mut upper = [0.0; 3];
    for axis in 0..3 {
        let index = parent[axis] as usize;
        let parent_lower = *edges[axis].get(index).context("OMF sub-block parent is out of range")?;
        let parent_upper = *edges[axis].get(index + 1).context("OMF sub-block parent is out of range")?;
        let size = parent_upper - parent_lower;
        lower[axis] = parent_lower + size * corners[axis];
        upper[axis] = parent_lower + size * corners[axis + 3];
    }
    Ok(BlockBounds {
        lower: DVec3::from_array(lower),
        upper: DVec3::from_array(upper),
    })
}

fn line_strings(vertices: &[DVec3], segments: &[[u32; 2]]) -> Vec<(Vec<DVec3>, bool)> {
    if vertices.len() >= 2 {
        let open = segments.len() == vertices.len() - 1 && segments.iter().enumerate().all(|(index, segment)| *segment == [index as u32, index as u32 + 1]);
        let closed = segments.len() == vertices.len()
            && segments[..segments.len() - 1]
                .iter()
                .enumerate()
                .all(|(index, segment)| *segment == [index as u32, index as u32 + 1])
            && segments.last() == Some(&[(vertices.len() - 1) as u32, 0]);
        if open || closed {
            return vec![(vertices.to_vec(), closed)];
        }
    }
    segments
        .iter()
        .filter_map(|[a, b]| Some((vec![*vertices.get(*a as usize)?, *vertices.get(*b as usize)?], false)))
        .collect()
}

fn style_bool(style: Option<&Value>, key: &str) -> Option<bool> {
    style?.get(key)?.as_bool()
}

fn style_f32(style: Option<&Value>, key: &str) -> Option<f32> {
    style?.get(key)?.as_f64().map(|value| value as f32)
}

fn style_value<T: serde::de::DeserializeOwned>(style: Option<&Value>, key: &str) -> Option<T> {
    T::deserialize(style?.get(key)?).ok()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| tr!(literal = "OMF import"))
}

fn safe_component(name: &str) -> String {
    let component = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let component = component.trim().trim_matches('.');
    if component.is_empty() { "element".to_owned() } else { component.to_owned() }
}

fn virtual_path(source: &str, element: &str, extension: &str) -> PathBuf {
    PathBuf::from("omf")
        .join(safe_component(&file_stem(source)))
        .join(format!("{}.{}", safe_component(element), extension))
}

/// Older Incline OMF files stored this choice as visibility.
fn style_loaded(style: Option<&Value>) -> bool {
    style_bool(style, "loaded").or_else(|| style_bool(style, "visible")).unwrap_or(true)
}
