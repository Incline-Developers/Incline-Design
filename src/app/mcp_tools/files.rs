//! The tools that read and write files: importing data, looking inside a CSV
//! before importing it, exporting, and saving and opening projects. Paths
//! are on this computer, and must be absolute.

use std::{collections::HashSet, path::PathBuf};

use serde_json::{Value, json};

use super::{Target, failed, ok};
use crate::{
    app::{App, commands::file::FileDialogAction},
    model::{
        ItemRef,
        drill_hole::DrillHoleSource,
        formats::{
            csv_block_model::{self, CsvColumnRole, FixedDimension},
            csv_drill_hole::{self, CsvDrillColumnRole, CsvDrillFileMapping, CsvDrillFileRole},
        },
    },
    ui::state::{OmfExportSection, OmfExportSelection, UiCommand},
};

/// What each importable file type is imported as, by extension.
const IMPORTS: [(&str, &[&str]); 6] = [
    ("project", &["omf"]),
    ("design", &["dxf"]),
    ("surface", &["obj", "stl", "ply"]),
    ("point_cloud", &["las", "laz", "xyz", "pts", "pcd"]),
    ("raster", &["tif", "tiff"]),
    ("optimisation_scenarios", &["json"]),
];

/// Block model CSV column roles, as the tools name them.
const BLOCK_ROLES: [(&str, CsvColumnRole); 10] = [
    ("value", CsvColumnRole::Value),
    ("category", CsvColumnRole::Category),
    ("ignore", CsvColumnRole::Ignore),
    ("id", CsvColumnRole::Id),
    ("x", CsvColumnRole::X),
    ("y", CsvColumnRole::Y),
    ("z", CsvColumnRole::Z),
    ("dx", CsvColumnRole::Dx),
    ("dy", CsvColumnRole::Dy),
    ("dz", CsvColumnRole::Dz),
];

/// Drillhole CSV table kinds, as the tools name them.
const TABLES: [(&str, CsvDrillFileRole); 5] = [
    ("collar", CsvDrillFileRole::Collar),
    ("survey", CsvDrillFileRole::Survey),
    ("interval", CsvDrillFileRole::Interval),
    ("segments", CsvDrillFileRole::ExplicitSegments),
    ("geophysics", CsvDrillFileRole::Geophysics),
];

/// Drillhole CSV column roles, as the tools name them. Any other name is a
/// field of that name.
const DRILL_ROLES: [(&str, CsvDrillColumnRole); 19] = [
    ("ignore", CsvDrillColumnRole::Ignore),
    ("hole_id", CsvDrillColumnRole::Dhid),
    ("east", CsvDrillColumnRole::East),
    ("north", CsvDrillColumnRole::North),
    ("elevation", CsvDrillColumnRole::Elevation),
    ("depth", CsvDrillColumnRole::Depth),
    ("azimuth", CsvDrillColumnRole::Azimuth),
    ("dip", CsvDrillColumnRole::Dip),
    ("inclination", CsvDrillColumnRole::Inclination),
    ("from", CsvDrillColumnRole::From),
    ("to", CsvDrillColumnRole::To),
    ("start_east", CsvDrillColumnRole::StartEast),
    ("start_north", CsvDrillColumnRole::StartNorth),
    ("start_elevation", CsvDrillColumnRole::StartElevation),
    ("end_east", CsvDrillColumnRole::EndEast),
    ("end_north", CsvDrillColumnRole::EndNorth),
    ("end_elevation", CsvDrillColumnRole::EndElevation),
    ("diameter", CsvDrillColumnRole::Diameter),
    ("gamma", CsvDrillColumnRole::Gamma),
];

fn block_role_name(role: CsvColumnRole) -> &'static str {
    BLOCK_ROLES.iter().find(|(_, candidate)| *candidate == role).map_or("ignore", |(name, _)| name)
}

