//! Point cloud import, load/unload and explorer commands.

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use anyhow::Context;
#[cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
use glam::DVec3;

#[cfg(not(target_arch = "wasm32"))]
use crate::app::file_name;
#[cfg(not(target_arch = "wasm32"))]
use crate::model::formats::point_cloud::{PointCloudFormat, read_point_cloud};
use crate::{
    app::App,
    i18n::{tr, tr_format},
    model::{
        SceneEntityId,
        formats::point_cloud::try_vec_with_capacity,
        point_cloud::{LoadedPointCloud, OpenPointCloud, PointCloudId, finite_bounds, prepare_for_render},
    },
    userspace_log, userspace_warn,
};

/// Uniform colour for clouds without per-point colours.
const DEFAULT_POINT_CLOUD_COLOR: [f32; 4] = [0.85, 0.87, 0.9, 1.0];
/// Screen-facing splat width in source/world metres.
const DEFAULT_POINT_SIZE: f32 = 0.1;
/// Share of a point-cloud load spent decoding the file, before the render
/// buffers are built from the decoded points. Decoding reports exactly within
/// its share; the preparation that follows is a single pass.
const DECODE_SHARE: f32 = 0.8;

impl<'a> App<'a> {
    pub(super) fn add_loaded_point_cloud(&mut self, loaded: LoadedPointCloud, visible: bool, color: [f32; 4], point_size: f32) {
        let id = PointCloudId(self.next_point_cloud_id);
        self.next_point_cloud_id += 1;
        let name = crate::model::project::unique_item_name(loaded.name, self.point_clouds.iter().map(|item| item.name.as_str()));
        userspace_log!(
            "{}",
            tr_format!(literal = "Loaded point cloud %name% (%count% points)", name = name.clone(), count = loaded.points.len())
        );
        self.point_clouds.push(OpenPointCloud {
            id,
            state: crate::model::project::ProjectItemState::dirty(loaded.path.file_name().map(|name| name.to_string_lossy().into_owned())).with_loaded(visible),
            name,
            points: loaded.points,
            colors: loaded.colors,
            classifications: loaded.classifications,
            prepared: loaded.prepared,
            bounds: loaded.bounds,
            color,
            point_size,
        });
        self.touch_active_project_content();
        self.invalidate_topology_bounds_and_redraw();
    }

