//! The tools in groups, so a client is not handed all of them at once.
//!
//! Every tool's description goes into the model's context with the tool list,
//! and with them all that is tens of thousands of tokens before any work is
//! done. So `tools/list` gives the core tools (reading the project, looking
//! at it, editing, files and items, undo and checkpoints) and `load_tools`,
//! which adds a group on request and tells the client the list has changed.
//! A tool in a group that is not loaded still answers if it is called by
//! name: the groups only shape what is listed.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::{blast, edit, failed, files, items, jobs, ok, planning, production, read, solids, style, survey, view};
use crate::app::App;

/// The tools listed whatever is loaded.
fn core() -> Vec<Value> {
    let mut tools = read::definitions();
    tools.extend(view::definitions());
    tools.extend(edit::definitions());
    tools.extend(items::definitions());
    tools.extend(files::definitions());
    tools.extend(jobs::definitions().into_iter().filter(|tool| tool["name"] == "get_tasks"));
    tools.push(load_tools_definition());
    tools
}

fn design() -> Vec<Value> {
    let mut tools = super::objects::definitions();
    tools.extend(super::pit_design::definitions());
    tools
}

fn surfaces() -> Vec<Value> {
    jobs::definitions().into_iter().filter(|tool| tool["name"] != "get_tasks").collect()
}

fn planning_tools() -> Vec<Value> {
    let mut tools = solids::definitions();
    tools.extend(planning::definitions());
    tools
}

/// A group of tools `load_tools` adds.
struct Group {
    name: &'static str,
    about: &'static str,
    tools: fn() -> Vec<Value>,
}

const GROUPS: [Group; 7] = [
    Group {
        name: "design",
        about: "drawing and editing design objects: move, offset, batters and berms, explode, insert points, drape; designing a pit with its haul ramp",
        tools: design,
    },
    Group {
        name: "surfaces",
        about: "building and cutting surfaces, contours, solids, point cloud surfaces, block model estimation",
        tools: surfaces,
    },
    Group {
        name: "display",
        about: "colours and styles of surfaces, block models and drillholes, draped rasters, display options",
        tools: style::definitions,
    },
    Group {
        name: "planning",
        about: "the Planning setup's solids and reserves, and pit optimisation scenarios",
        tools: planning_tools,
    },
    Group {
        name: "schedule",
        about: "the production schedule and haul network, its report and video",
        tools: production::definitions,
    },
    Group {
        name: "blasting",
        about: "drill patterns, initiation, tie-ins and charging",
        tools: blast::definitions,
    },
    Group {
        name: "survey",
        about: "coordinate systems, reference surfaces and points, geophysics, rain simulation",
        tools: survey::definitions,
    },
];

fn load_tools_definition() -> Value {
    let groups: Vec<String> = GROUPS.iter().map(|group| format!("{} ({})", group.name, group.about)).collect();
    json!({
        "name": "load_tools",
        "title": "Load more tools",
        "description": format!(
            "Add a group of tools to those listed, for work the ones listed do not cover. The groups: {}. Loads what is asked for and answers with the tools added and their schemas; the tool list then includes them.",
            groups.join("; ")
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "groups": {
                    "type": "array",
                    "items": { "type": "string", "enum": GROUPS.iter().map(|group| group.name).chain(["all"]).collect::<Vec<_>>() },
                    "description": "The groups to load; all loads every group.",
                },
            },
            "required": ["groups"],
            "additionalProperties": false,
        },
        "annotations": { "readOnlyHint": true },
    })
}

/// The tools `tools/list` gives with `loaded` loaded.
///
/// Each is marked to be loaded as it is listed. A client may hold back an
/// MCP server's tools until the model searches for them (Claude Code's tool
/// search does), which would cost a round trip for every tool and undo what
/// `load_tools` is for: the groups already keep the list short, so what is
/// listed is meant to be callable at once.
pub(super) fn listed(loaded: &BTreeSet<&'static str>) -> Vec<Value> {
    let mut tools = core();
    for group in GROUPS.iter().filter(|group| loaded.contains(group.name)) {
        tools.extend((group.tools)());
    }
    for tool in &mut tools {
        tool["_meta"] = json!({ "anthropic/alwaysLoad": true });
    }
    tools
}

impl App<'_> {
    /// The tools as `tools/list` gives them now.
    pub(crate) fn mcp_listed_tools(&self) -> Vec<Value> {
        listed(&self.mcp_loaded_groups)
    }

    /// A client has (re)connected: it starts from the core tools.
    pub(crate) fn reset_mcp_tool_groups(&mut self) {
        self.mcp_loaded_groups.clear();
    }

    pub(super) fn mcp_load_tools(&mut self, arguments: &Value) -> Value {
        let Some(asked) = arguments["groups"].as_array() else {
            return failed("Give the groups to load.");
        };
        let mut wanted = BTreeSet::new();
        for name in asked.iter().filter_map(Value::as_str) {
            if name == "all" {
                wanted.extend(GROUPS.iter().map(|group| group.name));
                continue;
            }
            match GROUPS.iter().find(|group| group.name == name) {
                Some(group) => {
                    wanted.insert(group.name);
                }
                None => return failed(format!("There is no group {name}. They are {}.", GROUPS.map(|group| group.name).join(", "))),
            }
        }
        let added: Vec<&'static str> = wanted.difference(&self.mcp_loaded_groups).copied().collect();
        self.mcp_loaded_groups.extend(added.iter().copied());
        if !added.is_empty() {
            self.mcp_outbox.push(json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
        }
        // Their schemas too, for a client that does not fetch the list again:
        // the tools answer when called by name either way.
        let tools: Vec<Value> = GROUPS
            .iter()
            .filter(|group| added.contains(&group.name))
            .flat_map(|group| (group.tools)())
            .map(|tool| json!({ "name": tool["name"], "description": tool["description"], "inputSchema": tool["inputSchema"] }))
            .collect();
        ok(&json!({
            "loaded": self.mcp_loaded_groups.iter().collect::<Vec<_>>(),
            "added": tools,
        }))
    }
}
