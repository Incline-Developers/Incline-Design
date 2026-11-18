//! The MCP server's life in the window: started and stopped to follow
//! Preferences > MCP, and its requests answered here on the main thread, in
//! JSON-RPC, as they come in (`crate::mcp`). The tools are in
//! [`super::mcp_tools`].
//!
//! The browser cannot listen for connections, so there the server stays off
//! and Preferences says why.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};

use crate::app::App;
#[cfg(not(target_arch = "wasm32"))]
use crate::{mcp, ui::state::McpStatus};

/// The protocol versions this server speaks, newest first.
#[cfg(not(target_arch = "wasm32"))]
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What the server tells a client about itself when it connects.
#[cfg(not(target_arch = "wasm32"))]
const INSTRUCTIONS: &str = "This server is Incline Design, a mine design application, with the user's project open in it. \
Its tools read what the project holds (surfaces, block models, drillholes, point clouds, rasters, design layers and their \
objects, the Planning setup, optimisation and schedules), show and look at it in the viewport, and change it the way the \
window's own controls do: drawing and editing designs, building and cutting surfaces, estimating block models, reserving, \
optimising, scheduling, drill and blast, and importing and exporting files. Ids are the numbers get_project and the other \
get_ tools report. Long work runs in the background and returns once started: get_tasks follows it and can wait for it, and \
get_activity says if it failed. Setups kept as JSON (solids' plans, scenarios, the schedule) are read whole and changed with \
JSON merge patches. Changes are made in the open window and can be undone there; render_view draws a picture of the result.\n\n\
Looking and showing: render_view takes the whole view as arguments and draws it on its own, so looking never moves the \
user's camera or selection. The window is the user's: get_selection reads what they have selected, to work on it, and \
show_in_window is the only tool that changes their view or selection, for when they have asked to be shown something.\n\n\
Working well with large projects: surfaces can run to tens of millions of triangles, and memory is the limit. Every change, \
deleted and replaced items included, is kept for undo until the project is saved; saving clears that history and frees it. \
So save after each heavy step (building, trimming or cutting a surface, deleting big items), and unload what the next step \
does not need with set_loaded, or pass unload_source where a tool offers it; get_project says what is loaded. \
open_project asks the user in the window when the open project has unsaved changes (unless told to discard them), and \
nothing can be saved until they answer, so save first and never reopen the open file to reload it. save_project with a path is Save As: the project then \
lives at that path, which keeps the user's own file untouched while you work.\n\n\
Designing: sample_surface reads the ground's RL at points, to fit a design to it before drawing. A terrain surface is \
single-valued, so its breaklines must not cross in plan at different RLs; when triangulate fails, get_activity names the \
two edges that conflict.";

impl App<'_> {
    /// Hand the event loop to the MCP server to wake it with, and start the
    /// server if it was left on.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn set_event_loop_proxy(&mut self, proxy: winit::event_loop::EventLoopProxy<()>) {
        self.event_loop_proxy = Some(proxy);
        let settings = mcp::load_settings();
        self.editor.mcp.enabled = settings.enabled;
        self.editor.mcp.port = settings.port;
        self.editor.mcp.token = settings.token;
        self.sync_mcp_server();
    }

    /// Start the server, stop it, or move it, to match the editor's
    /// settings.
    #[cfg(not(target_arch = "wasm32"))]
    fn sync_mcp_server(&mut self) {
        let state = &mut self.editor.mcp;
        if !state.enabled {
            self.mcp_server = None;
            state.status = McpStatus::Off;
            return;
        }
        if self.mcp_server.as_ref().is_some_and(|server| server.port() == state.port) {
            return;
        }
        // The old one lets go of its port before the new one takes it.
        self.mcp_server = None;
        let proxy = self.event_loop_proxy.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            if let Some(proxy) = &proxy {
                let _ = proxy.send_event(());
            }
        });
        match mcp::Server::start(state.port, state.token.clone(), wake) {
            Ok(server) => {
                state.status = McpStatus::Running;
                crate::userspace_log!("{}", crate::i18n::tr!("mcp-started", url = mcp::url(server.port())));
                self.mcp_server = Some(server);
            }
            Err(error) => {
                state.status = McpStatus::Failed(error.to_string());
                log::warn!("The MCP server could not start on port {}: {error}", state.port);
            }
        }
        self.redraw_requested = true;
    }

    /// Save the settings as the editor holds them, then bring the server
    /// into line.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_mcp_settings(&mut self) -> anyhow::Result<()> {
        let state = &self.editor.mcp;
        mcp::save_settings(&mcp::McpSettings {
            enabled: state.enabled,
            port: state.port,
            token: state.token.clone(),
        })?;
        self.sync_mcp_server();
        Ok(())
    }

    pub(crate) fn set_mcp_enabled(&mut self, enabled: bool) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let state = &mut self.editor.mcp;
            state.enabled = enabled;
            if state.token.is_empty() {
                state.token = mcp::new_token();
            }
            if state.port == 0 {
                state.port = mcp::DEFAULT_PORT;
            }
            self.save_mcp_settings()
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = enabled;
            Ok(())
        }
    }

    pub(crate) fn set_mcp_port(&mut self, port: u16) -> anyhow::Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if port == 0 || port == self.editor.mcp.port {
                return Ok(());
            }
            self.editor.mcp.port = port;
            self.save_mcp_settings()
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = port;
            Ok(())
        }
    }

    /// Answer the requests waiting for the main thread.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn serve_mcp(&mut self) {
        let calls = self.mcp_server.as_ref().map(mcp::Server::take_calls).unwrap_or_default();
        if calls.is_empty() {
            return;
        }
        for call in calls {
            // A wait for the background work is answered once it is done.
            if call.request["method"] == "tools/call"
                && call.request["params"]["name"] == "get_tasks"
                && call.request["params"]["arguments"]["wait_seconds"].as_f64().is_some_and(|seconds| seconds > 0.0)
            {
                self.begin_mcp_wait(call);
                continue;
            }
            let response = self.answer_mcp(&call.request);
            call.reply(response);
        }
        self.redraw_requested = true;
    }

    /// The JSON-RPC response to `request`.
    #[cfg(not(target_arch = "wasm32"))]
    fn answer_mcp(&mut self, request: &Value) -> Value {
        let id = request["id"].clone();
        let params = &request["params"];
        let result = match request["method"].as_str().unwrap_or_default() {
            "initialize" => Ok(initialize(params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": super::mcp_tools::definitions() })),
            "tools/call" => Ok(self.call_mcp_tool(params)),
            "resources/list" => Ok(json!({ "resources": [] })),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            method => Err(json!({ "code": -32601, "message": format!("Method not found: {method}") })),
        };
        match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        }
    }
}

/// The answer to `initialize`: the protocol version the client asked for if
/// this server speaks it, else the newest it does.
#[cfg(not(target_arch = "wasm32"))]
fn initialize(params: &Value) -> Value {
    let asked = params["protocolVersion"].as_str().unwrap_or_default();
    let version = PROTOCOL_VERSIONS.iter().find(|version| **version == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "incline-design", "title": crate::APP_NAME, "version": crate::APP_RELEASE },
        "instructions": INSTRUCTIONS,
    })
}
