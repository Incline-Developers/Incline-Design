//! The listener: takes connections on the loopback address and hands each
//! JSON-RPC request to the main thread ([`Call`]).

use std::{
    io,
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, SyncSender, channel, sync_channel},
    },
    thread::JoinHandle,
    time::Duration,
};

use serde_json::Value;

/// How often the listener looks for a stop while no one connects.
const ACCEPT_POLL: Duration = Duration::from_millis(100);

/// One JSON-RPC request, waiting for the main thread's answer.
pub(crate) struct Call {
    pub(crate) request: Value,
    reply: SyncSender<Value>,
}

impl Call {
    /// Answer the request. A client that gave up waiting is not an error.
    pub(crate) fn reply(self, response: Value) {
        let _ = self.reply.send(response);
    }
}

/// The server, while it runs. Dropping it stops it.
pub(crate) struct Server {
    port: u16,
    calls: Receiver<Call>,
    stop: Arc<AtomicBool>,
    listener: Option<JoinHandle<()>>,
    /// Set once the one client of a server on standard input has gone.
    closed: Arc<AtomicBool>,
}

impl Server {
    /// Listen on `port`, asking for `token`. `wake` is called whenever a
    /// request is waiting, from the connection's thread, to rouse the event
    /// loop.
    pub(crate) fn start(port: u16, token: String, wake: Arc<dyn Fn() + Send + Sync>) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let (sender, calls) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let token = Arc::new(token);
        let listener = std::thread::Builder::new()
            .name("mcp-listener".to_owned())
            .spawn(move || listen(&listener, &stopped, &token, &sender, &wake))?;
        Ok(Self {
            port,
            calls,
            stop,
            listener: Some(listener),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Serve the one client on the other end of standard input and output,
    /// a message a line each way, as `incline-design mcp --headless` does:
    /// the process is the client's own Incline. Nothing else may write to
    /// standard output, which is the protocol's.
    pub(crate) fn on_standard_io(wake: Arc<dyn Fn() + Send + Sync>) -> io::Result<Self> {
        let (sender, calls) = channel();
        let closed = Arc::new(AtomicBool::new(false));
        let gone = Arc::clone(&closed);
        // Not joined: it sits in a read of standard input until the client
        // closes it, which is also when the process is done.
        std::thread::Builder::new().name("mcp-stdio".to_owned()).spawn(move || {
            if let Err(error) = serve_standard_io(&sender, &wake) {
                log::warn!("The MCP client's connection failed: {error}");
            }
            gone.store(true, Ordering::Relaxed);
            wake();
        })?;
        Ok(Self {
            port: 0,
            calls,
            stop: Arc::new(AtomicBool::new(false)),
            listener: None,
            closed,
        })
    }

    /// Whether the client of a server on standard input has gone.
    pub(crate) fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// The requests that have come in since the last look.
    pub(crate) fn take_calls(&self) -> Vec<Call> {
        self.calls.try_iter().collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

/// Take connections until told to stop, each on a thread of its own.
fn listen(listener: &TcpListener, stop: &AtomicBool, token: &Arc<String>, sender: &Sender<Call>, wake: &Arc<dyn Fn() + Send + Sync>) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let token = Arc::clone(token);
                let sender = sender.clone();
                let wake = Arc::clone(wake);
                let spawned = std::thread::Builder::new()
                    .name("mcp-connection".to_owned())
                    .spawn(move || super::http::serve(stream, &token, &|request| ask(&sender, &wake, request)));
                if let Err(error) = spawned {
                    log::warn!("An MCP connection could not be served: {error}");
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(ACCEPT_POLL),
            Err(error) => {
                log::warn!("The MCP server could not take a connection: {error}");
                std::thread::sleep(ACCEPT_POLL);
            }
        }
    }
}

/// Answer each line of standard input on standard output until it closes.
fn serve_standard_io(sender: &Sender<Call>, wake: &Arc<dyn Fn() + Send + Sync>) -> io::Result<()> {
    use std::io::{BufRead, Write};

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let answer = |request| ask(sender, wake, request);
        let response = match serde_json::from_str::<Value>(&line) {
            Err(_) => Some(serde_json::json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "Parse error" } })),
            // A batch, from an older client: each answered in turn.
            Ok(Value::Array(messages)) => {
                let responses: Vec<Value> = messages.iter().filter_map(|message| super::http::one(message, &answer)).collect();
                (!responses.is_empty()).then_some(Value::Array(responses))
            }
            Ok(message) => super::http::one(&message, &answer),
        };
        if let Some(response) = response {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Hand `request` to the main thread and wait for its answer. `None` if the
/// window has gone.
fn ask(sender: &Sender<Call>, wake: &Arc<dyn Fn() + Send + Sync>, request: Value) -> Option<Value> {
    let (reply, answer) = sync_channel(1);
    sender.send(Call { request, reply }).ok()?;
    wake();
    answer.recv().ok()
}
