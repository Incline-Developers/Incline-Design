//! The tools for the Planning workspace: the project's reserve fields and how
//! each block model maps onto them, the solids reserved against them, how
//! each is benched, blasted and excluded, running the planning steps, and
//! the reserves they measure.
//!
//! A solid's benching, blasting and exclusions are read and written whole,
//! as the JSON the project file keeps them in: `get_solids` shows that
//! shape, and `update_solid` takes it back edited.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{failed, ok, parse_hex};
use crate::{
    app::App,
    model::{
        ReserveAggregation, ReserveField, ReserveFieldId, SolidEdit, SolidId, SolidKind,
        block_model::{BlockModelId, ReserveMappingSource},
        solid_reserves::ReserveTotals,
        triangulation::TriangulationId,
    },
    ui::state::{RenameTarget, SolidsStep, UiCommand},
};

/// The planning steps a run can be asked for, as the tools name them.
const STEPS: [(&str, SolidsStep); 4] = [
    ("benching", SolidsStep::Benching),
    ("blasting", SolidsStep::Blasting),
    ("dig_strips", SolidsStep::DigStrips),
    ("reserving", SolidsStep::Reserving),
];

fn kind_name(kind: SolidKind) -> &'static str {
    match kind {
        SolidKind::Pit => "pit",
        SolidKind::Dump => "dump",
        SolidKind::Stockpile => "stockpile",
    }
}

fn parse_kind(value: &Value) -> Result<SolidKind, String> {
    match value.as_str() {
        Some("pit") => Ok(SolidKind::Pit),
        Some("dump") => Ok(SolidKind::Dump),
        Some("stockpile") => Ok(SolidKind::Stockpile),
        _ => Err("kind is pit, dump or stockpile.".to_owned()),
    }
}

/// A tool's schema.
fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": false, "idempotentHint": read_only },
    })
}

