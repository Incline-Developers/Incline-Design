//! The schedule solve, run in a child process of the app.
//!
//! SCIP, SoPlex, Ipopt and MUMPS are C, C++ and Fortran, and a fault in any
//! of them - a heap corruption, an abort, a hang in `malloc` - is not a Rust
//! panic the job queue can catch. In the app's own process it takes the
//! whole app, and every unsaved edit, down with it; this happened with
//! MUMPS's METIS ordering (see `scip_blend::IPOPT_OPTIONS`). So the solve runs
//! in a second copy of the app's own binary, started with [`SOLVER_FLAG`],
//! and a fault there ends one run, which is reported as a failed attempt.
//!
//! # The exchange
//!
//! The app writes one [`Request`] to the process's stdin as a line of JSON:
//! the captured [`BlendInput`], the run identity and the options. The
//! process runs [`execute_schedule`] on it exactly as the app used to, and
//! writes [`Message`]s back on stdout, one per line: forwarded log records,
//! the day-by-day schedule when there is one, and the finished run. Every
//! message line carries [`FRAME`], so anything a native library prints to
//! stdout - SCIP's own log under `INCLINE_SCIP_LOG`, a Fortran runtime
//! warning - is told apart and logged rather than misread. stderr is logged
//! too, and its last lines explain a process that died.
//!
//! The app does not take the process's word for a schedule: each one is
//! replayed again on the app's side (see [`ScheduleCompletion::from_report`]).
//!
//! # Stopping
//!
//! Cancelling a run kills the process; nothing it holds needs a clean
//! shutdown. The process also watches its stdin, and closes itself down when
//! the app goes away without saying so.

