//! The tools for drill and blast: laying out a drill pattern, the holes a
//! round starts at, the connectors tying it in, what each hole is charged
//! with, and the libraries of charge products, charge rules and delays.

use std::collections::HashSet;

use glam::DVec2;
use serde_json::{Value, json};

use super::{failed, hex, ok, parse_hex, parse_point};
use crate::{
    app::App,
    model::{
        Command, ObjectId,
        blast::{ChargeProduct, ChargeRule},
        drill_hole::{DrillHoleId, DrillHoleRef, DrillPatternLayout, MAX_PATTERN_HOLES, OpenDrillHoleDataset, TieIn, generate_pattern_collars},
    },
    ui::state::{BlastLibraryItem, UiCommand},
};

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
    let collection = json!({ "type": "integer", "minimum": 0, "description": "The drillhole collection's id." });
    vec![
        tool(
            "create_drill_pattern",
            "Lay out a drill pattern",
            "Make a new drillhole collection of blast holes, as Create Drill Pattern does: a grid of collars filling a closed polyline at burden (between rows) by spacing (along a row), square or staggered, turned by rotation degrees anticlockwise from east; or the collars given. Each hole is drilled straight down to depth with diameter (in metres). Returns its id.",
            json!({
                "name": { "type": "string", "minLength": 1 },
                "boundary": { "type": "integer", "minimum": 0, "description": "A closed design polyline to fill." },
                "burden": { "type": "number", "exclusiveMinimum": 0 },
                "spacing": { "type": "number", "exclusiveMinimum": 0 },
                "rotation": { "type": "number", "description": "Degrees. 0 if left out." },
                "staggered": { "type": "boolean", "description": "Stagger alternate rows by half a spacing. False if left out." },
                "collars": { "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3 }, "description": "[x, y, z] collars, in place of boundary." },
                "depth": { "type": "number", "exclusiveMinimum": 0 },
                "diameter": { "type": "number", "exclusiveMinimum": 0, "description": "Metres, e.g. 0.165." },
            }),
            &["name", "depth", "diameter"],
            false,
        ),
        tool(
            "get_blast",
            "Get a blast's setup",
            "Read how a drillhole collection is set to fire: the holes it starts at with their delays, the connectors tying holes together (delay, product), and what each hole is charged with. Also the charge product and charge rule libraries and the delay products tie-ins are laid with.",
            json!({ "drillholes": collection }),
            &["drillholes"],
            true,
        ),
        tool(
            "set_initiation",
            "Set where a round starts",
            "Make a hole a starting point of its round, firing delay_ms after the shot, or stop it being one (delay_ms null). One undo step.",
            json!({ "drillholes": collection, "hole": { "type": "string", "description": "The hole's id." }, "delay_ms": { "type": ["integer", "null"], "minimum": 0 } }),
            &["drillholes", "hole", "delay_ms"],
            false,
        ),
        tool(
            "set_tie_ins",
            "Tie in a blast",
            "Replace a drillhole collection's connectors: each joins two holes by their ids with a delay in milliseconds, laid with a product (a name, and a #rrggbb colour it is drawn in). One undo step.",
            json!({
                "drillholes": collection,
                "ties": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "from": { "type": "string" },
                            "to": { "type": "string" },
                            "delay_ms": { "type": "integer", "minimum": 0 },
                            "product": { "type": "string" },
                            "colour": { "type": "string" },
                        },
                        "required": ["from", "to", "delay_ms"],
                    },
                },
            }),
            &["drillholes", "ties"],
            false,
        ),
        tool(
            "charge_holes",
            "Charge holes",
            "Load holes by a charge rule from the library, or unload them (rule null), as Charge does. One undo step.",
            json!({
                "drillholes": collection,
                "holes": { "type": "array", "items": { "type": "string" }, "description": "Hole ids. Every hole if left out." },
                "rule": { "type": ["string", "null"], "description": "A charge rule's name, from get_blast." },
            }),
            &["drillholes", "rule"],
            false,
        ),
        tool(
            "set_blast_library",
            "Edit the blast libraries",
            "Add or replace charge products and charge rules, in the shape get_blast shows them (matched by name; rename gives the old name to replace), delete them, or add and delete delay products. reload recharges holes already loaded by a rule changed.",
            json!({
                "products": { "type": "array", "items": { "type": "object" } },
                "rules": { "type": "array", "items": { "type": "object" } },
                "rename": { "type": "object", "additionalProperties": { "type": "string" }, "description": "New name to the old name it replaces." },
                "reload": { "type": "boolean" },
                "delete_products": { "type": "array", "items": { "type": "string" } },
                "delete_rules": { "type": "array", "items": { "type": "string" } },
                "add_delays": {
                    "type": "array",
                    "items": { "type": "object", "properties": { "name": { "type": "string" }, "delay_ms": { "type": "integer", "minimum": 0 }, "colour": { "type": "string" } }, "required": ["name", "delay_ms"] },
                },
                "delete_delays": { "type": "array", "items": { "type": "string" }, "description": "Delay products by name." },
            }),
            &[],
            false,
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_blast_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let result = match name {
            "create_drill_pattern" => self.mcp_create_drill_pattern(arguments),
            "get_blast" => self.mcp_get_blast(arguments),
            "set_initiation" => self.mcp_set_initiation(arguments),
            "set_tie_ins" => self.mcp_set_tie_ins(arguments),
            "charge_holes" => self.mcp_charge_holes(arguments),
            "set_blast_library" => self.mcp_set_blast_library(arguments),
            _ => return None,
        };
        Some(result.map_or_else(failed, |value| ok(&value)))
    }

    fn mcp_run_command(&mut self, command: UiCommand) -> Result<(), String> {
        self.handle_reported_ui_command(command).map_err(|error| format!("{error:#}"))
    }

    /// The loaded drillhole collection `arguments["drillholes"]` names.
    fn mcp_blast_collection(&self, arguments: &Value) -> Result<&OpenDrillHoleDataset, String> {
        let id = arguments["drillholes"].as_u64().ok_or("Give the drillhole collection's id.")?;
        let collection = self
            .drill_holes
            .iter()
            .find(|collection| collection.id.0 == id)
            .ok_or_else(|| format!("There is no drillhole collection with id {id}."))?;
        if !collection.state.loaded || collection.state.deferred.is_some() {
            return Err(format!("The drillholes {} are not loaded. Load them with set_loaded first.", collection.name));
        }
        Ok(collection)
    }

    fn mcp_create_drill_pattern(&mut self, arguments: &Value) -> Result<Value, String> {
        let name = arguments["name"]
            .as_str()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or("Give the pattern a name.")?;
        let (Some(depth), Some(diameter)) = (arguments["depth"].as_f64(), arguments["diameter"].as_f64()) else {
            return Err("Give depth and diameter.".to_owned());
        };
        let collars = match (arguments["boundary"].as_u64(), arguments["collars"].as_array()) {
            (Some(id), None) => {
                let document = self.workspace.active_document().ok_or("No project is open.")?;
                let object = document.get_object(ObjectId(id)).ok_or_else(|| format!("There is no design object with id {id}."))?;
                let Some((boundary, true)) = object.tessellated_path() else {
                    return Err("The boundary must be a closed polyline or a circle.".to_owned());
                };
                let (Some(burden), Some(spacing)) = (arguments["burden"].as_f64(), arguments["spacing"].as_f64()) else {
                    return Err("A pattern from a boundary needs burden and spacing.".to_owned());
                };
                let layout = if arguments["staggered"].as_bool().unwrap_or(false) {
                    DrillPatternLayout::Staggered
                } else {
                    DrillPatternLayout::Square
                };
                generate_pattern_collars(
                    &boundary,
                    burden,
                    spacing,
                    arguments["rotation"].as_f64().unwrap_or(0.0),
                    DVec2::ZERO,
                    layout,
                    MAX_PATTERN_HOLES,
                )
                .map_err(|error| super::plain(&error))?
            }
            (None, Some(collars)) => collars
                .iter()
                .map(|collar| parse_point(collar).ok_or("Collars are [x, y, z]."))
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err("Give a boundary to fill, or the collars.".to_owned()),
        };
        let before: HashSet<DrillHoleId> = self.drill_holes.iter().map(|collection| collection.id).collect();
        let holes = collars.len();
        self.mcp_run_command(UiCommand::CreateDrillPattern {
            name: name.to_owned(),
            collars,
            depth,
            diameter,
        })?;
        let new = self
            .drill_holes
            .iter()
            .find(|collection| !before.contains(&collection.id))
            .map(|collection| collection.id.0);
        Ok(json!({ "id": new, "holes": holes }))
    }

    fn mcp_get_blast(&self, arguments: &Value) -> Result<Value, String> {
        let collection = self.mcp_blast_collection(arguments)?;
        let dataset = &collection.dataset;
        let hole = |index: usize| dataset.holes.get(index).map(|hole| hole.dhid.clone());
        let library = &self.editor.blast_library;
        Ok(json!({
            "initiations": dataset.initiations.iter().map(|start| json!({ "hole": hole(start.hole), "delay_ms": start.delay_ms })).collect::<Vec<_>>(),
            "ties": dataset.ties.iter().map(|tie| json!({
                "from": hole(tie.a), "to": hole(tie.b), "delay_ms": tie.delay_ms, "product": tie.product,
                "colour": hex([tie.color[0], tie.color[1], tie.color[2], 1.0]),
            })).collect::<Vec<_>>(),
            "charges": dataset.charges.iter().map(|(index, charge)| json!({ "hole": hole(*index), "charge": serde_json::to_value(charge).unwrap_or_default() })).collect::<Vec<_>>(),
            "charge_products": serde_json::to_value(&library.products).unwrap_or_default(),
            "charge_rules": serde_json::to_value(&library.rules).unwrap_or_default(),
            "delay_products": self.editor.delay_products.iter().map(|product| json!({
                "name": product.name, "delay_ms": product.delay_ms,
                "colour": format!("#{:02x}{:02x}{:02x}", product.color.r(), product.color.g(), product.color.b()),
            })).collect::<Vec<_>>(),
        }))
    }

    /// A hole's index in `collection`, by its id.
    fn mcp_hole(collection: &OpenDrillHoleDataset, name: &str) -> Result<usize, String> {
        collection
            .dataset
            .holes
            .iter()
            .position(|hole| hole.dhid == name)
            .ok_or_else(|| format!("{} has no hole {name}.", collection.name))
    }

    fn mcp_set_initiation(&mut self, arguments: &Value) -> Result<Value, String> {
        let collection = self.mcp_blast_collection(arguments)?;
        let hole = Self::mcp_hole(collection, arguments["hole"].as_str().ok_or("Give the hole's id.")?)?;
        let delay = arguments["delay_ms"].as_u64().map(|delay| u32::try_from(delay).unwrap_or(u32::MAX));
        let target = DrillHoleRef { dataset: collection.id, hole };
        self.mcp_run_command(UiCommand::SetInitiation { target, delay_ms: delay })?;
        Ok(json!({ "hole": arguments["hole"], "delay_ms": delay }))
    }

    fn mcp_set_tie_ins(&mut self, arguments: &Value) -> Result<Value, String> {
        let collection = self.mcp_blast_collection(arguments)?;
        let ties = arguments["ties"].as_array().ok_or("Give the ties.")?;
        let default_product = self.editor.delay_products.first();
        let after = ties
            .iter()
            .map(|tie| {
                let a = Self::mcp_hole(collection, tie["from"].as_str().ok_or("Each tie needs from.")?)?;
                let b = Self::mcp_hole(collection, tie["to"].as_str().ok_or("Each tie needs to.")?)?;
                if a == b {
                    return Err("A tie joins two different holes.".to_owned());
                }
                let delay_ms = u32::try_from(tie["delay_ms"].as_u64().ok_or("Each tie needs delay_ms.")?).unwrap_or(u32::MAX);
                let product = tie["product"]
                    .as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| default_product.map(|product| product.name.clone()))
                    .unwrap_or_default();
                let color = match tie["colour"].as_str() {
                    Some(text) => parse_hex(text).map(|[r, g, b, _]| [r, g, b]).ok_or("Tie colours are #rrggbb.")?,
                    None => default_product.map_or([1.0, 1.0, 0.0], |product| {
                        let [r, g, b, _] = egui::Rgba::from(product.color).to_array();
                        [r, g, b]
                    }),
                };
                Ok(TieIn { a, b, delay_ms, product, color })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (dataset, before) = (collection.id, collection.dataset.ties.clone());
        let count = after.len();
        if before != after {
            self.execute_job_edit(Command::SetTieIns { dataset, before, after });
            self.invalidate_geometry();
        }
        Ok(json!({ "ties": count }))
    }

    fn mcp_charge_holes(&mut self, arguments: &Value) -> Result<Value, String> {
        let collection = self.mcp_blast_collection(arguments)?;
        let holes: Vec<usize> = match arguments["holes"].as_array() {
            None => (0..collection.dataset.holes.len()).collect(),
            Some(names) => names
                .iter()
                .map(|name| Self::mcp_hole(collection, name.as_str().unwrap_or_default()))
                .collect::<Result<_, _>>()?,
        };
        let rule = arguments["rule"].as_str().map(ToOwned::to_owned);
        if let Some(name) = &rule
            && !self.editor.blast_library.rules.iter().any(|candidate| candidate.name == *name)
        {
            return Err(format!("There is no charge rule {name}. get_blast lists them."));
        }
        let dataset = collection.id;
        self.mcp_run_command(UiCommand::ChargeHoles {
            dataset,
            holes: holes.clone(),
            rule: rule.clone(),
        })?;
        Ok(json!({ "holes": holes.len(), "rule": rule }))
    }

    fn mcp_set_blast_library(&mut self, arguments: &Value) -> Result<Value, String> {
        let rename = arguments["rename"].as_object();
        let original = |name: &str, existing: &[String]| -> Option<String> {
            rename
                .and_then(|rename| rename.get(name))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or_else(|| existing.iter().any(|candidate| candidate == name).then(|| name.to_owned()))
        };
        let mut done = Vec::new();
        for product in arguments["products"].as_array().into_iter().flatten() {
            let product: ChargeProduct = serde_json::from_value(product.clone()).map_err(|error| format!("A product is not in the shape get_blast shows: {error}"))?;
            let existing: Vec<String> = self.editor.blast_library.products.iter().map(|product| product.name.clone()).collect();
            let original = original(&product.name, &existing);
            done.push(format!("product {}", product.name));
            self.mcp_run_command(UiCommand::SaveChargeProduct { original, product })?;
        }
        let reload = arguments["reload"].as_bool().unwrap_or(false);
        for rule in arguments["rules"].as_array().into_iter().flatten() {
            let rule: ChargeRule = serde_json::from_value(rule.clone()).map_err(|error| format!("A rule is not in the shape get_blast shows: {error}"))?;
            let existing: Vec<String> = self.editor.blast_library.rules.iter().map(|rule| rule.name.clone()).collect();
            let original = original(&rule.name, &existing);
            done.push(format!("rule {}", rule.name));
            self.mcp_run_command(UiCommand::SaveChargeRule { original, rule, reload })?;
        }
        for name in arguments["delete_products"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            self.mcp_run_command(UiCommand::DeleteBlastLibraryItem(BlastLibraryItem::Product(name.to_owned())))?;
            done.push(format!("deleted product {name}"));
        }
        for name in arguments["delete_rules"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            self.mcp_run_command(UiCommand::DeleteBlastLibraryItem(BlastLibraryItem::Rule(name.to_owned())))?;
            done.push(format!("deleted rule {name}"));
        }
        for delay in arguments["add_delays"].as_array().into_iter().flatten() {
            let name = delay["name"].as_str().ok_or("Each delay product needs a name.")?.to_owned();
            let delay_ms = u32::try_from(delay["delay_ms"].as_u64().ok_or("Each delay product needs delay_ms.")?).unwrap_or(u32::MAX);
            let [r, g, b, _] = match delay["colour"].as_str() {
                Some(text) => parse_hex(text).ok_or("Delay colours are #rrggbb.")?,
                None => [1.0, 1.0, 0.0, 1.0],
            };
            let color: egui::Color32 = egui::Rgba::from_rgb(r, g, b).into();
            done.push(format!("delay {name}"));
            self.mcp_run_command(UiCommand::AddDelayProduct { delay_ms, name, color })?;
        }
        for name in arguments["delete_delays"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            let id = self
                .editor
                .delay_products
                .iter()
                .find(|product| product.name == name)
                .map(|product| product.id)
                .ok_or_else(|| format!("There is no delay product {name}."))?;
            self.mcp_run_command(UiCommand::DeleteDelayProduct(id))?;
            done.push(format!("deleted delay {name}"));
        }
        Ok(json!({ "done": done }))
    }
}
