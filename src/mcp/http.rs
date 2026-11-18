//! Just enough HTTP/1.1 to serve MCP's Streamable HTTP transport on the
//! loopback address: one JSON-RPC message per POST, answered with one JSON
//! response (or 202 for a notification), and connections kept open between
//! requests. A request the server has things to say about before it answers
//! (progress, a changed tool list) is answered as a stream of events
//! instead, sent in chunks as they come, the response last.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};

use serde_json::{Value, json};

/// How long a connection may sit idle before it is closed.
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
}

struct Response {
    status: u16,
    reason: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn empty(status: u16, reason: &'static str) -> Self {
        Self {
            status,
            reason,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn json(value: &Value) -> Self {
        Self {
            status: 200,
            reason: "OK",
            headers: vec![("Content-Type", "application/json".to_owned())],
            body: value.to_string().into_bytes(),
        }
    }
}

/// Takes a JSON-RPC request and returns its response, from the main thread,
/// handing what the server says before it to the sink as it comes.
pub(super) type Answer<'a> = dyn Fn(Value, &mut dyn FnMut(Value)) -> Option<Value> + 'a;

/// Serve one connection until the client closes it.
pub(super) fn serve(stream: TcpStream, token: &str, answer: &Answer<'_>) {
    if stream.set_nonblocking(false).is_err() || stream.set_read_timeout(Some(IDLE_TIMEOUT)).is_err() {
        return;
    }
    let Ok(writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let mut writer = writer;
    loop {
        let request = match read_request(&mut reader) {
            Ok(Some(request)) => request,
            Ok(None) | Err(_) => return,
        };
        let close = request.header("Connection").is_some_and(|value| value.eq_ignore_ascii_case("close"));
        if respond(&mut writer, &request, token, answer).is_err() || close {
            return;
        }
    }
}

fn respond(writer: &mut TcpStream, request: &Request, token: &str, answer: &Answer<'_>) -> io::Result<()> {
    let path = request.path.split('?').next().unwrap_or_default();
    if path != super::PATH {
        return write_response(writer, &Response::empty(404, "Not Found"));
    }
    // A web page can reach the loopback address too; the browser says so in
    // the Origin it sends. Only this computer's own pages may come in.
    if let Some(origin) = request.header("Origin")
        && !is_local_origin(origin)
    {
        return write_response(writer, &Response::empty(403, "Forbidden"));
    }
    let authorised = request
        .header("Authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|given| given.trim() == token);
    if !authorised {
        let mut response = Response::empty(401, "Unauthorized");
        response.headers.push(("WWW-Authenticate", "Bearer".to_owned()));
        return write_response(writer, &response);
    }
    match request.method.as_str() {
        "POST" => post(writer, request, answer),
        // A session ends; there is nothing kept for one to let go of.
        "DELETE" => write_response(writer, &Response::empty(200, "OK")),
        // No stream of messages from the server apart from a request's own:
        // everything it says is about a request.
        _ => {
            let mut response = Response::empty(405, "Method Not Allowed");
            response.headers.push(("Allow", "POST, DELETE".to_owned()));
            write_response(writer, &response)
        }
    }
}

fn post(writer: &mut TcpStream, request: &Request, answer: &Answer<'_>) -> io::Result<()> {
    let Ok(message) = serde_json::from_slice::<Value>(&request.body) else {
        return write_response(
            writer,
            &Response::json(&json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": "Parse error" },
            })),
        );
    };
    // A batch, from an older client: each answered in turn, with nothing
    // said along the way.
    if let Some(messages) = message.as_array() {
        let responses: Vec<Value> = messages.iter().filter_map(|message| one(message, answer, &mut |_| {})).collect();
        return write_response(
            writer,
            &if responses.is_empty() {
                Response::empty(202, "Accepted")
            } else {
                Response::json(&Value::Array(responses))
            },
        );
    }
    // The answer becomes a stream at the first thing said before it, if the
    // client takes one; until then it can still go as plain JSON.
    let streams = request.header("Accept").is_some_and(|accept| accept.contains("text/event-stream"));
    let mut stream: Option<io::Result<()>> = None;
    let mut notify = |notification: Value| {
        if !streams {
            return;
        }
        let written = stream.get_or_insert_with(|| write_stream_head(writer));
        if written.is_ok() {
            *written = write_event(writer, &notification);
        }
    };
    let initialize = message["method"] == "initialize";
    let response = one(&message, answer, &mut notify);
    match stream {
        Some(written) => {
            written?;
            if let Some(response) = response {
                write_event(writer, &response)?;
            }
            writer.write_all(b"0\r\n\r\n")?;
            writer.flush()
        }
        None => match response {
            Some(response) => {
                let mut response = Response::json(&response);
                if initialize {
                    response.headers.push(("Mcp-Session-Id", uuid::Uuid::new_v4().simple().to_string()));
                }
                write_response(writer, &response)
            }
            None => write_response(writer, &Response::empty(202, "Accepted")),
        },
    }
}

/// Start an answer sent as a stream of events.
fn write_stream_head(writer: &mut TcpStream) -> io::Result<()> {
    writer.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nTransfer-Encoding: chunked\r\n\r\n")?;
    writer.flush()
}

/// One message of a stream, as an event in a chunk of its own.
fn write_event(writer: &mut TcpStream, message: &Value) -> io::Result<()> {
    let event = format!("event: message\ndata: {message}\n\n");
    write!(writer, "{:x}\r\n{event}\r\n", event.len())?;
    writer.flush()
}

/// The response to one message: none for a notification, or for a response
/// the client sent.
pub(super) fn one(message: &Value, answer: &Answer<'_>, notify: &mut dyn FnMut(Value)) -> Option<Value> {
    if message.get("method").is_none() || message.get("id").is_none() {
        return None;
    }
    Some(answer(message.clone(), notify).unwrap_or_else(|| {
        json!({
            "jsonrpc": "2.0",
            "id": message["id"],
            "error": { "code": -32603, "message": "Incline Design is closing" },
        })
    }))
}

/// Whether `origin` is a page served from this computer.
fn is_local_origin(origin: &str) -> bool {
    let host = origin.split("://").nth(1).unwrap_or(origin);
    let host = host.split('/').next().unwrap_or(host);
    let host = if host.starts_with('[') {
        host.split(']').next().map(|host| &host[1..]).unwrap_or(host)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

/// Read one request, or `None` if the client closed the connection.
fn read_request(reader: &mut BufReader<TcpStream>) -> io::Result<Option<Request>> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad request line"));
    };
    let (method, path) = (method.to_owned(), path.to_owned());
    let mut headers = Vec::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    let mut request = Request {
        method,
        path,
        headers,
        body: Vec::new(),
    };
    let length = request.header("Content-Length").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
    request.body.resize(length, 0);
    reader.read_exact(&mut request.body)?;
    Ok(Some(request))
}

fn write_response(writer: &mut TcpStream, response: &Response) -> io::Result<()> {
    let mut head = format!("HTTP/1.1 {} {}\r\nContent-Length: {}\r\n", response.status, response.reason, response.body.len());
    for (name, value) in &response.headers {
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    writer.write_all(head.as_bytes())?;
    writer.write_all(&response.body)?;
    writer.flush()
}
