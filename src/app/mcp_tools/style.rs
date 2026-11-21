//! The tools that change how things are drawn: surface colours, what a block
//! model or drillholes are coloured by and how, draping rasters, the
//! viewport's display options, and section view.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{failed, ok, parse_hex, parse_point};
use crate::{
    app::App,
    model::{
        block_model::{BlockModelSlice, Boundary, ColorTransferFunction},
        drill_hole::{DrillCategoryColor, DrillColorPreset, DrillHoleStyle, WorkingSection},
    },
    ui::state::UiCommand,
};

/// A colour's RGB alone, for the drillhole colours that carry no alpha.
fn rgb(text: &str) -> Option<[f32; 3]> {
    parse_hex(text).map(|[r, g, b, _]| [r, g, b])
}

/// A tool that only changes how things look.
fn looks(name: &str, title: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true },
    })
}

pub(super) fn definitions() -> Vec<Value> {
    let colour = json!({ "type": "string", "description": "#rrggbb" });
    let colours = json!({ "type": "array", "items": { "type": "string" }, "description": "#rrggbb colours." });
    vec![
        looks(
            "set_surface_colour",
            "Colour a surface",
            "Set the colour a surface is drawn in. One undo step.",
            json!({ "id": { "type": "integer", "minimum": 0 }, "colour": colour }),
            &["id", "colour"],
        ),
        looks(
            "colour_block_model",
            "Colour a block model",
            "Choose the variable a block model is coloured by, and optionally its colours, as the legend does. For a numeric variable give bands (each band's lowest value and colour, ascending; blocks below the first band are hidden unless below_colour is given) or a continuous ramp (min, max and evenly spread colours). For a categorical one give category colours by name. reset rebuilds the default colours from the data. Give slice to show only the blocks inside a box, or slice null to show them all.",
            json!({
                "id": { "type": "integer", "minimum": 0 },
                "variable": { "type": "string" },
                "bands": { "type": "array", "items": { "type": "object", "properties": { "from": { "type": "number" }, "colour": colour }, "required": ["from", "colour"] } },
                "below_colour": { "type": "string", "description": "#rrggbb for blocks below the first band. Hidden if left out." },
                "continuous": { "type": "object", "properties": { "min": { "type": "number" }, "max": { "type": "number" }, "colours": colours }, "required": ["min", "max", "colours"] },
                "categories": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Category name to #rrggbb." },
                "reset": { "type": "boolean" },
                "slice": {
                    "type": ["object", "null"],
                    "properties": { "min": { "type": "array", "items": { "type": "number" } }, "max": { "type": "array", "items": { "type": "number" } } },
                    "description": "{min: [x, y, z], max: [x, y, z]} in world coordinates, or null for the whole model.",
                },
            }),
            &["id"],
        ),
        looks(
            "style_drillholes",
            "Style drillholes",
            "Choose how a drillhole collection is drawn and coloured, as its colour dialog does. Only what is given changes. colour_by is a field key from get_drillholes, or null for plain white. A numeric field takes a preset ramp; a categorical one takes colours by category. Working sections group a categorical field's codes to colour (and mine) together.",
            json!({
                "id": { "type": "integer", "minimum": 0 },
                "colour_by": { "type": ["string", "null"] },
                "preset": { "type": "string", "enum": ["rainbow", "greyscale", "heat", "green_yellow_red"] },
                "categories": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Category to #rrggbb." },
                "style": { "type": "string", "enum": ["true_diameter", "string_and_discs"] },
                "width_scale": { "type": "number", "exclusiveMinimum": 0, "description": "true_diameter: multiplies the drilled diameter." },
                "min_pixels": { "type": "number", "exclusiveMinimum": 0, "description": "true_diameter: narrowest a hole is drawn, in pixels." },
                "disc_diameter": { "type": "number", "exclusiveMinimum": 0, "description": "string_and_discs: disc diameter in metres." },
                "string_pixels": { "type": "number", "exclusiveMinimum": 0, "description": "string_and_discs: string width in pixels." },
                "working_sections": {
                    "type": "array",
                    "items": { "type": "object", "properties": { "name": { "type": "string" }, "field": { "type": "string" }, "codes": { "type": "array", "items": { "type": "string" } } }, "required": ["name", "field", "codes"] },
                    "description": "Replaces the collection's working sections.",
                },
                "colour_by_working_section": { "type": "string", "description": "A categorical field key: colour it by working section." },
            }),
            &["id"],
        ),
        looks(
            "drape_raster",
            "Drape a raster",
            "Drape a raster (an image such as an aerial photo) over every surface it overlaps, or take it off them.",
            json!({ "id": { "type": "integer", "minimum": 0 }, "draped": { "type": "boolean" } }),
            &["id", "draped"],
        ),
        looks(
            "set_display",
            "Set display options",
            "Turn the viewport's display options on or off. Only what is given changes.",
            json!({
                "grid": { "type": "boolean", "description": "The coordinate grid (the RL grid in a section)." },
                "wireframes": { "type": "boolean", "description": "Triangle edges on surfaces." },
                "contours": { "type": "boolean", "description": "Contour lines on surfaces." },
                "points": { "type": "boolean", "description": "A dot at every vertex of polylines and pit designs." },
                "cinematic": { "type": "boolean", "description": "Presentation shading." },
            }),
            &[],
        ),
    ]
}

