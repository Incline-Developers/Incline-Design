//! Large synthetic OMF round-trip for profiling. Not part of the normal suite:
//!
//! ```bash
//! cargo test --profile profiling omf_round_trip_benchmark -- --ignored --nocapture
//! OMF_BENCH_PHASES=open cargo flamegraph --profile profiling --unit-test incline -- omf_round_trip_benchmark --ignored
//! ```
//!
//! `OMF_BENCH_PHASES` is a comma list of `write`, `open`, `materialize`,
//! `resave`, `evict` (default: all). `OMF_BENCH_SCALE` multiplies the item sizes.

use std::{collections::BTreeMap, sync::Arc, time::Instant};

use glam::DVec3;

use super::*;
use crate::model::{
    ObjectId, OpenItem,
    block_model::{BlockModelId, OpenBlockModel},
    drill_hole::{DrillColorState, DrillField, DrillFieldKind, DrillHoleId, DrillInterval, DrillValue, TraceStation},
    point_cloud::PointCloudId,
    progress::Progress,
    project::ProjectItemState,
    raster::RasterTextureId,
    triangulation::TriangulationId,
};

/// Deterministic xorshift so every run profiles the same bytes.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn terrain(x: f64, y: f64) -> f64 {
    120.0 + 30.0 * (x * 0.004).sin() * (y * 0.003).cos() + 8.0 * (x * 0.03 + y * 0.02).sin()
}

const ORIGIN: DVec3 = DVec3::new(512_000.0, 7_380_000.0, 0.0);

fn design(rng: &mut Rng, scale: usize) -> ProjectFile {
    let mut design = project::new_empty(None);
    let document = &mut design.document;
    for layer_index in 0..40 {
        let layer = document.add_layer(format!("Layer {layer_index}"), None, [0.2, 0.5, 0.8, 1.0], true, 0.0);
        for object_index in 0..150 * scale {
            let base = ORIGIN + DVec3::new(rng.unit() * 4000.0, rng.unit() * 4000.0, rng.unit() * 200.0);
            let color = if object_index % 3 == 0 {
                ObjectColor::Fixed([1.0, 0.2, 0.1, 1.0])
            } else {
                ObjectColor::ByLayer
            };
            document.add_object(|id: ObjectId| match object_index % 8 {
                0 => Object::Point { id, layer, pos: base, color },
                1 => Object::Circle {
                    id,
                    layer,
                    center: base,
                    radius: 5.0 + rng.unit() * 50.0,
                    color,
                    fill: FillStyle::Clear,
                    line_weight: 1.0,
                },
                2 => Object::Text {
                    id,
                    layer,
                    pos: base,
                    content: format!("RL {:.1}", base.z),
                    height: 2.5,
                    rotation: 0.0,
                    color,
                },
                _ => {
                    let count = 20 + (rng.next() % 300) as usize;
                    let verts = (0..count)
                        .map(|step| {
                            let angle = step as f64 * 0.05;
                            PolyVertex {
                                pos: base + DVec3::new(angle.cos() * step as f64, angle.sin() * step as f64, step as f64 * 0.1),
                                bulge: 0.0,
                            }
                        })
                        .collect();
                    Object::Polyline {
                        id,
                        layer,
                        verts,
                        closed: object_index % 2 == 0,
                        color,
                        fill: FillStyle::Clear,
                        line_weight: 1.5,
                    }
                }
            });
        }
    }
    design
}