    /// Decode point-cloud bytes chosen in the browser into retained project
    /// data. `display_path` contains a filename only and is used for
    /// provenance during the import transaction.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn open_point_cloud_input(&mut self, input: crate::model::input::InputFile, display_path: std::path::PathBuf) {
        let source_name = input.source.name.clone();
        let name = crate::model::project::imported_item_name(std::path::Path::new(&source_name), &crate::i18n::tr!(literal = "Point cloud"));
        self.spawn_job_reporting_progress(
            crate::i18n::tr_format!(literal = "Loading %name%", name = &source_name),
            vec![crate::app::jobs::JobKey::Anonymous],
            move |cancel, progress| {
                if cancel.is_cancelled() {
                    anyhow::bail!("Cancelled");
                }
                let data = crate::model::formats::point_cloud::read_point_cloud_bytes(&input.source.name, &input.bytes, &progress.phase(0.0, DECODE_SHARE))?;
                if data.points.is_empty() {
                    anyhow::bail!("Point cloud {} contains no points", input.source.name);
                }
                let (min, max) = data
                    .bounds
                    .or_else(|| finite_bounds(&data.points))
                    .with_context(|| format!("Point cloud {} contains no finite points", input.source.name))?;
                let prepared = prepare_for_render(&data.points, data.colors.as_deref(), data.classifications.as_deref(), (min, max));
                let colors = data.colors.map(std::sync::Arc::new);
                let classifications = data.classifications.map(std::sync::Arc::new);
                progress.set_fraction(1.0);
                Ok(LoadedPointCloud {
                    name,
                    path: display_path,
                    points: std::sync::Arc::new(data.points),
                    colors,
                    classifications,
                    prepared: std::sync::Arc::new(prepared),
                    bounds: (min, max),
                })
            },
            move |app, result| match result {
                Ok(loaded) => {
                    let should_fit = !app.scene_has_renderables();
                    app.add_loaded_point_cloud(loaded, true, DEFAULT_POINT_CLOUD_COLOR, DEFAULT_POINT_SIZE);
                    if should_fit {
                        app.fit_view_to_extents();
                    }
                    app.invalidate_topology_bounds_and_redraw();
                }
                Err(error) => userspace_warn!("{}", tr_format!(literal = "Failed to load point cloud: %error%", error = format!("{error:#}"))),
            },
        );
    }

    /// Entry point for point-cloud files chosen in the Import menu.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn import_point_cloud_path(&mut self, path: &Path) -> Result<()> {
        if PointCloudFormat::from_path(path).is_none() {
            anyhow::bail!("Unsupported point cloud file: {}", path.display());
        }
        if !path.is_file() {
            anyhow::bail!("Point cloud file does not exist: {}", path.display());
        }
        self.open_point_cloud_path(path.to_path_buf());
        Ok(())
    }

    /// Decode a point cloud on a background thread; completion is drained by
    /// `poll_point_cloud_loads`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_point_cloud_path(&mut self, path: PathBuf) {
        if self.pending_point_cloud_loads.iter().any(|(_, pending, _, _)| *pending == path) {
            return;
        }

        let source_name = file_name(&path);
        let name = crate::model::project::imported_item_name(&path, &crate::i18n::tr!(literal = "Point cloud"));
        let (ticket, progress) = self.begin_reported_task(crate::i18n::tr_format!(literal = "Loading %name%", name = &source_name));

        let (tx, rx) = std::sync::mpsc::channel();
        let console_report = crate::logging::retain_current_report();
        let worker_console_report = console_report.as_ref().map(crate::logging::ConsoleReportHandle::child);
        self.pending_point_cloud_loads.push((ticket, path.clone(), rx, console_report));
        let window = self.window.clone();
        crate::app::jobs::spawn_pool_task(move || {
            let compute = || {
                crate::app::jobs::run_compute_catching_panic(|| -> Result<LoadedPointCloud> {
                    let data = read_point_cloud(&path, &progress.phase(0.0, DECODE_SHARE)).with_context(|| format!("Failed to read point cloud {}", path.display()))?;
                    if data.points.is_empty() {
                        anyhow::bail!("Point cloud {} contains no points", path.display());
                    }
                    let (min, max) = data
                        .bounds
                        .or_else(|| finite_bounds(&data.points))
                        .with_context(|| format!("Point cloud {} contains no finite points", path.display()))?;
                    let prepared = prepare_for_render(&data.points, data.colors.as_deref(), data.classifications.as_deref(), (min, max));
                    let colors = data.colors.map(std::sync::Arc::new);
                    let classifications = data.classifications.map(std::sync::Arc::new);
                    progress.set_fraction(1.0);
                    Ok(LoadedPointCloud {
                        name,
                        path,
                        points: std::sync::Arc::new(data.points),
                        colors,
                        classifications,
                        prepared: std::sync::Arc::new(prepared),
                        bounds: (min, max),
                    })
                })
            };
            let result = if let Some(report) = worker_console_report.as_ref() {
                report.scope(compute)
            } else {
                compute()
            };
            let _ = tx.send(result);
            if let Some(window) = window {
                window.request_redraw();
            }
        });
    }

    pub(crate) fn poll_point_cloud_loads(&mut self) {
        let receivers = std::mem::take(&mut self.pending_point_cloud_loads);
        let mut still_pending = Vec::new();
        for (ticket, path, rx, console_report) in receivers {
            let result = rx.try_recv();
            if matches!(result, Err(std::sync::mpsc::TryRecvError::Empty)) {
                still_pending.push((ticket, path, rx, console_report));
                continue;
            }
            let complete = || match result {
                Ok(Ok(loaded)) => {
                    let should_fit = !self.scene_has_renderables();
                    self.add_loaded_point_cloud(loaded, true, DEFAULT_POINT_CLOUD_COLOR, DEFAULT_POINT_SIZE);
                    if should_fit {
                        self.fit_view_to_extents();
                    }
                    self.finish_background_task(ticket, true);
                    // Clouds render from point_cloud_gpu's per-id cache, not
                    // the document scene, so only bounds/redraw are stale.
                    self.invalidate_topology_bounds_and_redraw();
                }
                Ok(Err(error)) => {
                    userspace_warn!("{}", tr_format!(literal = "Failed to load point cloud: %error%", error = format!("{error:#}")));
                    self.finish_background_task(ticket, false);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => unreachable!(),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    userspace_warn!("{}", tr_format!(literal = "Point-cloud loader disconnected for %path%", path = path.display()));
                    self.finish_background_task(ticket, false);
                }
            };
            if let Some(report) = console_report.as_ref() {
                report.scope(complete);
            } else {
                complete();
            }
            drop(console_report);
        }
        self.pending_point_cloud_loads = still_pending;
    }

    pub(crate) fn close_point_cloud(&mut self, id: PointCloudId) {
        self.set_item_loaded(crate::model::ItemRef::PointCloud(id), false);
    }

    pub(crate) fn release_pointcloud_runtime(&mut self, id: PointCloudId) {
        self.cancel_jobs(|key| *key == crate::app::jobs::JobKey::PointCloud(id));
        let entity = SceneEntityId::PointCloud(id);
        self.editor.selected_handles.remove(&entity);
        self.editor.hidden_handles.remove(&entity);
        self.editor.translucent_handles.remove(&entity);
        self.invalidate_topology_bounds_and_redraw();
    }

    pub(crate) fn remove_point_cloud(&mut self, id: PointCloudId) {
        self.cancel_jobs(|key| *key == crate::app::jobs::JobKey::PointCloud(id));
        let entity = SceneEntityId::PointCloud(id);
        self.editor.selected_handles.remove(&entity);
        self.editor.hidden_handles.remove(&entity);
        self.editor.explicitly_frozen.remove(&entity);
        self.editor.frozen_handles.remove(&entity);
        self.editor.translucent_handles.remove(&entity);
        self.delete_project_item(crate::model::ItemRef::PointCloud(id));
    }

    /// Open the Join dialog, ticking every loaded cloud the first time and
    /// keeping a still-valid previous pick afterwards.
    pub(crate) fn open_point_cloud_join(&mut self) {
        // The clouds to join are chosen in the explorer or the viewport before
        // the dialog opens, so it reports the set rather than offering one.
        let sources = self.selected_point_clouds();
        if sources.len() < 2 {
            userspace_warn!("{}", tr!(literal = "Select two or more loaded point clouds before joining them"));
            return;
        }
        self.editor.point_cloud_join_open = true;
        self.editor.point_cloud_join_sources = sources;
        if self.editor.point_cloud_join_name_input.trim().is_empty() {
            self.editor.point_cloud_join_name_input = tr!(literal = "Joined Cloud");
        }
    }

    /// Concatenate several loaded clouds into one new cloud.
    ///
    /// Survey deliveries arrive tiled across many files, and every downstream
    /// tool - terrain reconstruction above all - works on one cloud at a time,
    /// so the tiles have to become a single cloud before they are useful.
    pub(crate) fn run_point_cloud_join(&mut self, cloud_ids: Vec<PointCloudId>, name: String, remove_sources: bool) -> anyhow::Result<()> {
        if cloud_ids.len() < 2 {
            anyhow::bail!("Select at least two point clouds to join");
        }
        let mut sources = Vec::with_capacity(cloud_ids.len());
        for id in &cloud_ids {
            let cloud = self
                .point_clouds
                .iter()
                .find(|cloud| cloud.id == *id)
                .ok_or_else(|| anyhow::anyhow!("A selected point cloud is no longer loaded"))?;
            sources.push(JoinSource {
                points: cloud.points.clone(),
                colors: cloud.colors.clone(),
                classifications: cloud.classifications.clone(),
                color: cloud.color,
                bounds: cloud.bounds,
            });
        }
        // The joined cloud inherits the first source's appearance, which is
        // what the uniform colour of an uncoloured cloud would have drawn.
        let color = sources[0].color;
        let point_size = self.point_clouds.iter().find(|cloud| cloud.id == cloud_ids[0]).map_or(0.1, |cloud| cloud.point_size);
        let keys = cloud_ids.iter().map(|id| crate::app::jobs::JobKey::PointCloud(*id)).collect();
        let job_name = name.clone();
        let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| -> anyhow::Result<LoadedPointCloud> {
            join_point_clouds(&sources, job_name, cancel, progress)
        };
        let removed = remove_sources.then(|| cloud_ids.clone());
        let apply = move |app: &mut App, result: anyhow::Result<LoadedPointCloud>| match result {
            Ok(loaded) => {
                app.add_loaded_point_cloud(loaded, true, color, point_size);
                if let Some(removed) = removed {
                    for id in removed {
                        app.remove_point_cloud(id);
                    }
                }
            }
            Err(error) => userspace_warn!("{}", tr_format!(literal = "Failed to join point clouds: %error%", error = format!("{error:#}"))),
        };
        self.spawn_job_reporting_progress(tr_format!(literal = "Joining %name%", name = &name), keys, compute, apply);
        Ok(())
    }
}