fn drill_role_name(role: &CsvDrillColumnRole) -> String {
    match role {
        CsvDrillColumnRole::Attribute(name) => format!("field:{name}"),
        CsvDrillColumnRole::LongDensity => "long_density".to_owned(),
        CsvDrillColumnRole::ShortDensity => "short_density".to_owned(),
        role => DRILL_ROLES.iter().find(|(_, candidate)| candidate == role).map_or("ignore", |(name, _)| name).to_owned(),
    }
}

fn parse_drill_role(name: &str) -> CsvDrillColumnRole {
    match name {
        "long_density" => CsvDrillColumnRole::LongDensity,
        "short_density" => CsvDrillColumnRole::ShortDensity,
        name => match DRILL_ROLES.iter().find(|(key, _)| *key == name) {
            Some((_, role)) => role.clone(),
            None => CsvDrillColumnRole::Attribute(name.strip_prefix("field:").unwrap_or(name).to_owned()),
        },
    }
}

/// An absolute path from the arguments.
fn absolute(value: &Value) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.as_str().ok_or("Give a path.")?);
    if !path.is_absolute() {
        return Err(format!("{} is not an absolute path.", path.display()));
    }
    Ok(path)
}

/// A tool's schema.
fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false },
    })
}

pub(super) fn definitions() -> Vec<Value> {
    let path = json!({ "type": "string", "description": "An absolute path on this computer." });
    let imports: Vec<String> = IMPORTS.iter().map(|(kind, extensions)| format!("{} ({kind})", extensions.join(", "))).collect();
    vec![
        tool(
            "import_files",
            "Import files",
            &format!(
                "Import files into the open project, as the File > Import menus do, each by its extension: {}. A project file's contents are added to this one. CSV files are imported with import_block_model_csv or import_drillholes_csv. Runs in the background: get_tasks follows it.",
                imports.join("; ")
            ),
            json!({ "paths": { "type": "array", "minItems": 1, "items": path } }),
            &["paths"],
            false,
        ),
        tool(
            "preview_csv",
            "Look inside a CSV",
            "Read a CSV file's headers and first rows, with how Incline Design would map its columns if it were imported as a block model, and as which drillhole table, so the mapping can be checked or changed before importing.",
            json!({ "path": path }),
            &["path"],
            true,
        ),
        tool(
            "import_block_model_csv",
            "Import a block model CSV",
            "Import a block model from a CSV of block centroids, as File > Import > CSV Block Model does. Columns are mapped as preview_csv guesses; give columns to change any (header to role: value, category, ignore, id, x, y, z, dx, dy, dz). Give block_size when the file has no dx, dy and dz columns. Runs in the background.",
            json!({
                "path": path,
                "columns": { "type": "object", "additionalProperties": { "type": "string", "enum": BLOCK_ROLES.map(|(name, _)| name) } },
                "block_size": { "type": "array", "items": { "type": "number", "exclusiveMinimum": 0 }, "minItems": 3, "maxItems": 3 },
            }),
            &["path"],
            false,
        ),
        tool(
            "import_drillholes_csv",
            "Import drillhole CSVs",
            "Import a drillhole collection from CSV tables, as File > Import > Drillhole CSV does: a collar table with a survey and interval tables, or a table of explicit segments. Each file's table kind and columns are guessed as preview_csv shows; give table and columns to change them. Column roles: ignore, hole_id, east, north, elevation, depth, azimuth, dip, inclination, from, to, start_east, start_north, start_elevation, end_east, end_north, end_elevation, diameter, gamma, long_density, short_density; any other name makes the column a field of that name. Runs in the background.",
            json!({
                "name": { "type": "string", "minLength": 1, "description": "The new collection's name." },
                "files": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": path,
                            "table": { "type": "string", "enum": TABLES.map(|(name, _)| name) },
                            "columns": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Header to role." },
                        },
                        "required": ["path"],
                    },
                },
            }),
            &["name", "files"],
            false,
        ),
        tool(
            "export",
            "Export",
            "Write part of the project to a file, as the File > Export menus do. what: omf (the items listed, or everything), project_dxf (every design layer), layer_dxf (one design layer), surface (obj, stl or ply, by the path's extension), block_model_csv, drillholes_csv (a set of CSV tables named from the path), viewport_png, haul_roads_dxf (the haul network's roads), or optimisation_scenarios (every pit optimisation scenario, as JSON). Runs in the background.",
            json!({
                "what": { "type": "string", "enum": ["omf", "project_dxf", "layer_dxf", "surface", "block_model_csv", "drillholes_csv", "viewport_png", "haul_roads_dxf", "optimisation_scenarios"] },
                "id": { "type": "integer", "minimum": 0, "description": "The layer, surface, block model or drillhole collection, for those." },
                "items": super::items_schema("omf: what to export. Everything if left out."),
                "path": path,
            }),
            &["what", "path"],
            false,
        ),
        tool(
            "save_project",
            "Save the project",
            "Save the open project, as File > Save does, or to a new file, as Save As does. A project never saved needs a path.",
            json!({ "path": { "type": "string", "description": "Save as this absolute .omf path. The project's own file if left out." } }),
            &[],
            false,
        ),
        tool(
            "new_project",
            "Start a new project",
            "Start a new, empty project in place of the one open, as File > New does. If the open project has unsaved changes, the user is asked in the window whether to save them first, unless discard_changes is given; with no window (mcp --headless), the call fails instead.",
            json!({ "discard_changes": discard_changes() }),
            &[],
            false,
        ),
        tool(
            "open_project",
            "Open a project",
            "Open a project file in place of the one open, as File > Open does. If the open project has unsaved changes, the user is asked in the window whether to save them first, unless discard_changes is given; with no window (mcp --headless), the call fails instead.",
            json!({ "path": path, "discard_changes": discard_changes() }),
            &["path"],
            false,
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_file_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "import_files" => self.mcp_import_files(arguments),
            "preview_csv" => preview_csv(arguments),
            "import_block_model_csv" => self.mcp_import_block_model(arguments),
            "import_drillholes_csv" => self.mcp_import_drillholes(arguments),
            "export" => self.mcp_export(arguments),
            "save_project" => self.mcp_save_project(arguments),
            "open_project" => self.mcp_open_project(arguments),
            "new_project" => self
                .mcp_replace_project(FileDialogAction::NewProject, arguments)
                .map(|()| json!({ "new_project": true })),
            _ => return None,
        };
        Some(result.map_or_else(failed, |value| ok(&value)))
    }

    fn mcp_command(&mut self, command: UiCommand) -> Result<(), String> {
        self.handle_reported_ui_command(command).map_err(|error| format!("{error:#}"))
    }

    fn mcp_action(&mut self, action: FileDialogAction) -> Result<(), String> {
        self.execute_file_dialog_action(action).map_err(|error| format!("{error:#}"))
    }

    fn mcp_import_files(&mut self, arguments: &Value) -> Result<Value, String> {
        let paths = arguments["paths"]
            .as_array()
            .ok_or("Give the paths.")?
            .iter()
            .map(absolute)
            .collect::<Result<Vec<_>, _>>()?;
        let mut by_kind: Vec<(&str, Vec<PathBuf>)> = IMPORTS.iter().map(|(kind, _)| (*kind, Vec::new())).collect();
        for path in paths {
            let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default().to_lowercase();
            let Some(kind) = IMPORTS.iter().find(|(_, extensions)| extensions.contains(&extension.as_str())).map(|(kind, _)| *kind) else {
                return Err(format!("{} is not a file type import_files reads.", path.display()));
            };
            if !path.is_file() {
                return Err(format!("There is no file {}.", path.display()));
            }
            if let Some((_, paths)) = by_kind.iter_mut().find(|(candidate, _)| *candidate == kind) {
                paths.push(path);
            }
        }
        let mut started = Vec::new();
        for (kind, paths) in by_kind.into_iter().filter(|(_, paths)| !paths.is_empty()) {
            let count = paths.len();
            self.mcp_command(match kind {
                "project" => UiCommand::ImportOmfPaths(paths),
                "design" => UiCommand::ImportDxfPathsInto(paths),
                "surface" => UiCommand::ImportTriangulationPaths(paths),
                "point_cloud" => UiCommand::ImportPointCloudPaths(paths),
                "raster" => UiCommand::ImportRasterPaths(paths),
                _ => {
                    // Scenario files merge into the project's list one at a time.
                    self.ensure_optimization_scenarios_loaded();
                    for path in paths {
                        self.mcp_action(FileDialogAction::ImportOptimizationScenarios(path))?;
                    }
                    started.push(json!({ "as": kind, "files": count }));
                    continue;
                }
            })?;
            started.push(json!({ "as": kind, "files": count }));
        }
        Ok(json!({ "started": started }))
    }

    fn mcp_import_block_model(&mut self, arguments: &Value) -> Result<Value, String> {
        let path = absolute(&arguments["path"])?;
        let preview = csv_block_model::preview_path(&path).map_err(|error| error.to_string())?;
        let mut mapping = preview.mapping;
        if let Some(columns) = arguments["columns"].as_object() {
            for (header, role) in columns {
                let index = preview
                    .headers
                    .iter()
                    .position(|candidate| candidate == header)
                    .ok_or_else(|| format!("The CSV has no column {header}."))?;
                let role = role
                    .as_str()
                    .and_then(|role| BLOCK_ROLES.iter().find(|(name, _)| *name == role))
                    .map(|(_, role)| *role)
                    .ok_or_else(|| format!("{role} is not a column role."))?;
                mapping.roles[index] = role;
            }
        }
        if let Some(size) = arguments["block_size"].as_array() {
            let size: Vec<f64> = size.iter().filter_map(Value::as_f64).filter(|size| *size > 0.0).collect();
            let [dx, dy, dz] = size.as_slice() else {
                return Err("block_size must be [dx, dy, dz], each above 0.".to_owned());
            };
            mapping.fixed_dx = Some(FixedDimension(*dx));
            mapping.fixed_dy = Some(FixedDimension(*dy));
            mapping.fixed_dz = Some(FixedDimension(*dz));
        }
        let columns: serde_json::Map<String, Value> = preview
            .headers
            .iter()
            .zip(&mapping.roles)
            .map(|(header, role)| (header.clone(), json!(block_role_name(*role))))
            .collect();
        self.mcp_command(UiCommand::ImportCsvBlockModel { path, mapping })?;
        Ok(json!({ "started": true, "columns": columns }))
    }

    fn mcp_import_drillholes(&mut self, arguments: &Value) -> Result<Value, String> {
        let name = arguments["name"]
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or("Give the collection a name.")?;
        let mut files = Vec::new();
        let mut described = Vec::new();
        for file in arguments["files"].as_array().ok_or("Give the files.")? {
            let path = absolute(&file["path"])?;
            let preview = csv_drill_hole::preview_path(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            let mut mapping = csv_drill_hole::initial_mapping(path.clone(), &preview);
            if let Some(table) = file["table"].as_str() {
                let role = TABLES
                    .iter()
                    .find(|(key, _)| *key == table)
                    .map(|(_, role)| *role)
                    .ok_or_else(|| format!("{table} is not a table kind."))?;
                mapping = CsvDrillFileMapping {
                    columns: csv_drill_hole::default_columns(role, &preview.headers),
                    role,
                    path: path.clone(),
                };
            }
            if let Some(columns) = file["columns"].as_object() {
                for (header, role) in columns {
                    let index = preview
                        .headers
                        .iter()
                        .position(|candidate| candidate == header)
                        .ok_or_else(|| format!("{} has no column {header}.", path.display()))?;
                    mapping.columns[index] = parse_drill_role(role.as_str().ok_or("Column roles are names.")?);
                }
            }
            if mapping.role == CsvDrillFileRole::Unassigned {
                return Err(format!("Which table {} is could not be told from its headers: give its table.", path.display()));
            }
            described.push(json!({
                "path": path.display().to_string(),
                "table": TABLES.iter().find(|(_, role)| *role == mapping.role).map(|(name, _)| *name),
                "columns": preview.headers.iter().zip(&mapping.columns).map(|(header, role)| (header.clone(), json!(drill_role_name(role)))).collect::<serde_json::Map<_, _>>(),
            }));
            files.push(mapping);
        }
        self.mcp_command(UiCommand::ImportDrillHole(DrillHoleSource::Csv {
            name: name.to_owned(),
            files,
            browser_path: None,
        }))?;
        Ok(json!({ "started": true, "files": described }))
    }

    fn mcp_export(&mut self, arguments: &Value) -> Result<Value, String> {
        let path = absolute(&arguments["path"])?;
        let what = arguments["what"].as_str().ok_or("Give what to export.")?;
        let id = arguments["id"].as_u64();
        let need = |kind: &str| -> Result<u64, String> { id.ok_or_else(|| format!("Exporting {kind} needs its id.")) };
        let runtime_id = self.workspace.active_project().map(|project| project.runtime_id).ok_or("No project is open.")?;
        let action = match what {
            "omf" => {
                let selection = self.mcp_omf_selection(&arguments["items"])?;
                let snapshot = self.omf_export_snapshot(&selection).map_err(|error| format!("{error:#}"))?;
                FileDialogAction::ExportOmf {
                    snapshot: Box::new(snapshot),
                    path,
                }
            }
            "project_dxf" => FileDialogAction::ExportProjectDxf {
                project_runtime_id: runtime_id,
                path,
            },
            "layer_dxf" => match self.mcp_target("design_layer", need("a design layer")?)? {
                Target::Layer(layer) => FileDialogAction::ExportLayerDxf {
                    project_runtime_id: runtime_id,
                    layer,
                    path,
                },
                _ => unreachable!("a design_layer target is a layer"),
            },
            "surface" => match self.mcp_target("surface", need("a surface")?)? {
                Target::Item(ItemRef::Triangulation(id)) => FileDialogAction::ExportTriangulation { id, path },
                _ => unreachable!("a surface target is a triangulation"),
            },
            "block_model_csv" => match self.mcp_target("block_model", need("a block model")?)? {
                Target::Item(ItemRef::BlockModel(id)) => FileDialogAction::ExportBlockModelCsv { id, path },
                _ => unreachable!("a block_model target is a block model"),
            },
            "drillholes_csv" => match self.mcp_target("drillholes", need("drillholes")?)? {
                Target::Item(ItemRef::DrillHole(id)) => FileDialogAction::ExportDrillHoleCsv { id, path },
                _ => unreachable!("a drillholes target is a drillhole collection"),
            },
            "viewport_png" => FileDialogAction::ExportViewportImage(path),
            "haul_roads_dxf" => FileDialogAction::ExportHaulRoads {
                project: Box::new(self.haul_roads_project().ok_or("No project is open.")?),
                path,
            },
            "optimisation_scenarios" => {
                self.ensure_optimization_scenarios_loaded();
                FileDialogAction::ExportOptimizationScenarios(path)
            }
            _ => return Err("what is omf, project_dxf, layer_dxf, surface, block_model_csv, drillholes_csv, viewport_png, haul_roads_dxf or optimisation_scenarios.".to_owned()),
        };
        self.mcp_action(action)?;
        Ok(json!({ "started": what }))
    }

    /// What an OMF export takes: the items listed, or everything.
    fn mcp_omf_selection(&self, items: &Value) -> Result<OmfExportSelection, String> {
        let targets = self.mcp_targets(items)?;
        if targets.is_empty() {
            return Ok(OmfExportSelection::default());
        }
        fn only<Id: Eq + std::hash::Hash>(ids: impl Iterator<Item = Id>) -> OmfExportSection<Id> {
            OmfExportSection {
                all: false,
                items: ids.collect::<HashSet<_>>(),
            }
        }
        Ok(OmfExportSelection {
            designs: only(targets.iter().filter_map(|target| match target {
                Target::Layer(id) => Some(*id),
                _ => None,
            })),
            triangulations: only(targets.iter().filter_map(|target| match target {
                Target::Item(ItemRef::Triangulation(id)) => Some(*id),
                _ => None,
            })),
            rasters: only(targets.iter().filter_map(|target| match target {
                Target::Item(ItemRef::Raster(id)) => Some(*id),
                _ => None,
            })),
            point_clouds: only(targets.iter().filter_map(|target| match target {
                Target::Item(ItemRef::PointCloud(id)) => Some(*id),
                _ => None,
            })),
            block_models: only(targets.iter().filter_map(|target| match target {
                Target::Item(ItemRef::BlockModel(id)) => Some(*id),
                _ => None,
            })),
            drill_holes: only(targets.iter().filter_map(|target| match target {
                Target::Item(ItemRef::DrillHole(id)) => Some(*id),
                _ => None,
            })),
        })
    }

    fn mcp_save_project(&mut self, arguments: &Value) -> Result<Value, String> {
        let project = self.workspace.active_project().ok_or("No project is open.")?;
        let runtime_id = project.runtime_id;
        if arguments["path"].is_string() {
            let path = absolute(&arguments["path"])?;
            self.mcp_action(FileDialogAction::SaveProjectAs {
                project_runtime_id: runtime_id,
                path: path.clone(),
            })?;
            return Ok(json!({ "saving_to": path.display().to_string() }));
        }
        if project.path.is_none() {
            return Err("This project has never been saved: give a path.".to_owned());
        }
        self.mcp_command(UiCommand::SaveProject)?;
        Ok(json!({ "saving": true }))
    }

    fn mcp_open_project(&mut self, arguments: &Value) -> Result<Value, String> {
        let path = absolute(&arguments["path"])?;
        if !path.is_file() {
            return Err(format!("There is no file {}.", path.display()));
        }
        self.mcp_replace_project(FileDialogAction::OpenProject(vec![path.clone()]), arguments)?;
        Ok(json!({ "opening": path.display().to_string() }))
    }

    /// Put `action`'s project in place of the one open: its unsaved changes
    /// dropped when the arguments say to, else left for the user to decide
    /// on in the window, or, with no window, refused.
    fn mcp_replace_project(&mut self, action: FileDialogAction, arguments: &Value) -> Result<(), String> {
        let discard = arguments["discard_changes"].as_bool().unwrap_or(false);
        if !discard && self.headless && self.has_unsaved_changes_for_exit() {
            return Err("The open project has unsaved changes. Save them with save_project, or pass discard_changes to drop them.".to_owned());
        }
        self.project_replacement_bypass = discard;
        let result = self.mcp_action(action);
        self.project_replacement_bypass = false;
        result
    }
}

/// A CSV's headers, first rows, and the mappings an import would guess.
fn preview_csv(arguments: &Value) -> Result<Value, String> {
    let path = absolute(&arguments["path"])?;
    let block = csv_block_model::preview_path(&path).map_err(|error| error.to_string())?;
    let drill = csv_drill_hole::preview_path(&path).map_err(|error| error.to_string())?;
    let mapping = csv_drill_hole::initial_mapping(path, &drill);
    Ok(json!({
        "headers": block.headers,
        "rows": block.rows,
        "as_block_model": block.headers.iter().zip(&block.mapping.roles).map(|(header, role)| (header.clone(), json!(block_role_name(*role)))).collect::<serde_json::Map<_, _>>(),
        "as_drillholes": {
            "table": TABLES.iter().find(|(_, role)| *role == mapping.role).map(|(name, _)| *name),
            "columns": drill.headers.iter().zip(&mapping.columns).map(|(header, role)| (header.clone(), json!(drill_role_name(role)))).collect::<serde_json::Map<_, _>>(),
        },
    }))
}

/// The schema of the argument that drops a project's unsaved changes.
fn discard_changes() -> Value {
    json!({ "type": "boolean", "description": "Drop the open project's unsaved changes without asking. False if left out." })
}