fn triangulation(id: u64, side: usize, loaded: bool) -> OpenTriangulation {
    let spacing = 4.0;
    let mut vertices = Vec::with_capacity(side * side);
    for j in 0..side {
        for i in 0..side {
            let (x, y) = (i as f64 * spacing, j as f64 * spacing);
            let p = ORIGIN + DVec3::new(x, y, terrain(x, y) + id as f64 * 25.0);
            vertices.push(Vertex { x: p.x, y: p.y, z: p.z });
        }
    }
    let mut faces = Vec::with_capacity((side - 1) * (side - 1) * 2);
    for j in 0..side - 1 {
        for i in 0..side - 1 {
            let a = (j * side + i) as u32;
            let b = a + 1;
            let c = a + side as u32;
            faces.push([a, b, c + 1]);
            faces.push([a, c + 1, c]);
        }
    }
    let mesh = Triangulation::from_vertices_and_faces(vertices, faces).unwrap();
    OpenTriangulation {
        id: TriangulationId(id),
        state: ProjectItemState::dirty(MemberKind::Triangulation, Some(format!("surface_{id}.dxf"))).with_loaded(loaded),
        name: format!("Surface {id}"),
        spatial: Arc::new(crate::model::spatial::TriangleBvh::build(&mesh)),
        edges: unique_edges(&mesh),
        surface_face_order: Arc::new(spatial_surface_face_order(&mesh)),
        mesh: Arc::new(mesh),
        color: [0.6, 0.6, 0.6, 1.0],
        line_color: [0.1, 0.1, 0.1, 1.0],
        line_weight: Some(1.0),
        raster_texture: None,
        raster_opacity: 1.0,
    }
}

fn point_cloud(rng: &mut Rng, id: u64, count: usize, loaded: bool) -> OpenPointCloud {
    let points = (0..count)
        .map(|_| {
            let (x, y) = (rng.unit() * 3000.0, rng.unit() * 3000.0);
            ORIGIN + DVec3::new(x, y, terrain(x, y) + rng.unit() * 0.3)
        })
        .collect::<Vec<_>>();
    let colors = points.iter().map(|p| u32::from_le_bytes([(p.z as u32 % 256) as u8, 120, 80, 255])).collect::<Vec<_>>();
    let classifications = (0..count).map(|index| [1u8, 2, 2, 2, 3, 5, 6][index % 7]).collect::<Vec<_>>();
    let bounds = shift_and_measure(&mut points.clone(), DVec3::ZERO).unwrap();
    OpenPointCloud {
        id: PointCloudId(id),
        state: ProjectItemState::dirty(MemberKind::PointCloud, Some(format!("scan_{id}.laz"))).with_loaded(loaded),
        name: format!("Scan {id}"),
        prepared: Arc::new(prepare_for_render(&points, Some(&colors), Some(&classifications), bounds)),
        points: Arc::new(points),
        colors: Some(Arc::new(colors)),
        classifications: Some(Arc::new(classifications)),
        bounds,
        color: [0.9, 0.9, 0.9, 1.0],
        point_size: 0.1,
    }
}

fn block_model(rng: &mut Rng, id: u64, dims: [usize; 3], loaded: bool) -> OpenBlockModel {
    let cell = DVec3::new(10.0, 10.0, 5.0);
    let count = dims[0] * dims[1] * dims[2];
    let mut blocks = Vec::with_capacity(count);
    for k in 0..dims[2] {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                let lower = DVec3::new(i as f64, j as f64, k as f64) * cell;
                blocks.push(BlockBounds { lower, upper: lower + cell });
            }
        }
    }
    let numeric = |name: &str, rng: &mut Rng| BlockModelColumn {
        name: name.to_owned(),
        values: Arc::new((0..count).map(|_| rng.unit() * 5.0).collect()),
        categories: None,
        category_colors: BTreeMap::new(),
    };
    let mut columns = vec![numeric("au", rng), numeric("cu", rng), numeric("density", rng), numeric("value", rng)];
    columns.push(BlockModelColumn {
        name: "rock".to_owned(),
        values: Arc::new((0..count).map(|index| (index % 5) as f64).collect()),
        categories: Some((0..5).map(|code| (code, format!("Rock {code}"))).collect()),
        category_colors: BTreeMap::new(),
    });
    let upper = cell * DVec3::from_array(dims.map(|dim| dim as f64));
    let model = BlockModelData::from_columns(count, DVec3::ZERO, upper, columns, count * std::mem::size_of::<BlockBounds>())
        .unwrap()
        .with_transform(ORIGIN, glam::DMat3::IDENTITY)
        .unwrap();
    let blocks = Arc::new(BlockBoundsSource::Explicit(blocks));
    let renderable = Arc::new(RenderableBlockIndices::All(count));
    let uniform_grid = blocks.uniform_grid();
    let active = Some("au".to_owned());
    OpenBlockModel {
        id: BlockModelId(id),
        state: ProjectItemState::dirty(MemberKind::BlockModel, Some(format!("model_{id}.csv"))).with_loaded(loaded),
        name: format!("Block model {id}"),
        active_values_cache: OpenBlockModel::prepare_active_values_cache(&model, &renderable, active.as_deref()),
        world_bounds: block_world_bounds(&model, &blocks, &renderable),
        opaque_surface_blocks: uniform_grid.as_ref().and_then(|grid| opaque_surface_block_count(&blocks, &renderable, grid)),
        model,
        blocks,
        renderable_block_indices: renderable,
        uniform_grid,
        color: [0.7, 0.7, 0.7, 1.0],
        slice: None,
        active_color_variable: active,
        color_transfers: BTreeMap::new(),
        hide_empty_color_values: true,
    }
}