pub(super) fn definitions() -> Vec<Value> {
    let id = json!({ "type": "integer", "minimum": 0 });
    let optional_id = json!({ "type": ["integer", "null"], "minimum": 0 });
    vec![
        tool(
            "get_solids",
            "Get solids and reserve fields",
            "Read the Planning workspace's setup: the project's reserve fields (each with how it aggregates), each block model's mapping onto them and whether it is in the reserves, and every solid (kind, design surface, topography, block model, colour, and its benching, blasting and exclusions plans as stored). Also whether the reserves are measured and current.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "add_solid",
            "Add a solid",
            "Add a solid to reserve: the ground between a design surface (pit shell or dump design) and the topography, filled by a block model (required for a pit). Returns its id. One undo step.",
            json!({
                "name": { "type": "string", "minLength": 1 },
                "kind": { "type": "string", "enum": ["pit", "dump", "stockpile"] },
                "surface": optional_id,
                "topography": optional_id,
                "block_model": optional_id,
            }),
            &["name", "kind"],
            false,
        ),
        tool(
            "update_solid",
            "Change a solid",
            "Change a solid's setup. Only what is given changes. benching, blasting and exclusions replace those plans whole, in the shape get_solids shows them. Surfaces and block model take null to unset. Each change is an undo step.",
            json!({
                "id": id,
                "name": { "type": "string", "minLength": 1 },
                "kind": { "type": "string", "enum": ["pit", "dump", "stockpile"] },
                "colour": { "type": "string", "description": "#rrggbb" },
                "surface": optional_id,
                "topography": optional_id,
                "block_model": optional_id,
                "benching": { "type": "object" },
                "blasting": { "type": "object" },
                "exclusions": { "type": "object" },
            }),
            &["id"],
            false,
        ),
        tool(
            "delete_solid",
            "Delete a solid",
            "Delete a solid from the Planning setup. One undo step.",
            json!({ "id": id }),
            &["id"],
            false,
        ),
        tool(
            "set_reserve_field",
            "Add or change a reserve field",
            "Add a reserve field (a column every block model maps onto, such as Tonnes or Fe), or change one's name or aggregation: sum, volume_average (a density), weighted_average (a grade, by a summed field such as Tonnes) or category (a grouping such as rock type). Give id to change an existing field. Returns its id.",
            json!({
                "id": id,
                "name": { "type": "string", "minLength": 1 },
                "aggregation": { "type": "string", "enum": ["sum", "volume_average", "weighted_average", "category"] },
                "weight_field": { "type": "integer", "minimum": 0, "description": "weighted_average: the summed field it is weighted by." },
            }),
            &[],
            false,
        ),
        tool(
            "delete_reserve_field",
            "Delete a reserve field",
            "Delete a reserve field and every block model's mapping onto it.",
            json!({ "id": id }),
            &["id"],
            false,
        ),
        tool(
            "map_reserve_field",
            "Map a block model onto a reserve field",
            "Set where one block model's values for a reserve field come from: one of its variables (column), a variable given per cubic metre and multiplied by each block's volume (per_volume, such as density to tonnes), or one constant for every block. Give none of them to unmap. included puts the block model in the reserves or takes it out.",
            json!({
                "block_model": id,
                "field": { "type": "integer", "minimum": 0 },
                "column": { "type": "string" },
                "per_volume": { "type": "string" },
                "constant": { "type": "number" },
                "included": { "type": "boolean" },
            }),
            &["block_model"],
            false,
        ),
        tool(
            "run_planning",
            "Run the planning steps",
            "Run the Planning workspace's steps, as their Run buttons do: benching (the solids cut into benches and flitches), blasting (benches into blasts), dig_strips (flitches into dig blocks) and reserving (the dig blocks measured against the block models). With no step, all of them in order. Runs in the background: get_tasks follows it, and get_reserves reads the result.",
            json!({ "step": { "type": "string", "enum": STEPS.map(|(name, _)| name) } }),
            &[],
            false,
        ),
        tool(
            "get_reserves",
            "Get reserves",
            "Read the reserves the planning steps measured: each reserve field's figure, the volume and the dig block count, totalled by solid, by bench, or listed block by block. Category fields are broken down by category. Says what to run first if the reserves are not measured or out of date.",
            json!({ "by": { "type": "string", "enum": ["solid", "bench", "block"], "description": "solid if left out." } }),
            &[],
            true,
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_solid_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "get_solids" => Ok(self.mcp_solids()),
            "add_solid" => self.mcp_add_solid(arguments),
            "update_solid" => self.mcp_update_solid(arguments),
            "delete_solid" => self.mcp_delete_solid(arguments),
            "set_reserve_field" => self.mcp_set_reserve_field(arguments),
            "delete_reserve_field" => self.mcp_delete_reserve_field(arguments),
            "map_reserve_field" => self.mcp_map_reserve_field(arguments),
            "run_planning" => self.mcp_run_planning(arguments),
            "get_reserves" => self.mcp_reserves(arguments),
            _ => return None,
        };
        Some(result.map_or_else(failed, |value| ok(&value)))
    }

    fn mcp_ui(&mut self, command: UiCommand) -> Result<(), String> {
        self.handle_reported_ui_command(command).map_err(|error| format!("{error:#}"))
    }

    fn mcp_solids(&self) -> Value {
        let Some(document) = self.workspace.active_document() else {
            return json!({ "error": "No project is open." });
        };
        let fields: Vec<Value> = document.reserve_fields().iter().map(field_json).collect();
        let models: Vec<Value> = self
            .block_models
            .iter()
            .map(|model| {
                let mapping: Vec<Value> = model
                    .reserve_mapping
                    .iter()
                    .map(|mapping| {
                        let source = match &mapping.source {
                            ReserveMappingSource::Column(column) => json!({ "column": column }),
                            ReserveMappingSource::PerVolume(column) => json!({ "per_volume": column }),
                            ReserveMappingSource::Constant(value) => json!({ "constant": value }),
                        };
                        json!({ "field": mapping.field.0, "from": source })
                    })
                    .collect();
                json!({ "id": model.id.0, "name": model.name, "included": model.included_in_reserves, "mapping": mapping })
            })
            .collect();
        let solids: Vec<Value> = document
            .solids()
            .iter()
            .map(|solid| {
                json!({
                    "id": solid.id.0,
                    "name": solid.name,
                    "kind": kind_name(solid.kind),
                    "surface": solid.surface.map(|id| id.0),
                    "topography": solid.topography.map(|id| id.0),
                    "block_model": solid.block_model.map(|id| id.0),
                    "colour": super::hex(solid.color),
                    "benching": serde_json::to_value(&solid.benching).unwrap_or_default(),
                    "blasting": serde_json::to_value(&solid.blasting).unwrap_or_default(),
                    "exclusions": serde_json::to_value(&solid.exclusions).unwrap_or_default(),
                })
            })
            .collect();
        let reserves = match self.planning_snapshot() {
            Ok(_) => "measured and current".to_owned(),
            Err(not_ready) => super::plain(&not_ready.describe()),
        };
        json!({ "reserve_fields": fields, "block_models": models, "solids": solids, "reserves": reserves })
    }

    /// The surface `value` names, `None` for null, or why it names nothing.
    fn mcp_optional_surface(&self, value: &Value) -> Result<Option<TriangulationId>, String> {
        match value.as_u64() {
            None => Ok(None),
            Some(id) => self
                .triangulations
                .iter()
                .find(|surface| surface.id.0 == id)
                .map(|surface| Some(surface.id))
                .ok_or_else(|| format!("There is no surface with id {id}.")),
        }
    }

    fn mcp_optional_block_model(&self, value: &Value) -> Result<Option<BlockModelId>, String> {
        match value.as_u64() {
            None => Ok(None),
            Some(id) => self
                .block_models
                .iter()
                .find(|model| model.id.0 == id)
                .map(|model| Some(model.id))
                .ok_or_else(|| format!("There is no block model with id {id}.")),
        }
    }

    fn mcp_solid_ids(&self) -> Vec<SolidId> {
        self.workspace
            .active_document()
            .map(|document| document.solids().iter().map(|solid| solid.id).collect())
            .unwrap_or_default()
    }

    fn mcp_add_solid(&mut self, arguments: &Value) -> Result<Value, String> {
        let name = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()).ok_or("Give the solid a name.")?;
        let before = self.mcp_solid_ids();
        self.mcp_ui(UiCommand::AddSolid {
            name: name.to_owned(),
            kind: parse_kind(&arguments["kind"])?,
            surface: self.mcp_optional_surface(&arguments["surface"])?,
            topography: self.mcp_optional_surface(&arguments["topography"])?,
            block_model: self.mcp_optional_block_model(&arguments["block_model"])?,
        })?;
        let new = self.mcp_solid_ids().into_iter().find(|id| !before.contains(id));
        Ok(json!({ "id": new.map(|id| id.0) }))
    }

    fn mcp_update_solid(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["id"].as_u64().map(SolidId).ok_or("Give the solid's id.")?;
        if !self.mcp_solid_ids().contains(&id) {
            return Err(format!("There is no solid with id {}. get_solids lists them.", id.0));
        }
        let mut edits = Vec::new();
        if arguments.get("kind").is_some() {
            edits.push(SolidEdit::Kind(parse_kind(&arguments["kind"])?));
        }
        if let Some(colour) = arguments["colour"].as_str() {
            edits.push(SolidEdit::Color(parse_hex(colour).ok_or("colour must be #rrggbb.")?));
        }
        if let Some(value) = arguments.get("surface") {
            edits.push(SolidEdit::Surface(self.mcp_optional_surface(value)?));
        }
        if let Some(value) = arguments.get("topography") {
            edits.push(SolidEdit::Topography(self.mcp_optional_surface(value)?));
        }
        if let Some(value) = arguments.get("block_model") {
            edits.push(SolidEdit::BlockModel(self.mcp_optional_block_model(value)?));
        }
        if let Some(value) = arguments.get("benching") {
            edits.push(SolidEdit::Benching(
                serde_json::from_value(value.clone()).map_err(|error| format!("benching is not in the shape get_solids shows: {error}"))?,
            ));
        }
        if let Some(value) = arguments.get("blasting") {
            edits.push(SolidEdit::Blasting(
                serde_json::from_value(value.clone()).map_err(|error| format!("blasting is not in the shape get_solids shows: {error}"))?,
            ));
        }
        if let Some(value) = arguments.get("exclusions") {
            edits.push(SolidEdit::Exclusions(
                serde_json::from_value(value.clone()).map_err(|error| format!("exclusions is not in the shape get_solids shows: {error}"))?,
            ));
        }
        let changes = edits.len();
        for edit in edits {
            self.mcp_ui(UiCommand::UpdateSolid { solid: id, edit })?;
        }
        if let Some(name) = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty()) {
            self.mcp_ui(UiCommand::RenameItem {
                target: RenameTarget::Solid(id),
                new_name: name.to_owned(),
            })?;
        }
        Ok(json!({ "id": id.0, "changes": changes }))
    }

    fn mcp_delete_solid(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["id"].as_u64().map(SolidId).ok_or("Give the solid's id.")?;
        if !self.mcp_solid_ids().contains(&id) {
            return Err(format!("There is no solid with id {}.", id.0));
        }
        self.mcp_ui(UiCommand::DeleteSolid(id))?;
        Ok(json!({ "deleted": id.0 }))
    }

    fn mcp_reserve_fields(&self) -> Vec<ReserveField> {
        self.workspace.active_document().map(|document| document.reserve_fields().to_vec()).unwrap_or_default()
    }

    fn mcp_set_reserve_field(&mut self, arguments: &Value) -> Result<Value, String> {
        let fields = self.mcp_reserve_fields();
        let aggregation = match arguments["aggregation"].as_str() {
            None => None,
            Some("sum") => Some(ReserveAggregation::Sum),
            Some("volume_average") => Some(ReserveAggregation::VolumeAverage),
            Some("category") => Some(ReserveAggregation::Category),
            Some("weighted_average") => {
                let weight = arguments["weight_field"].as_u64().map(ReserveFieldId).ok_or("weighted_average needs weight_field.")?;
                if !fields.iter().any(|field| field.id == weight && field.aggregation == ReserveAggregation::Sum) {
                    return Err("weight_field must be a summed reserve field.".to_owned());
                }
                Some(ReserveAggregation::WeightedAverage { weight_field: weight })
            }
            Some(_) => return Err("aggregation is sum, volume_average, weighted_average or category.".to_owned()),
        };
        let name = arguments["name"].as_str().map(str::trim).filter(|name| !name.is_empty());
        let id = match arguments["id"].as_u64().map(ReserveFieldId) {
            Some(id) => {
                if !fields.iter().any(|field| field.id == id) {
                    return Err(format!("There is no reserve field with id {}.", id.0));
                }
                if let Some(aggregation) = aggregation {
                    self.mcp_ui(UiCommand::SetReserveFieldAggregation { field: id, aggregation })?;
                }
                if let Some(name) = name {
                    self.mcp_ui(UiCommand::RenameItem {
                        target: RenameTarget::ReserveField(id),
                        new_name: name.to_owned(),
                    })?;
                }
                id
            }
            None => {
                let name = name.ok_or("A new reserve field needs a name.")?;
                self.mcp_ui(UiCommand::AddReserveField {
                    name: name.to_owned(),
                    aggregation: aggregation.unwrap_or(ReserveAggregation::Sum),
                })?;
                self.mcp_reserve_fields()
                    .into_iter()
                    .find(|field| !fields.iter().any(|old| old.id == field.id))
                    .map(|field| field.id)
                    .ok_or("The field was not added: is the name already taken?")?
            }
        };
        Ok(json!({ "id": id.0 }))
    }

    fn mcp_delete_reserve_field(&mut self, arguments: &Value) -> Result<Value, String> {
        let id = arguments["id"].as_u64().map(ReserveFieldId).ok_or("Give the field's id.")?;
        if !self.mcp_reserve_fields().iter().any(|field| field.id == id) {
            return Err(format!("There is no reserve field with id {}.", id.0));
        }
        self.mcp_ui(UiCommand::DeleteReserveField(id))?;
        Ok(json!({ "deleted": id.0 }))
    }

    fn mcp_map_reserve_field(&mut self, arguments: &Value) -> Result<Value, String> {
        let model = self.mcp_optional_block_model(&arguments["block_model"])?.ok_or("Give the block model's id.")?;
        if let Some(field) = arguments["field"].as_u64().map(ReserveFieldId) {
            if !self.mcp_reserve_fields().iter().any(|candidate| candidate.id == field) {
                return Err(format!("There is no reserve field with id {}.", field.0));
            }
            let variables: Vec<String> = self
                .block_models
                .iter()
                .find(|candidate| candidate.id == model)
                .map(|candidate| candidate.model.metadata.variables.iter().map(|variable| variable.name.clone()).collect())
                .unwrap_or_default();
            let column = |key: &str| -> Result<Option<String>, String> {
                match arguments[key].as_str() {
                    None => Ok(None),
                    Some(name) if variables.iter().any(|variable| variable == name) => Ok(Some(name.to_owned())),
                    Some(name) => Err(format!("The block model has no variable {name}.")),
                }
            };
            let source = match (column("column")?, column("per_volume")?, arguments["constant"].as_f64()) {
                (Some(name), None, None) => Some(ReserveMappingSource::Column(name)),
                (None, Some(name), None) => Some(ReserveMappingSource::PerVolume(name)),
                (None, None, Some(value)) => Some(ReserveMappingSource::Constant(value)),
                (None, None, None) => None,
                _ => return Err("Give one of column, per_volume and constant.".to_owned()),
            };
            self.mcp_ui(UiCommand::SetReserveMapping {
                block_model: model,
                field,
                source,
            })?;
        }
        if let Some(included) = arguments["included"].as_bool() {
            self.mcp_ui(UiCommand::SetReserveModelIncluded { block_model: model, included })?;
        }
        Ok(json!({ "block_model": model.0 }))
    }

    fn mcp_run_planning(&mut self, arguments: &Value) -> Result<Value, String> {
        let command = match arguments["step"].as_str() {
            None => UiCommand::RunAllPlanningStages,
            Some(name) => UiCommand::RunPlanningStage(
                STEPS
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, step)| *step)
                    .ok_or("step is benching, blasting, dig_strips or reserving.")?,
            ),
        };
        self.mcp_ui(command)?;
        Ok(json!({ "started": arguments["step"].as_str().unwrap_or("all") }))
    }

    fn mcp_reserves(&self, arguments: &Value) -> Result<Value, String> {
        let snapshot = self
            .planning_snapshot()
            .map_err(|not_ready| format!("The reserves are not ready: {}", super::plain(&not_ready.describe())))?;
        let fields = self.mcp_reserve_fields();
        let by = arguments["by"].as_str().unwrap_or("solid");
        let mut groups: BTreeMap<String, Group> = BTreeMap::new();
        let mut rows = Vec::new();
        for block in &snapshot.blocks {
            let totals = match &block.material {
                crate::app::commands::solids_view::MaterialState::Measured(totals) => Some(totals),
                _ => None,
            };
            match by {
                "block" => {
                    let mut row = json!({
                        "solid": block.solid_name,
                        "bench": block.bench.base,
                        "flitch": block.flitch.base,
                        "block": block.name,
                        "plan_area": block.plan_area,
                        "volume": block.volume,
                    });
                    if let Some(totals) = totals {
                        row["reserves"] = totals_json(totals, &fields);
                    }
                    rows.push(row);
                }
                _ => {
                    let key = if by == "bench" {
                        format!("{} | bench {}", block.solid_name, block.bench.base)
                    } else {
                        block.solid_name.clone()
                    };
                    let group = groups.entry(key).or_default();
                    group.blocks += 1;
                    group.volume += block.volume.unwrap_or(0.0);
                    if let Some(totals) = totals {
                        if group.measured {
                            group.totals.merge(totals);
                        } else {
                            group.totals = totals.clone();
                            group.measured = true;
                        }
                    }
                }
            }
        }
        if by == "block" {
            return Ok(json!({ "blocks": rows, "excluded_blocks": snapshot.excluded.len() }));
        }
        let rows: Vec<Value> = groups
            .into_iter()
            .map(|(key, group)| {
                json!({
                    "group": key,
                    "dig_blocks": group.blocks,
                    "volume": group.volume,
                    "reserves": group.measured.then(|| totals_json(&group.totals, &fields)),
                })
            })
            .collect();
        Ok(json!({ "by": by, "rows": rows, "excluded_blocks": snapshot.excluded.len() }))
    }
}