impl App<'_> {
    /// Run `name` if it is one of these tools.
    pub(super) fn call_mcp_style_tool(&mut self, name: &str, arguments: &Value) -> Option<Value> {
        let commands = match name {
            "set_surface_colour" => self.mcp_surface_colour(arguments),
            "colour_block_model" => self.mcp_colour_block_model(arguments),
            "style_drillholes" => self.mcp_style_drillholes(arguments),
            "drape_raster" => self.mcp_drape_raster(arguments),
            "set_display" => Ok(display(arguments)),
            _ => return None,
        };
        let commands = match commands {
            Ok(commands) => commands,
            Err(error) => return Some(failed(error)),
        };
        for command in commands {
            if let Err(error) = self.handle_reported_ui_command(command) {
                return Some(failed(format!("{error:#}")));
            }
        }
        Some(ok(&json!({ "done": name })))
    }

    fn mcp_surface_colour(&self, arguments: &Value) -> Result<Vec<UiCommand>, String> {
        let id = arguments["id"].as_u64().ok_or("Give the surface's id.")?;
        let surface = self
            .triangulations
            .iter()
            .find(|surface| surface.id.0 == id)
            .ok_or_else(|| format!("There is no surface with id {id}."))?;
        let colour = arguments["colour"].as_str().and_then(parse_hex).ok_or("colour must be #rrggbb.")?;
        Ok(vec![UiCommand::SetTriangulationColor(surface.id, colour)])
    }

    fn mcp_colour_block_model(&self, arguments: &Value) -> Result<Vec<UiCommand>, String> {
        let id = arguments["id"].as_u64().ok_or("Give the block model's id.")?;
        let model = self
            .block_models
            .iter()
            .find(|model| model.id.0 == id)
            .ok_or_else(|| format!("There is no block model with id {id}."))?;
        let mut commands = Vec::new();
        let variable_name = arguments["variable"].as_str().map(ToOwned::to_owned).or_else(|| model.active_color_variable.clone());
        if let Some(name) = arguments["variable"].as_str() {
            if model.model.variable(name).is_none() {
                return Err(format!("The block model has no variable {name}. get_block_model lists them."));
            }
            commands.push(UiCommand::SetBlockModelColorVariable {
                id: model.id,
                variable: name.to_owned(),
            });
        }
        let colours = |value: &Value| -> Result<Vec<[f32; 4]>, String> {
            value
                .as_array()
                .ok_or("colours must be a list.")?
                .iter()
                .map(|colour| colour.as_str().and_then(parse_hex).ok_or_else(|| "Colours are #rrggbb.".to_owned()))
                .collect()
        };
        let transfer = if let Some(bands) = arguments["bands"].as_array() {
            let mut bands = bands
                .iter()
                .map(|band| {
                    Ok((
                        band["from"].as_f64().ok_or("Each band needs from.")?,
                        band["colour"].as_str().and_then(parse_hex).ok_or("Each band needs a #rrggbb colour.")?,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if bands.is_empty() {
                return Err("Give at least one band.".to_owned());
            }
            bands.sort_by(|a, b| a.0.total_cmp(&b.0));
            let below = match arguments["below_colour"].as_str() {
                Some(text) => parse_hex(text).ok_or("below_colour must be #rrggbb.")?,
                None => [0.0; 4],
            };
            Some(ColorTransferFunction::Discrete {
                boundaries: bands
                    .iter()
                    .enumerate()
                    .map(|(index, (from, _))| Boundary {
                        id: index as u64 + 1,
                        value: *from,
                        inclusive: false,
                    })
                    .collect(),
                gradient: std::iter::once(below).chain(bands.into_iter().map(|(_, colour)| colour)).collect(),
            })
        } else if arguments["continuous"].is_object() {
            let continuous = &arguments["continuous"];
            let (Some(min), Some(max)) = (continuous["min"].as_f64(), continuous["max"].as_f64()) else {
                return Err("continuous needs min and max.".to_owned());
            };
            let gradient = colours(&continuous["colours"])?;
            if gradient.len() < 2 || min >= max {
                return Err("continuous needs min below max and at least two colours.".to_owned());
            }
            Some(ColorTransferFunction::Continuous { range: (min, max), gradient })
        } else if let Some(categories) = arguments["categories"].as_object() {
            let name = variable_name.as_deref().ok_or("Choose the variable to colour by.")?;
            let variable = model.model.variable(name).ok_or_else(|| format!("There is no variable {name}."))?;
            let mut gradient: BTreeMap<u32, [f32; 4]> = match model.color_transfers.get(name) {
                Some(ColorTransferFunction::Category { gradient }) => gradient.clone(),
                _ => BTreeMap::new(),
            };
            for (label, colour) in categories {
                let code = variable
                    .strings
                    .iter()
                    .find(|(_, text)| *text == label)
                    .map(|(code, _)| *code)
                    .ok_or_else(|| format!("{name} has no category {label}."))?;
                gradient.insert(code, colour.as_str().and_then(parse_hex).ok_or("Category colours are #rrggbb.")?);
            }
            Some(ColorTransferFunction::Category { gradient })
        } else {
            None
        };
        if let Some(transfer) = transfer {
            commands.push(UiCommand::SetBlockModelColorTransfer { id: model.id, transfer });
        } else if arguments["reset"].as_bool().unwrap_or(false) {
            commands.push(UiCommand::ResetBlockModelColorTransfer { id: model.id });
        }
        match &arguments["slice"] {
            Value::Null if arguments.get("slice").is_some() => commands.push(UiCommand::SetBlockModelSlice { id: model.id, slice: None }),
            Value::Object(slice) => {
                let (Some(min), Some(max)) = (slice.get("min").and_then(parse_point), slice.get("max").and_then(parse_point)) else {
                    return Err("slice needs min and max as [x, y, z].".to_owned());
                };
                // Given in world coordinates, kept in the model's own.
                let (min, max) = model.world_box_to_local(min.min(max), min.max(max));
                commands.push(UiCommand::SetBlockModelSlice {
                    id: model.id,
                    slice: Some(BlockModelSlice { min, max }),
                });
            }
            _ => {}
        }
        Ok(commands)
    }

    fn mcp_style_drillholes(&self, arguments: &Value) -> Result<Vec<UiCommand>, String> {
        let id = arguments["id"].as_u64().ok_or("Give the collection's id.")?;
        let collection = self
            .drill_holes
            .iter()
            .find(|collection| collection.id.0 == id)
            .ok_or_else(|| format!("There is no drillhole collection with id {id}."))?;
        let id = collection.id;
        let state = &collection.color;
        let field_exists = |key: &str| collection.dataset.fields.iter().any(|field| field.key == key);
        let mut commands = Vec::new();
        match &arguments["colour_by"] {
            Value::Null if arguments.get("colour_by").is_some() => commands.push(UiCommand::SetDrillHoleColorField { id, field: None }),
            Value::String(field) if field_exists(field) => commands.push(UiCommand::SetDrillHoleColorField { id, field: Some(field.clone()) }),
            Value::String(field) => return Err(format!("The drillholes have no field {field}. get_drillholes lists them.")),
            _ => {}
        }
        if let Some(preset) = arguments["preset"].as_str() {
            let preset = match preset {
                "rainbow" => DrillColorPreset::Rainbow,
                "greyscale" => DrillColorPreset::Grayscale,
                "heat" => DrillColorPreset::Heat,
                "green_yellow_red" => DrillColorPreset::GreenYellowRed,
                _ => return Err("preset is rainbow, greyscale, heat or green_yellow_red.".to_owned()),
            };
            commands.push(UiCommand::SetDrillHoleColorPreset { id, preset });
        }
        if let Some(categories) = arguments["categories"].as_object() {
            let categories = categories
                .iter()
                .map(|(value, colour)| {
                    Ok(DrillCategoryColor {
                        value: value.clone(),
                        color: colour.as_str().and_then(rgb).ok_or("Category colours are #rrggbb.")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            commands.push(UiCommand::SetDrillHoleCategoryColors { id, categories });
        }
        if let Some(style) = arguments["style"].as_str() {
            let style = match style {
                "true_diameter" => DrillHoleStyle::TrueDiameter,
                "string_and_discs" => DrillHoleStyle::StringAndDiscs,
                _ => return Err("style is true_diameter or string_and_discs.".to_owned()),
            };
            commands.push(UiCommand::SetDrillHoleStyle { id, style });
        }
        if arguments["width_scale"].is_number() || arguments["min_pixels"].is_number() {
            commands.push(UiCommand::SetDrillHoleWidth {
                id,
                radius_scale: arguments["width_scale"].as_f64().unwrap_or(state.radius_scale),
                min_pixel_diameter: arguments["min_pixels"].as_f64().map_or(state.min_pixel_diameter, |pixels| pixels as f32),
            });
        }
        if arguments["disc_diameter"].is_number() || arguments["string_pixels"].is_number() {
            commands.push(UiCommand::SetDrillHoleDiscs {
                id,
                disc_diameter: arguments["disc_diameter"].as_f64().unwrap_or(state.disc_diameter),
                string_pixel_width: arguments["string_pixels"].as_f64().map_or(state.string_pixel_width, |pixels| pixels as f32),
            });
        }
        if let Some(sections) = arguments["working_sections"].as_array() {
            let sections = sections
                .iter()
                .map(|section| {
                    Ok(WorkingSection {
                        name: section["name"].as_str().ok_or("Each working section needs a name.")?.to_owned(),
                        field: section["field"]
                            .as_str()
                            .filter(|field| field_exists(field))
                            .ok_or("Each working section needs a field the drillholes have.")?
                            .to_owned(),
                        codes: section["codes"]
                            .as_array()
                            .ok_or("Each working section needs its codes.")?
                            .iter()
                            .filter_map(Value::as_str)
                            .map(ToOwned::to_owned)
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            commands.push(UiCommand::SetDrillHoleWorkingSections { id, sections });
        }
        if let Some(field) = arguments["colour_by_working_section"].as_str() {
            if !field_exists(field) {
                return Err(format!("The drillholes have no field {field}."));
            }
            commands.push(UiCommand::SetDrillHoleColorByWorkingSection { id, field: field.to_owned() });
        }
        Ok(commands)
    }

    fn mcp_drape_raster(&self, arguments: &Value) -> Result<Vec<UiCommand>, String> {
        let id = arguments["id"].as_u64().ok_or("Give the raster's id.")?;
        let raster = self
            .raster_textures
            .iter()
            .find(|raster| raster.id.0 == id)
            .ok_or_else(|| format!("There is no raster with id {id}."))?;
        let draped = arguments["draped"].as_bool().ok_or("Give draped.")?;
        Ok(vec![if draped {
            UiCommand::DrapeRaster(raster.id)
        } else {
            UiCommand::UndrapeRaster(raster.id)
        }])
    }
}

/// The display option commands `arguments` asks for.
fn display(arguments: &Value) -> Vec<UiCommand> {
    let mut commands = Vec::new();
    if let Some(on) = arguments["grid"].as_bool() {
        commands.push(UiCommand::SetGridShown(on));
    }
    if let Some(on) = arguments["wireframes"].as_bool() {
        commands.push(UiCommand::SetTopologyWireframes(on));
    }
    if let Some(on) = arguments["contours"].as_bool() {
        commands.push(UiCommand::SetTopologyContours(on));
    }
    if let Some(on) = arguments["points"].as_bool() {
        commands.push(UiCommand::SetShowPoints(on));
    }
    if let Some(on) = arguments["cinematic"].as_bool() {
        commands.push(UiCommand::SetCinematicEnabled(on));
    }
    commands
}