fn drill_holes(rng: &mut Rng, id: u64, holes: usize, loaded: bool) -> OpenDrillHoleDataset {
    let lithologies = ["BIF", "Shale", "Dolerite", "Granite"];
    let holes = (0..holes)
        .map(|index| {
            let collar = ORIGIN + DVec3::new(rng.unit() * 3000.0, rng.unit() * 3000.0, 150.0);
            let dip = DVec3::new(0.1, 0.05, -1.0).normalize();
            // Edge cases the layout must carry exactly: a lone collar station,
            // a hole with no intervals, missing values, and partial coverage.
            let stations = if index % 211 == 5 { 0 } else { 20 };
            let trace = (0..=stations)
                .map(|step| TraceStation {
                    depth: step as f64 * 10.0,
                    position: collar + dip * (step as f64 * 10.0),
                })
                .collect();
            let intervals = (0..if index % 97 == 3 { 0 } else { 40 })
                .map(|step| {
                    let mut values = BTreeMap::from([
                        ("au".to_owned(), DrillValue::Numeric(rng.unit() * 3.0)),
                        ("fe".to_owned(), DrillValue::Numeric(rng.unit() * 60.0)),
                        ("lith".to_owned(), DrillValue::Category(lithologies[(rng.next() % 4) as usize].to_owned())),
                    ]);
                    if step % 7 != 0 {
                        values.insert("cu".to_owned(), DrillValue::Numeric(rng.unit()));
                    }
                    DrillInterval {
                        from: step as f64 * 5.0,
                        to: step as f64 * 5.0 + 5.0,
                        values,
                    }
                })
                .collect();
            DrillHole {
                dhid: format!("DH{index:05}"),
                collar,
                diameter: (index % 13 != 0).then_some(0.14),
                trace,
                render_ranges: if index % 50 == 0 { vec![(0.0, 40.0), (60.5, 200.0)] } else { Vec::new() },
                intervals,
            }
        })
        .collect();
    let mut dataset = DrillHoleDataset::new(holes);
    let dropped = dataset.apply_stored_ties(crate::model::drill_hole::StoredTieIns {
        ties: (0..20)
            .map(|index| crate::model::drill_hole::StoredTieIn {
                from: format!("DH{index:05}"),
                to: format!("DH{:05}", index + 1),
                delay_ms: 17,
                product: "TLD 17".to_owned(),
                color: [0.2, 0.4, 0.6],
            })
            .collect(),
        initiations: vec![crate::model::drill_hole::StoredInitiation {
            hole: "DH00000".to_owned(),
            delay_ms: 0,
        }],
        initiation: None,
    });
    assert_eq!(dropped, 0);
    assert!(dataset.fields.iter().any(|field: &DrillField| matches!(field.kind, DrillFieldKind::Categorical { .. })));
    OpenDrillHoleDataset {
        id: DrillHoleId(id),
        state: ProjectItemState::dirty(MemberKind::DrillHole, Some("collars.csv".to_owned())).with_loaded(loaded),
        name: format!("Drilling {id}"),
        dataset: Arc::new(dataset),
        color: DrillColorState::default(),
    }
}