use std::{
    collections::VecDeque,
    ffi::OsStr,
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

use super::{
    jobs::CancelFlag,
    schedule_solve::{ScheduleActivity, ScheduleCompletion, ScheduleRunIdentity, ScheduleSolveOptions, SolveTermination, execute_schedule},
    scip_blend::ReportedCompletion,
};
use crate::{i18n::tr, model::schedule::optimisation::blended::input::BlendInput};

/// The command-line argument that starts the app's binary as a solver
/// process instead of the app.
pub(crate) const SOLVER_FLAG: &str = "--schedule-solver";

/// Marks a message line in the process's stdout.
const FRAME: &str = "\u{1e}incline-solver ";

/// How often the app checks for a cancelled run while it waits.
const POLL: Duration = Duration::from_millis(100);

/// How long a process that has sent its answer may take to exit before it is
/// killed.
const EXIT_GRACE: Duration = Duration::from_secs(5);

/// How long an orphaned process, its app gone, may take to stop its solve
/// before it aborts.
const ORPHAN_GRACE: Duration = Duration::from_secs(30);

/// stderr lines kept to explain a process that died.
const STDERR_TAIL: usize = 12;

#[derive(serde::Serialize, serde::Deserialize)]
struct Request<I> {
    identity: ScheduleRunIdentity,
    options: ScheduleSolveOptions,
    input: I,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Message {
    Log {
        level: usize,
        target: String,
        text: String,
    },
    /// The day-by-day schedule, shown while the whole horizon solves.
    Early(Box<ReportedCompletion>),
    Done(Box<ReportedCompletion>),
}

/// Whether this process was started as a solver process.
pub(crate) fn is_solver_invocation() -> bool {
    std::env::args_os().nth(1).as_deref() == Some(OsStr::new(SOLVER_FLAG))
}

/// Solve in a solver process, with [`execute_schedule`]'s contract: the
/// same completion, and `early` called at most once with a usable
/// day-by-day schedule. Returns once the run has finished, failed or been
/// cancelled; the process is gone by then.
pub(crate) fn solve(
    input: Arc<BlendInput>,
    identity: ScheduleRunIdentity,
    options: ScheduleSolveOptions,
    cancel: &CancelFlag,
    early: &dyn Fn(&ScheduleCompletion),
) -> ScheduleCompletion {
    let run_id = identity.run_id;
    let failed = |termination: SolveTermination, diagnostic: String| {
        let mut out = ScheduleCompletion::new(identity, options, Arc::clone(&input));
        out.stop(termination, diagnostic);
        out
    };
    let mut process = match Process::start() {
        Ok(process) => process,
        Err(error) => {
            log::error!("schedule run {run_id}: the solver process did not start: {error}");
            return failed(SolveTermination::BackendFailure, tr!("schedule-solver-not-started", reason = error.to_string()));
        }
    };
    log::info!("schedule run {run_id}: solving in process {}", process.child.id());

    let request = Request {
        identity,
        options,
        input: &*input,
    };
    let sent = process.send(&request);

    let mut answer = None;
    if sent.is_ok() {
        loop {
            if cancel.is_cancelled() {
                process.kill();
                return failed(SolveTermination::Cancelled, "cancelled while the solver process ran".into());
            }
            match process.messages.recv_timeout(POLL) {
                Ok(Message::Log { level, target, text }) => {
                    let level = log::Level::iter().nth(level.saturating_sub(1)).unwrap_or(log::Level::Info);
                    log::log!(target: &target, level, "{text}");
                }
                Ok(Message::Early(report)) => {
                    let shown = ScheduleCompletion::from_report(identity, options, Arc::clone(&input), *report, cancel);
                    if shown.usable() {
                        early(&shown);
                    } else {
                        log::warn!("schedule run {run_id}: the day-by-day schedule failed the app's replay: {:?}", shown.diagnostic);
                    }
                }
                Ok(Message::Done(report)) => {
                    answer = Some(report);
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    let status = process.finish(answer.is_some());
    match answer {
        Some(report) => ScheduleCompletion::from_report(identity, options, input, *report, cancel),
        None if cancel.is_cancelled() => failed(SolveTermination::Cancelled, "cancelled while the solver process ran".into()),
        None => {
            let tail = process.stderr_tail();
            let ended = status.map_or_else(|| "still running; killed".to_owned(), describe_exit);
            log::error!(
                "schedule run {run_id}: the solver process ended without an answer ({ended}); request sent: {}",
                sent.is_ok()
            );
            for line in &tail {
                log::error!("schedule run {run_id}: solver process: {line}");
            }
            failed(SolveTermination::BackendFailure, tr!("schedule-solver-crashed", status = ended))
        }
    }
}

/// A running solver process and the threads reading its output. Dropping it
/// kills the process, so no path out of [`solve`] leaves one behind.
struct Process {
    child: Child,
    messages: mpsc::Receiver<Message>,
    stderr: Arc<Mutex<VecDeque<String>>>,
    readers: Vec<std::thread::JoinHandle<()>>,
}

impl Process {
    fn start() -> std::io::Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command.arg(SOLVER_FLAG).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // A debug build is a console program; its solver process would
            // otherwise open a console window of its own.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        let (sender, messages) = mpsc::channel();
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let kept = Arc::clone(&tail);
        let pid = child.id();
        let readers = vec![
            std::thread::Builder::new()
                .name("schedule-solver-out".into())
                .spawn(move || read_messages(stdout, &sender, pid))?,
            std::thread::Builder::new().name("schedule-solver-err".into()).spawn(move || {
                for line in lines(stderr) {
                    log::warn!("solver process {pid}: {line}");
                    let mut tail = kept.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    if tail.len() == STDERR_TAIL {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            })?,
        ];
        Ok(Self {
            child,
            messages,
            stderr: tail,
            readers,
        })
    }

    /// Write the request. stdin then stays open: its closing is how the
    /// process learns that the app has gone.
    fn send(&mut self, request: &Request<&BlendInput>) -> std::io::Result<()> {
        let stdin = self.child.stdin.as_mut().expect("stdin was piped");
        let mut writer = std::io::BufWriter::new(stdin);
        serde_json::to_writer(&mut writer, request)?;
        writer.write_all(b"\n")?;
        writer.flush()
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Reap the process: at once when it has already said everything, or
    /// after [`EXIT_GRACE`] otherwise. `None` if it had to be killed.
    fn finish(&mut self, answered: bool) -> Option<ExitStatus> {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + EXIT_GRACE;
        let status = loop {
            match self.child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
                _ => {
                    if answered {
                        log::warn!("solver process {} did not exit after answering; killed", self.child.id());
                    }
                    self.kill();
                    break None;
                }
            }
        };
        // Both pipes are closed once the process has gone, so the readers
        // finish; joining them makes the stderr tail complete.
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
        status
    }

    fn stderr_tail(&self) -> Vec<String> {
        self.stderr.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().cloned().collect()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            self.kill();
        }
    }
}

fn describe_exit(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            let name = match signal {
                6 => " (abort)",
                9 => " (killed)",
                11 => " (segmentation fault)",
                _ => "",
            };
            return format!("signal {signal}{name}");
        }
    }
    status.to_string()
}

/// Lines of a pipe, lossily decoded: native output need not be UTF-8.
fn lines(pipe: impl Read) -> impl Iterator<Item = String> {
    let mut reader = BufReader::new(pipe);
    std::iter::from_fn(move || {
        let mut line = Vec::new();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(String::from_utf8_lossy(line.trim_ascii_end()).into_owned()),
        }
    })
}

fn read_messages(stdout: impl Read, sender: &mpsc::Sender<Message>, pid: u32) {
    for line in lines(stdout) {
        let Some(at) = line.find(FRAME) else {
            log::info!("solver process {pid}: {line}");
            continue;
        };
        if at > 0 {
            log::info!("solver process {pid}: {}", &line[..at]);
        }
        match serde_json::from_str(&line[at + FRAME.len()..]) {
            Ok(message) => {
                if sender.send(message).is_err() {
                    return;
                }
            }
            // Without its answer the run fails as a process that ended
            // without one, which is what it amounts to.
            Err(error) => log::error!("solver process {pid}: unreadable message: {error}"),
        }
    }
}

/// The solver process's `main`: read one request, solve it, report, exit.
pub(crate) fn run_solver() -> i32 {
    let output = Arc::new(Mutex::new(std::io::stdout()));
    let _ = log::set_boxed_logger(Box::new(Forward(Arc::clone(&output))));
    log::set_max_level(log::LevelFilter::Info);

    let mut stdin = BufReader::new(std::io::stdin());
    let mut line = String::new();
    if let Err(error) = stdin.read_line(&mut line) {
        eprintln!("the solver process could not read its request: {error}");
        return 2;
    }
    let request: Request<BlendInput> = match serde_json::from_str(&line) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("the solver process could not read its request: {error}");
            return 2;
        }
    };
    drop(line);

    let cancel = CancelFlag::default();
    let watched = cancel.clone();
    // Nothing more is ever written to stdin, so it ends only when the app
    // closes it or exits. Either way no one will read an answer: stop the
    // solve, and if it does not stop, stop the process.
    let _ = std::thread::Builder::new().name("schedule-solver-watch".into()).spawn(move || {
        let _ = std::io::copy(&mut stdin, &mut std::io::sink());
        watched.cancel();
        std::thread::sleep(ORPHAN_GRACE);
        std::process::abort();
    });

    let early = |completion: &ScheduleCompletion| send(&output, &Message::Early(Box::new(completion.report())));
    let completion = execute_schedule(Arc::new(request.input), request.identity, request.options, &cancel, &ScheduleActivity::default(), &early);
    send(&output, &Message::Done(Box::new(completion.report())));
    0
}

/// Write one message line. Held under one lock, so log records from other
/// threads never land inside a message.
fn send(output: &Mutex<std::io::Stdout>, message: &Message) {
    let Ok(mut text) = serde_json::to_string(message) else {
        eprintln!("the solver process could not encode a message");
        return;
    };
    text.insert_str(0, FRAME);
    text.push('\n');
    let output = output.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut out = output.lock();
    let _ = out.write_all(text.as_bytes()).and_then(|()| out.flush());
}

/// The solver process's logger: every record goes to the app, which logs it
/// as its own.
struct Forward(Arc<Mutex<std::io::Stdout>>);

impl log::Log for Forward {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            send(
                &self.0,
                &Message::Log {
                    level: record.level() as usize,
                    target: record.target().to_owned(),
                    text: record.args().to_string(),
                },
            );
        }
    }

    fn flush(&self) {}
}
