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
        })
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

/// Hand `request` to the main thread and wait for its answer. `None` if the
/// window has gone.
fn ask(sender: &Sender<Call>, wake: &Arc<dyn Fn() + Send + Sync>, request: Value) -> Option<Value> {
    let (reply, answer) = sync_channel(1);
    sender.send(Call { request, reply }).ok()?;
    wake();
    answer.recv().ok()
}