fn raster(id: u64, side: u32, loaded: bool) -> OpenRasterTexture {
    let pixels = (0..side * side)
        .flat_map(|index| {
            let (x, y) = (index % side, index / side);
            [(x ^ y) as u8, (x / 3) as u8, (y / 5) as u8, 255]
        })
        .collect::<Vec<_>>();
    let scale = 1.0 / 3000.0;
    OpenRasterTexture {
        id: RasterTextureId(id),
        state: ProjectItemState::dirty(MemberKind::Raster, Some("ortho.tif".to_owned())).with_loaded(loaded),
        name: format!("Ortho {id}"),
        source_size: [side, side],
        preview_size: [side / 4, side / 4],
        rgba: Arc::new(crate::model::raster::downscale_rgba(&pixels, [side, side], [side / 4, side / 4]).unwrap()),
        full_rgba: Arc::new(pixels),
        world_to_uv: [scale, 0.0, -ORIGIN.x * scale, 0.0, scale, -ORIGIN.y * scale],
        projection: "EPSG:28350".to_owned(),
        driver_name: "GTiff".to_owned(),
    }
}

fn snapshot(scale: usize, loaded: bool) -> ProjectSnapshot {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let side = (400.0 * (scale as f64).sqrt()) as usize;
    ProjectSnapshot {
        name: "OMF benchmark".to_owned(),
        designs: Some(design(&mut rng, scale)),
        triangulations: (1..=4).map(|id| triangulation(id, side, loaded)).collect(),
        block_models: vec![block_model(&mut rng, 1, [100, 100, 40 * scale], loaded)],
        drill_holes: vec![drill_holes(&mut rng, 1, 1500 * scale, loaded)],
        point_clouds: (1..=2).map(|id| point_cloud(&mut rng, id, 1_000_000 * scale, loaded)).collect(),
        rasters: vec![raster(1, 2048, loaded)],
        folders: FolderRegistry::default(),
    }
}

