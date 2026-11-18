//! `incline-design mcp`: the MCP server over standard input and output, for
//! a client that can only start a server as a program (Claude Desktop's
//! local servers, among others).
//!
//! It is not a second Incline: it relays each message, a line of JSON, to
//! the window already running, over the HTTP server Preferences > MCP
//! switches on, with the port and token saved there, and writes the answer
//! back as a line. Nothing else is written to standard output, which is the
//! protocol's.
//!
//! `incline-design mcp --headless` is the other way round: an Incline of the
//! client's own in this process, with no window (`app::headless`).

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
        let reused = connection.is_some();
        // What the window says before its answer goes straight out, a line
        // each, as it comes.
        let mut forward = |message: &str| -> io::Result<()> {
            writeln!(stdout, "{message}")?;
            stdout.flush()
        };
        let mut result = relay(&mut connection, &settings, &line, &mut forward);
        // A connection kept from an earlier message goes stale when the
        // window restarts or drops it while idle; the message never reached
        // the window then, so it is sent again on a fresh one.
        if reused && result.as_ref().is_err_and(stale) {
            connection = None;
            result = relay(&mut connection, &settings, &line, &mut forward);
        }
        let answer = match result {
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
fn relay(connection: &mut Option<BufReader<TcpStream>>, settings: &McpSettings, message: &str, forward: &mut dyn FnMut(&str) -> io::Result<()>) -> io::Result<Option<String>> {
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
    let mut chunked = false;
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
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("Transfer-Encoding")
            && value.trim().eq_ignore_ascii_case("chunked")
        {
            chunked = true;
        }
    }
    if chunked {
        return read_event_stream(reader, forward).map(Some);
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

/// Read an answer sent as a stream of events, passing on each message that
/// comes before the response, and return the response.
fn read_event_stream(reader: &mut BufReader<TcpStream>, forward: &mut dyn FnMut(&str) -> io::Result<()>) -> io::Result<String> {
    let mut pending = String::new();
    let mut response = None;
    loop {
        let mut size = String::new();
        reader.read_line(&mut size)?;
        let size = usize::from_str_radix(size.trim(), 16).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "a chunk without a size"))?;
        let mut chunk = vec![0; size + 2];
        reader.read_exact(&mut chunk)?;
        if size == 0 {
            break;
        }
        pending.push_str(&String::from_utf8_lossy(&chunk[..size]));
        while let Some(end) = pending.find("\n\n") {
            let event: String = pending.drain(..end + 2).collect();
            for data in event.lines().filter_map(|line| line.strip_prefix("data:")) {
                let data = data.trim();
                let is_response = serde_json::from_str::<Value>(data).is_ok_and(|message| message.get("method").is_none());
                if is_response {
                    response = Some(data.to_owned());
                } else {
                    forward(data)?;
                }
            }
        }
    }
    response.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "the window's stream ended without an answer"))
}

/// Whether `error` means the connection was closed under the message before
/// the window read it.
fn stale(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe)
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
