//! Incline's MCP server: lets an AI tool on this computer (Claude Code,
//! Claude Desktop, or any other MCP client) see and work with the open
//! project.
//!
//! The server speaks MCP's Streamable HTTP transport at
//! `http://127.0.0.1:<port>/mcp`. It listens on the loopback address only,
//! asks every request for the bearer token shown in Preferences > MCP, and
//! turns away requests a web page sends (an `Origin` that is not this
//! computer), so nothing but a program the user set up on this computer can
//! reach it.
//!
//! Connections are served on threads of their own ([`http`]), but every
//! JSON-RPC request is handed to the main thread as a [`Call`] and answered
//! there (`app/mcp.rs`), where the project lives: a tool sees exactly what
//! the window shows, and changes it the way the window's own commands do.
//!
//! A client that can only start a server as a program and talk to it over
//! its standard input and output (Claude Desktop's local servers) runs
//! `incline-design mcp`, which relays to the running window ([`stdio`]).

mod http;
mod server;
pub(crate) mod stdio;

use std::io;

use serde::{Deserialize, Serialize};
pub(crate) use server::Server;

/// The port the server listens on unless the user picks another.
pub(crate) const DEFAULT_PORT: u16 = 2608;
/// The path the server answers on.
pub(crate) const PATH: &str = "/mcp";
/// Where the settings are kept, beside the preferences.
const SETTINGS_FILE: &str = "mcp.toml";

/// The server's saved settings, in a file of their own beside the
/// preferences, readable by the user alone since it holds the token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct McpSettings {
    pub(crate) enabled: bool,
    pub(crate) port: u16,
    /// What a client must send as `Authorization: Bearer <token>`. Made
    /// the first time the server starts.
    pub(crate) token: String,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            token: String::new(),
        }
    }
}

/// A fresh token: two random UUIDs' worth of hex, too long to guess.
pub(crate) fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}

/// The address a client connects to.
pub(crate) fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}{PATH}")
}

/// The saved settings, or the defaults if there are none.
pub(crate) fn load_settings() -> McpSettings {
    let read = crate::app::io::data_path(SETTINGS_FILE).and_then(std::fs::read_to_string);
    match read {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|error| {
            log::warn!("The MCP settings could not be read: {error}");
            McpSettings::default()
        }),
        Err(error) => {
            if error.kind() != io::ErrorKind::NotFound {
                log::warn!("The MCP settings could not be read: {error}");
            }
            McpSettings::default()
        }
    }
}

/// Save the settings, made readable by their owner alone before anything is
/// written, as they hold the token.
pub(crate) fn save_settings(settings: &McpSettings) -> io::Result<()> {
    use std::io::Write;

    let path = crate::app::io::data_path(SETTINGS_FILE)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(settings).map_err(io::Error::other)?;
    crate::model::atomic_file::write_atomic(&path, |file| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(text.as_bytes())?;
        Ok(())
    })
    .map_err(io::Error::other)
}