fn timed<T>(label: &str, run: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = run();
    eprintln!("{label:<34} {:>9.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    value
}

#[test]
#[ignore = "profiling harness; run explicitly"]
fn omf_round_trip_benchmark() {
    let phases = std::env::var("OMF_BENCH_PHASES").unwrap_or_else(|_| "write,open,materialize,resave,evict".to_owned());
    let enabled = |phase: &str| phases.split(',').any(|name| name.trim() == phase);
    let scale = std::env::var("OMF_BENCH_SCALE").ok().and_then(|value| value.parse().ok()).unwrap_or(1usize);
    let repeat = std::env::var("OMF_BENCH_REPEAT").ok().and_then(|value| value.parse().ok()).unwrap_or(1usize);
    let progress = Progress::new();
    let phase = progress.phase(0.0, 1.0);

    // Archives are built only when a selected phase reads them, so a profile of
    // one phase is not dominated by setup.
    let loaded = timed("build loaded snapshot", || snapshot(scale, true));
    let loaded_bytes = if enabled("open") || std::env::var("OMF_BENCH_DUMP").is_ok() {
        let bytes = timed("write loaded (archive)", || to_bytes(loaded.clone(), Compression::Archive, &phase).unwrap());
        eprintln!("archive size                       {:>9.1} MB", bytes.len() as f64 / 1e6);
        if let Ok(path) = std::env::var("OMF_BENCH_DUMP") {
            std::fs::write(path, &bytes).unwrap();
        }
        bytes
    } else {
        Vec::new()
    };
    let unloaded_bytes = if enabled("materialize") || enabled("resave") {
        to_bytes(snapshot(scale, false), Compression::Archive, &phase).unwrap()
    } else {
        Vec::new()
    };

    for _ in 0..repeat {
        if enabled("write") {
            timed("write: archive", || to_bytes(loaded.clone(), Compression::Archive, &phase).unwrap());
        }
        if enabled("open") {
            let bundle = timed("open: eager decode", || from_bytes("bench.omf", loaded_bytes.clone(), &phase).unwrap());
            assert_eq!(bundle.triangulations.len(), 4);
            assert_eq!(bundle.point_clouds.len(), 2);
            assert_eq!(bundle.block_models.len(), 1);
            assert_eq!(bundle.drill_holes.len(), 1);
            assert_eq!(bundle.rasters.len(), 1);
            assert_eq!(bundle.designs.len(), 1);
            assert_eq!(bundle.designs[0].document.objects().len(), 40 * 150 * scale);
            let points = &bundle.point_clouds[0].loaded;
            assert_eq!(points.points.len(), loaded.point_clouds[0].points.len());
            assert_eq!(points.points[12345], loaded.point_clouds[0].points[12345]);
            assert_eq!(points.colors.as_deref(), loaded.point_clouds[0].colors.as_deref());
            assert_eq!(points.classifications.as_deref(), loaded.point_clouds[0].classifications.as_deref());
            assert_eq!(bundle.triangulations[2].loaded.mesh.vertices(), loaded.triangulations[2].mesh.vertices());
            let model = &bundle.block_models[0].loaded.model;
            assert_eq!(model.metadata.n_blocks, loaded.block_models[0].model.metadata.n_blocks);
            assert_eq!(model.shared_numeric_values("cu"), loaded.block_models[0].model.shared_numeric_values("cu"));
            assert_eq!(model.shared_numeric_values("rock"), loaded.block_models[0].model.shared_numeric_values("rock"));
            let (read, written) = (&bundle.drill_holes[0].loaded.dataset, &loaded.drill_holes[0].dataset);
            assert_eq!(read.holes, written.holes);
            assert_eq!(read.fields, written.fields);
            assert_eq!(serde_json::to_value(read.stored_ties()).unwrap(), serde_json::to_value(written.stored_ties()).unwrap());
            assert_eq!(bundle.rasters[0].loaded.full_rgba, loaded.rasters[0].full_rgba);
        }
        if enabled("materialize") {
            let bundle = timed("materialize: index only", || from_bytes("bench.omf", unloaded_bytes.clone(), &phase).unwrap());
            let deferred = bundle
                .triangulations
                .iter()
                .filter_map(|item| item.deferred.as_ref())
                .chain(bundle.block_models.iter().filter_map(|item| item.deferred.as_ref()))
                .chain(bundle.drill_holes.iter().filter_map(|item| item.deferred.as_ref()))
                .chain(bundle.point_clouds.iter().filter_map(|item| item.deferred.as_ref()))
                .chain(bundle.rasters.iter().filter_map(|item| item.deferred.as_ref()))
                .map(|(asset, _)| asset.clone())
                .collect::<Vec<_>>();
            assert_eq!(deferred.len(), 9);
            timed("materialize: each deferred item", || {
                for asset in &deferred {
                    let bundle = asset.read().unwrap();
                    assert_eq!(bundle.item_count(), 1);
                }
            });
        }
        if enabled("resave") {
            let bundle = from_bytes("bench.omf", unloaded_bytes.clone(), &phase).unwrap();
            let mut snapshot = ProjectSnapshot {
                name: "resave".to_owned(),
                ..Default::default()
            };
            for (index, imported) in bundle.triangulations.into_iter().enumerate() {
                let mut item = triangulation(100 + index as u64, 2, false);
                item.state = item.state.with_deferred(imported.deferred);
                item.state.payload_source = PayloadSource::for_triangulation(imported.payload_source, &item);
                snapshot.triangulations.push(item);
            }
            for (index, imported) in bundle.point_clouds.into_iter().enumerate() {
                let mut item = point_cloud(&mut Rng(7), 100 + index as u64, 1, false);
                item.state = item.state.with_deferred(imported.deferred);
                item.state.payload_source = PayloadSource::for_point_cloud(imported.payload_source, &item);
                snapshot.point_clouds.push(item);
            }
            let bytes = timed("resave: copy unchanged payloads", || to_bytes(snapshot, Compression::Archive, &phase).unwrap());
            eprintln!("resave size                        {:>9.1} MB", bytes.len() as f64 / 1e6);
        }
        if enabled("evict") {
            timed("evict: every item (scratch)", || {
                let items = loaded
                    .triangulations
                    .iter()
                    .map(|item| OpenItem::Triangulation(Box::new(item.clone())))
                    .chain(loaded.block_models.iter().map(|item| OpenItem::BlockModel(Box::new(item.clone()))))
                    .chain(loaded.drill_holes.iter().map(|item| OpenItem::DrillHole(Box::new(item.clone()))))
                    .chain(loaded.point_clouds.iter().map(|item| OpenItem::PointCloud(Box::new(item.clone()))))
                    .chain(loaded.rasters.iter().map(|item| OpenItem::Raster(Box::new(item.clone()))));
                for item in items {
                    item.evict(&phase).unwrap();
                }
            });
        }
    }
}

/// Open a real archive (`OMF_BENCH_FILE`), load every unloaded item, and summarise.
#[test]
#[ignore = "profiling harness; run explicitly"]
fn omf_open_file_benchmark() {
    let path = std::env::var("OMF_BENCH_FILE").expect("OMF_BENCH_FILE");
    let bytes = std::fs::read(&path).unwrap();
    let progress = Progress::new();
    let phase = progress.phase(0.0, 1.0);
    for _ in 0..3 {
        let bundle = timed("file: open", || from_bytes("file.omf", bytes.clone(), &phase).unwrap());
        let deferred = bundle
            .triangulations
            .iter()
            .filter_map(|item| item.deferred.as_ref())
            .chain(bundle.block_models.iter().filter_map(|item| item.deferred.as_ref()))
            .chain(bundle.drill_holes.iter().filter_map(|item| item.deferred.as_ref()))
            .chain(bundle.point_clouds.iter().filter_map(|item| item.deferred.as_ref()))
            .chain(bundle.rasters.iter().filter_map(|item| item.deferred.as_ref()))
            .map(|(asset, _)| asset.clone())
            .collect::<Vec<_>>();
        let loaded = timed("file: materialize deferred", || deferred.iter().map(|asset| asset.read().unwrap()).collect::<Vec<_>>());
        let mut summary = format!(
            "designs={} objects={} tris={} blocks={} drill={} clouds={} rasters={} deferred={}",
            bundle.designs.len(),
            bundle.designs.iter().map(|design| design.document.objects().len()).sum::<usize>(),
            bundle.triangulations.len(),
            bundle.block_models.len(),
            bundle.drill_holes.len(),
            bundle.point_clouds.len(),
            bundle.rasters.len(),
            deferred.len()
        );
        for item in loaded.iter().chain([&bundle]) {
            for tri in &item.triangulations {
                summary += &format!(" tri[{}v/{}f]", tri.loaded.mesh.vertex_count(), tri.loaded.mesh.face_count());
            }
            for cloud in &item.point_clouds {
                summary += &format!(" cloud[{}p,colors={}]", cloud.loaded.points.len(), cloud.loaded.colors.is_some());
            }
            for model in &item.block_models {
                summary += &format!(" blocks[{}]", model.loaded.model.metadata.n_blocks);
            }
        }
        eprintln!("{summary}");
    }
}

/// Drill holes alone, written and read back.
#[test]
#[ignore = "profiling harness; run explicitly"]
fn omf_drill_layout_benchmark() {
    let progress = Progress::new();
    let phase = progress.phase(0.0, 1.0);
    let holes = drill_holes(&mut Rng(3), 1, 1500, true);
    let expected = holes.dataset.clone();
    let snapshot = ProjectSnapshot {
        name: "drill".to_owned(),
        drill_holes: vec![holes],
        ..Default::default()
    };
    for _ in 0..3 {
        let bytes = timed("drill: write", || to_bytes(snapshot.clone(), Compression::Archive, &phase).unwrap());
        let bundle = timed("drill: open", || from_bytes("d.omf", bytes.clone(), &phase).unwrap());
        assert_eq!(bundle.drill_holes[0].loaded.dataset.holes, expected.holes);
        assert!(bundle.warnings.is_empty(), "{:?}", bundle.warnings);
        let members = zip_member_count(&bytes);
        eprintln!("drill: size                        {:>9.2} MB in {members} members", bytes.len() as f64 / 1e6);
    }
}

fn zip_member_count(bytes: &[u8]) -> usize {
    let reader = omf_crate::file::Reader::new(bytes.to_vec()).unwrap();
    let (project, _) = reader.project().unwrap();
    fn arrays(value: &Value) -> usize {
        match value {
            Value::Object(map) => usize::from(map.contains_key("filename")) + map.values().map(arrays).sum::<usize>(),
            Value::Array(items) => items.iter().map(arrays).sum(),
            _ => 0,
        }
    }
    arrays(&serde_json::to_value(&project).unwrap()) + 1
}

/// One-off: rewrite `OMF_CONVERT_FILE`'s old per-hole drillhole datasets in the
/// consolidated layout, copying every other array byte for byte.
#[test]
#[ignore = "one-off conversion; run explicitly"]
fn omf_convert_old_drill_holes() {
    let path = std::env::var("OMF_CONVERT_FILE").expect("OMF_CONVERT_FILE");
    let bytes = std::fs::read(&path).unwrap();
    let mut reader = omf_crate::file::Reader::new(bytes.clone()).unwrap();
    reader.set_limits(reader_limits());
    let (project, _) = reader.project().unwrap();
    let mut writer = omf_crate::file::Writer::new(Cursor::new(Vec::new())).unwrap();
    writer.set_compression(Compression::Archive.into());

    // Every array is an object with a `filename`; copy its member and point
    // the reference at the copy.
    fn copy_arrays(value: &mut Value, writer: &mut omf_crate::file::Writer<Cursor<Vec<u8>>>, bytes: &[u8]) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(name)) = map.get("filename").cloned() {
                    let count = map.get("item_count").and_then(Value::as_u64).unwrap_or(0);
                    let member = zip_member(bytes, &name);
                    let copied = if name.ends_with(".parquet") {
                        serde_json::to_value(writer.array_bytes::<omf_crate::array_type::Vertex>(count, &member).unwrap()).unwrap()
                    } else {
                        serde_json::to_value(writer.image_bytes(&member).unwrap()).unwrap()
                    };
                    map.insert("filename".to_owned(), copied["filename"].clone());
                } else {
                    for child in map.values_mut() {
                        copy_arrays(child, writer, bytes);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|child| copy_arrays(child, writer, bytes)),
            _ => {}
        }
    }

    let mut converted = omf_crate::Project::new(project.name.clone());
    let mut expected = Vec::new();
    for element in &project.elements {
        if kind(element) == Some("drillhole_dataset") {
            let omf_crate::Geometry::Composite(group) = &element.geometry else { panic!() };
            let holes = group
                .elements
                .iter()
                .map(|child| DrillHole::deserialize(child.metadata.get("incline:drill_hole").expect("old layout")).unwrap())
                .collect::<Vec<_>>();
            let mut dataset = DrillHoleDataset::new(holes);
            if let Some(ties) = element.metadata.get(META_TIE_INS) {
                assert_eq!(dataset.apply_stored_ties(crate::model::drill_hole::StoredTieIns::deserialize(ties).unwrap()), 0);
            }
            expected.push(dataset.holes.clone());
            let open = OpenDrillHoleDataset {
                id: DrillHoleId(element_id(element).unwrap_or(1)),
                state: ProjectItemState::dirty(MemberKind::DrillHole, None).with_loaded(style_loaded(element.metadata.get(META_STYLE))),
                name: element.name.clone(),
                dataset: Arc::new(dataset),
                color: style_value(element.metadata.get(META_STYLE), "color").unwrap_or_default(),
            };
            let mut rewritten = write_drill_holes(&mut writer, &open).unwrap().unwrap();
            // Identity, provenance, folder and section travel unchanged.
            for (key, value) in &element.metadata {
                if key != META_KIND {
                    rewritten.metadata.insert(key.clone(), value.clone());
                }
            }
            converted.elements.push(rewritten);
        } else {
            let mut value = serde_json::to_value(element).unwrap();
            copy_arrays(&mut value, &mut writer, &bytes);
            converted.elements.push(serde_json::from_value(value).unwrap());
        }
    }
    assert!(!expected.is_empty(), "no drillhole datasets to convert");
    converted.description = project.description.clone();
    converted.author = project.author.clone();
    converted.application = project.application.clone();
    converted.coordinate_reference_system = project.coordinate_reference_system.clone();
    converted.units = project.units.clone();
    converted.origin = project.origin;
    converted.metadata = project.metadata.clone();
    let output = writer.finish(converted).unwrap().0.into_inner();

    let progress = Progress::new();
    let bundle = from_bytes("converted.omf", output.clone(), &progress.phase(0.0, 1.0)).unwrap();
    let mut read = Vec::new();
    for item in &bundle.drill_holes {
        match &item.deferred {
            Some((asset, _)) => read.push(asset.read().unwrap().drill_holes.pop().unwrap().loaded.dataset.holes.clone()),
            None => read.push(item.loaded.dataset.holes.clone()),
        }
    }
    // Origins were applied by the reader; the old holes are relative to it.
    let origin = DVec3::from_array(project.origin);
    for holes in &mut expected {
        for hole in holes {
            hole.collar += origin;
            hole.trace.iter_mut().for_each(|station| station.position += origin);
        }
    }
    assert_eq!(read, expected);
    eprintln!(
        "converted {} dataset(s): {} -> {} bytes; warnings: {:?}",
        expected.len(),
        bytes.len(),
        output.len(),
        bundle.warnings
    );
    std::fs::write(&path, output).unwrap();
}

