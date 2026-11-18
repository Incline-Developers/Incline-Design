//! `incline-design mcp`: the MCP server over standard input and output, for
//! a client that can only start a server as a program (Claude Desktop's
//! local servers, among others).
//!
//! It is not a second Incline: it relays each message, a line of JSON, to
//! the window already running, over the HTTP server Preferences > MCP
//! switches on, with the port and token saved there, and writes the answer
//! back as a line. Nothing else is written to standard output, which is the
//! protocol's.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};

use serde_json::{Value, json};

use super::McpSettings;

/// How long to wait for the window to answer one message.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(600);

/// Relay standard input to the running window until it closes.
pub(crate) fn run() -> io::Result<()> {
    let settings = super::load_settings();
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut connection: Option<BufReader<TcpStream>> = None;
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let answer = match relay(&mut connection, &settings, &line) {
            Ok(answer) => answer,
            Err(error) => {
                connection = None;
                unreachable_answer(&line, &settings, &error)
            }
        };
        if let Some(answer) = answer {
            writeln!(stdout, "{answer}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Send one message and read the answer, if it has one.
fn relay(connection: &mut Option<BufReader<TcpStream>>, settings: &McpSettings, message: &str) -> io::Result<Option<String>> {
    if !settings.enabled {
        return Err(io::Error::new(io::ErrorKind::NotConnected, "the MCP server is off"));
    }
    if connection.is_none() {
        let stream = TcpStream::connect(("127.0.0.1", settings.port))?;
        stream.set_read_timeout(Some(ANSWER_TIMEOUT))?;
        *connection = Some(BufReader::new(stream));
    }
    let Some(reader) = connection.as_mut() else {
        return Ok(None);
    };
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
        super::PATH,
        settings.port,
        settings.token,
        message.len(),
        message
    );
    reader.get_mut().write_all(request.as_bytes())?;
    let mut status = String::new();
    if reader.read_line(&mut status)? == 0 {
        return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "the window closed the connection"));
    }
    let code: u16 = status.split_whitespace().nth(1).and_then(|code| code.parse().ok()).unwrap_or(0);
    let mut length = 0;
    let mut header = String::new();
    loop {
        header.clear();
        reader.read_line(&mut header)?;
        let line = header.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("Content-Length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    match code {
        200 => Ok(Some(String::from_utf8_lossy(&body).into_owned())),
        202 => Ok(None),
        401 => Err(io::Error::new(io::ErrorKind::PermissionDenied, "the token did not match the window's")),
        other => Err(io::Error::other(format!("the window answered {other}"))),
    }
}

/// What to answer when the window could not be reached: an error for a
/// request, nothing for a notification.
fn unreachable_answer(line: &str, settings: &McpSettings, error: &io::Error) -> Option<String> {
    let message: Value = serde_json::from_str(line).ok()?;
    message.get("method")?;
    let id = message.get("id")?.clone();
    let reason = format!(
        "Incline Design could not be reached on port {} ({error}). Open it, and switch the MCP server on in Preferences > MCP.",
        settings.port
    );
    Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": reason } }).to_string())
}
