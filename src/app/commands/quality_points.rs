//! Quality points: composited values from a CSV laid out as design layers,
//! one per seam and quality, each point at height = value.

use std::{collections::HashSet, path::PathBuf};

use anyhow::Result;
use glam::DVec3;

use crate::{
    app::App,
    i18n::tr,
    model::{
        Command, Document, Layer, Object, ObjectColor, ObjectId, SceneEntityId, SectionKind,
        formats::csv_quality_points::{self, ParsedQualityPoints, QualityCsvError, QualityMapping, QualityPreview, QualitySet},
        project::{imported_item_name, unique_item_name},
    },
    userspace_log, userspace_warn,
};

impl App<'_> {
    /// The mapping page's preview of a picked file, or the reason it has none.
    pub(crate) fn set_quality_preview(&mut self, preview: Option<Result<QualityPreview, QualityCsvError>>) {
        match preview.unwrap_or_else(|| Err(QualityCsvError::Invalid(tr!("io-no-csv-chosen")))) {
            Ok(preview) => {
                self.editor.import_quality_preview = Some(preview);
                self.editor.import_csv_error = None;
            }
            Err(error) => {
                self.editor.import_quality_preview = None;
                self.editor.import_csv_error = Some(error.to_string());
            }
        }
    }

    pub(crate) fn import_quality_points(&mut self, path: PathBuf, mapping: QualityMapping) -> Result<()> {
        let runtime_id = self
            .workspace
            .active_project()
            .map(|project| project.runtime_id)
            .ok_or_else(|| anyhow::anyhow!(tr!("common-no-open-project")))?;
        #[cfg(not(target_arch = "wasm32"))]
        let read = {
            let path = path.clone();
            move || std::fs::read(path)
        };
        #[cfg(target_arch = "wasm32")]
        let read = {
            let file = self
                .take_web_import_files(crate::ui::state::DataMenu::QualityPoints)?
                .pop()
                .ok_or_else(|| anyhow::anyhow!(tr!("io-no-csv-chosen")))?;
            self.editor.import_quality_preview = None;
            move || Ok::<_, std::io::Error>(file.bytes)
        };
        let file = imported_item_name(&path, &tr!("io-quality-points-file"));
        self.spawn_job_reporting_progress(
            tr!("cmd-quality-points-reading"),
            vec![crate::app::jobs::JobKey::Anonymous],
            move |cancel, _| {
                let bytes = read()?;
                if cancel.is_cancelled() {
                    anyhow::bail!("Cancelled");
                }
                Ok(csv_quality_points::parse(&bytes, &mapping)?)
            },
            move |app, result| match result {
                Ok(parsed) => app.add_quality_points(runtime_id, &file, &parsed),
                Err(error) => userspace_warn!("{error:#}"),
            },
        );
        Ok(())
    }

    /// Only into the project the file was picked for: a switch while the
    /// file was read leaves the result unused.
    fn add_quality_points(&mut self, runtime_id: u32, file: &str, parsed: &ParsedQualityPoints) {
        if self.workspace.active_project().is_none_or(|project| project.runtime_id != runtime_id) {
            userspace_warn!("{}", tr!("cmd-quality-points-project-changed"));
            return;
        }
        let known = self
            .drill_holes
            .iter()
            .flat_map(|dataset| &dataset.color.strat_columns)
            .flat_map(|column| column.codes.iter().map(String::as_str))
            .collect::<HashSet<_>>();
        if known.is_empty() {
            userspace_log!("{}", tr!("cmd-quality-points-no-strat-column"));
        } else {
            let unknown = unknown_seams(&parsed.sets, &known);
            if !unknown.is_empty() {
                userspace_warn!("{}", tr!("cmd-quality-points-unknown-seams", seams = unknown.join(", ")));
            }
        }
        let Some(project) = self.workspace.active_project_mut() else {
            return;
        };
        let (commands, made) = quality_layers(&mut project.project.document, &parsed.sets);
        self.execute_edit(Command::Batch(commands));
        // The points become the selection, so Build Surface is ready for them.
        self.select_only(made.iter().copied().map(SceneEntityId::Object));
        userspace_log!(
            "{}",
            tr!(
                "cmd-quality-points-made",
                file = file.to_owned(),
                layers = parsed.sets.len().to_string(),
                points = made.len().to_string(),
                blank = parsed.blank.to_string()
            )
        );
        self.invalidate_geometry();
    }
}

/// One layer of points per seam and quality, in file order, made as one
/// batch so the import is one undo step.
pub(crate) fn quality_layers(document: &mut Document, sets: &[QualitySet]) -> (Vec<Command>, Vec<ObjectId>) {
    let mut names: Vec<String> = Vec::with_capacity(sets.len());
    let mut commands = Vec::with_capacity(sets.len());
    let mut made = Vec::new();
    for set in sets {
        let requested = tr!("cmd-quality-points-layer", seam = set.seam.clone(), quality = set.quality.clone());
        let name = unique_item_name(requested, document.layers().iter().map(|layer| layer.name.as_str()).chain(names.iter().map(String::as_str)));
        let id = document.allocate_layer_id();
        let objects = set
            .points
            .iter()
            .map(|point| Object::Point {
                id: document.allocate_object_id(),
                layer: id,
                pos: DVec3::new(point.x, point.y, point.value),
                color: ObjectColor::ByLayer,
            })
            .collect::<Vec<_>>();
        made.extend(objects.iter().map(Object::id));
        names.push(name.clone());
        let layer = Layer {
            id,
            name,
            color_index: None,
            color: [1.0, 1.0, 1.0, 1.0],
            loaded: true,
            hidden: false,
            elevation: 0.0,
            folder: None,
            section: SectionKind::Designs,
        };
        commands.push(Command::AddLayerSnapshot { layer, objects });
    }
    (commands, made)
}

/// Seams no open strat column names, each once, in file order.
fn unknown_seams<'a>(sets: &'a [QualitySet], known: &HashSet<&str>) -> Vec<&'a str> {
    let mut unknown = Vec::new();
    for set in sets {
        if !known.contains(set.seam.as_str()) && !unknown.contains(&set.seam.as_str()) {
            unknown.push(set.seam.as_str());
        }
    }
    unknown
}