/// A stored member's bytes, located through the central directory.
fn zip_member(archive: &[u8], name: &str) -> Vec<u8> {
    let u16_at = |at: usize| u16::from_le_bytes([archive[at], archive[at + 1]]) as usize;
    let u32_at = |at: usize| u32::from_le_bytes(archive[at..at + 4].try_into().unwrap()) as u64;
    let u64_at = |at: usize| u64::from_le_bytes(archive[at..at + 8].try_into().unwrap());
    let mut offset = 0;
    while let Some(position) = archive[offset..].windows(4).position(|window| window == [0x50, 0x4b, 0x01, 0x02]) {
        let start = offset + position;
        offset = start + 4;
        let name_len = u16_at(start + 28);
        if &archive[start + 46..start + 46 + name_len] != name.as_bytes() {
            continue;
        }
        let (mut compressed, mut uncompressed, mut header) = (u32_at(start + 20), u32_at(start + 24), u32_at(start + 42));
        // Zip64: the extra field holds, in order, whichever of these overflowed.
        let extra = start + 46 + name_len;
        let mut at = extra;
        while at + 4 <= extra + u16_at(start + 30) {
            let (id, len) = (u16_at(at), u16_at(at + 2));
            if id == 1 {
                let mut field = at + 4;
                for value in [&mut uncompressed, &mut compressed, &mut header] {
                    if *value == u32::MAX as u64 {
                        *value = u64_at(field);
                        field += 8;
                    }
                }
            }
            at += 4 + len;
        }
        let header = header as usize;
        let data = header + 30 + u16_at(header + 26) + u16_at(header + 28);
        return archive[data..data + compressed as usize].to_vec();
    }
    panic!("member {name} not found");
}