/// One cloud's contribution to a join, owned by the worker.
struct JoinSource {
    points: std::sync::Arc<Vec<DVec3>>,
    colors: Option<std::sync::Arc<Vec<u32>>>,
    classifications: Option<std::sync::Arc<Vec<u8>>>,
    color: [f32; 4],
    bounds: (DVec3, DVec3),
}

/// The uniform colour an uncoloured cloud draws with, in the packed RGBA8 the
/// render points carry.
fn packed_uniform_color(color: [f32; 4]) -> u32 {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u32;
    channel(color[0]) | (channel(color[1]) << 8) | (channel(color[2]) << 16) | (channel(color[3]) << 24)
}

/// Share of a join spent copying points, before the render hierarchy is built
/// from them. The copy reports per source; the preparation that follows is a
/// single pass.
const JOIN_COPY_SHARE: f32 = 0.35;

fn join_point_clouds(sources: &[JoinSource], name: String, cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress) -> anyhow::Result<LoadedPointCloud> {
    let total: usize = sources.iter().map(|source| source.points.len()).sum();
    if total == 0 {
        anyhow::bail!("The selected point clouds contain no points");
    }
    // One cloud carrying colours makes the join coloured: the rest contribute
    // the uniform colour they were drawn with, so the tiles stay tellable
    // apart rather than all turning grey.
    let colored = sources.iter().any(|source| source.colors.is_some());
    // Classifications, unlike colours, are all or nothing. Padding an
    // unclassified tile out to "unclassified" would leave a bare-earth filter
    // silently deleting that whole tile, so a mixed join keeps none of them and
    // says so.
    let classified = sources.iter().all(|source| source.classifications.is_some());
    if !classified && sources.iter().any(|source| source.classifications.is_some()) {
        userspace_warn!(
            "{}",
            tr!(literal = "Dropped point classifications: some of the joined clouds are unclassified, and a partly classified cloud cannot be filtered to ground.")
        );
    }
    let mut points = try_vec_with_capacity::<DVec3>(total, "joined point cloud")?;
    let mut colors = if colored {
        Some(try_vec_with_capacity::<u32>(total, "joined point cloud colours")?)
    } else {
        None
    };
    let mut classifications = if classified {
        Some(try_vec_with_capacity::<u8>(total, "joined point cloud classifications")?)
    } else {
        None
    };
    let mut copied = 0usize;
    for source in sources {
        if cancel.is_cancelled() {
            anyhow::bail!("Cancelled");
        }
        points.extend_from_slice(&source.points);
        if let Some(colors) = colors.as_mut() {
            // A short colour array is padded rather than trusted to match:
            // `prepare_for_render` indexes colours by source index.
            let matched = source.colors.as_deref().map_or(0, |values| values.len().min(source.points.len()));
            if let Some(values) = source.colors.as_deref() {
                colors.extend_from_slice(&values[..matched]);
            }
            colors.resize(colors.len() + source.points.len() - matched, packed_uniform_color(source.color));
        }
        if let Some(classifications) = classifications.as_mut() {
            // Padded the same way and for the same reason as the colours.
            let codes = source.classifications.as_deref().expect("a classified join has every source classified");
            let matched = codes.len().min(source.points.len());
            classifications.extend_from_slice(&codes[..matched]);
            classifications.resize(classifications.len() + source.points.len() - matched, crate::model::point_cloud::CLASS_UNCLASSIFIED);
        }
        copied += source.points.len();
        progress.set_fraction(JOIN_COPY_SHARE * copied as f32 / total as f32);
    }

    // Each source's stored bounds already cover its own points, so the union
    // is the joined extent without a second pass over every point.
    let bounds = sources
        .iter()
        .map(|source| source.bounds)
        .reduce(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))
        .filter(|(min, max)| min.is_finite() && max.is_finite())
        .or_else(|| finite_bounds(&points))
        .ok_or_else(|| anyhow::anyhow!("The selected point clouds contain no finite points"))?;
    let prepared = prepare_for_render(&points, colors.as_deref(), classifications.as_deref(), bounds);
    progress.set_fraction(1.0);
    userspace_log!(
        "{}",
        tr_format!(
            literal = "Joined %count% clouds into %name% (%points% points)",
            count = sources.len(),
            name = name.clone(),
            points = total
        )
    );
    Ok(LoadedPointCloud {
        name,
        // Synthesised rather than read: the project item carries no source file.
        path: std::path::PathBuf::new(),
        points: std::sync::Arc::new(points),
        colors: colors.map(std::sync::Arc::new),
        classifications: classifications.map(std::sync::Arc::new),
        prepared: std::sync::Arc::new(prepared),
        bounds,
    })
}