/// Dig blocks added up.
#[derive(Default)]
struct Group {
    blocks: usize,
    volume: f64,
    totals: ReserveTotals,
    /// Whether any block in it was measured, so the first merge seeds the
    /// totals rather than merging into an empty set that drops categories.
    measured: bool,
}

/// One reserve field as the tools describe it.
fn field_json(field: &ReserveField) -> Value {
    let aggregation = match field.aggregation {
        ReserveAggregation::Sum => json!("sum"),
        ReserveAggregation::VolumeAverage => json!("volume_average"),
        ReserveAggregation::Category => json!("category"),
        ReserveAggregation::WeightedAverage { weight_field } => json!({ "weighted_average_by": weight_field.0 }),
    };
    json!({ "id": field.id.0, "name": field.name, "aggregation": aggregation })
}

/// Measured totals, each field's figure by name, and the category fields'
/// breakdowns.
fn totals_json(totals: &ReserveTotals, fields: &[ReserveField]) -> Value {
    let figures = |group: &crate::model::solid_reserves::ReserveGroup| -> Value {
        let mut values = serde_json::Map::new();
        for field in fields {
            if let Some(total) = group.numeric.get(&field.id) {
                values.insert(field.name.clone(), json!(total.value(field.aggregation)));
            }
        }
        values.insert("covered_volume".to_owned(), json!(group.covered_volume));
        Value::Object(values)
    };
    let mut result = figures(&totals.all);
    for field in fields {
        if let Some(categories) = totals.categories.get(&field.id) {
            let breakdown: serde_json::Map<String, Value> = categories.iter().map(|(name, group)| (name.clone(), figures(group))).collect();
            result[format!("by_{}", field.name)] = Value::Object(breakdown);
        }
    }
    result
}
