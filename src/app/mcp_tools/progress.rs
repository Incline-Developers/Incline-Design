//! Calls held until the background work they wait on is done.
//!
//! Most work an MCP tool starts runs in the background, and the tool returns
//! once it has started. A client that gives a progress token with its call
//! is answered only when that work is done instead, and told how it goes
//! meanwhile with progress notifications: so it waits without polling, and
//! the answer carries how the work went. `get_tasks` with `wait_seconds`
//! is held the same way.
//!
//! A held call waits for every task that was not already running when it
//! came in: the ones it started, and any they lead to (a surface built and
//! then loaded, say).

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::ok;
use crate::{
    app::{App, BackgroundTaskTicket},
    mcp::Call,
};

/// How often a progress notification may go out while nothing much moves.
const PROGRESS_HEARTBEAT: Duration = Duration::from_secs(2);
/// How far the work must get for a notification to go out sooner.
const PROGRESS_STEP: f64 = 0.01;

/// A call waiting for the background work to finish.
pub(crate) struct PendingWait {
    call: Call,
    /// When to answer anyway: `get_tasks`'s `wait_seconds`. A tool's own
    /// work is waited for to the end.
    until: Option<Instant>,
    /// Tasks already running when the call came, which it does not wait for.
    before: HashSet<BackgroundTaskTicket>,
    /// The tool's answer, given once the work is done; `None` answers with
    /// the tasks, as `get_tasks` does.
    result: Option<Value>,
    /// The progress last told, as a fraction done, and when.
    told: Option<(f64, Instant)>,
}

impl App<'_> {
    /// The tasks running now.
    pub(crate) fn running_task_tickets(&self) -> HashSet<BackgroundTaskTicket> {
        self.background_tasks.reported.iter().map(|task| task.ticket).collect()
    }

    /// Answer `call` with `result`, or, if it asked to hear how the work it
    /// started goes and it started some, hold it until that is done.
    /// `before` is what was running when it came in.
    pub(crate) fn reply_or_hold(&mut self, call: Call, result: Value, before: HashSet<BackgroundTaskTicket>) {
        let started = self.running_task_tickets().difference(&before).next().is_some();
        let held = call.request["method"] == "tools/call" && call.progress_token().is_some() && started && result["result"]["isError"] != true;
        if !held {
            call.reply(result);
            return;
        }
        self.mcp_waits.push(PendingWait {
            call,
            until: None,
            before,
            result: Some(result),
            told: None,
        });
        self.poll_mcp_waits();
    }

    /// Take a `get_tasks` call that asked to wait, to be answered once
    /// nothing runs or its time is up.
    pub(crate) fn begin_mcp_wait(&mut self, call: Call) {
        let seconds = call.request["params"]["arguments"]["wait_seconds"].as_f64().unwrap_or(0.0);
        let until = Instant::now() + Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX / 2);
        self.mcp_waits.push(PendingWait {
            call,
            until: Some(until),
            before: HashSet::new(),
            result: None,
            told: None,
        });
        self.poll_mcp_waits();
    }

    /// Tell the held calls how their work goes, and answer those whose work
    /// is done. Returns whether any are still waiting, to keep the event
    /// loop coming back.
    pub(crate) fn poll_mcp_waits(&mut self) -> bool {
        if self.mcp_waits.is_empty() {
            return false;
        }
        let now = Instant::now();
        let mut waiting = Vec::new();
        for mut wait in std::mem::take(&mut self.mcp_waits) {
            let tasks: Vec<_> = self.background_tasks.reported.iter().filter(|task| !wait.before.contains(&task.ticket)).collect();
            if tasks.is_empty() || wait.until.is_some_and(|until| now >= until) {
                self.answer_wait(wait);
                continue;
            }
            if let Some(token) = wait.call.progress_token().cloned() {
                let fractions: Vec<f64> = tasks
                    .iter()
                    .map(|task| task.progress.snapshot().map_or(0.0, |progress| f64::from(progress.fraction)))
                    .collect();
                let fraction = fractions.iter().sum::<f64>() / fractions.len() as f64;
                // Progress only goes forward, though a task the work led to
                // starts again from nothing.
                let fraction = wait.told.map_or(fraction, |(told, _)| fraction.max(told)).clamp(0.0, 1.0);
                let due = wait
                    .told
                    .is_none_or(|(told, at)| fraction - told >= PROGRESS_STEP || now.duration_since(at) >= PROGRESS_HEARTBEAT);
                if due {
                    let message = tasks
                        .iter()
                        .map(|task| task.label.trim_end_matches(['.', '…', ' ']).to_owned())
                        .collect::<Vec<_>>()
                        .join("; ");
                    wait.call.notify(json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/progress",
                        "params": { "progressToken": token, "progress": (fraction * 100.0).round(), "total": 100, "message": message },
                    }));
                    wait.told = Some((fraction, now));
                }
            }
            waiting.push(wait);
        }
        self.mcp_waits = waiting;
        !self.mcp_waits.is_empty()
    }

    /// Answer a held call: the tool's result with what was logged while it
    /// waited, or the tasks.
    fn answer_wait(&mut self, wait: PendingWait) {
        let id = wait.call.request["id"].clone();
        let response = match wait.result {
            Some(mut response) => {
                // Whatever went wrong in the work is in the log; it goes in
                // the answer, as it does for any call.
                let notes: Vec<String> = self.mcp_new_warnings().into_iter().collect();
                if let Some(content) = response["result"]["content"].as_array_mut() {
                    content.push(json!({ "type": "text", "text": "The work it started has finished." }));
                    content.extend(notes.into_iter().map(|text| json!({ "type": "text", "text": text })));
                }
                response
            }
            None => json!({ "jsonrpc": "2.0", "id": id, "result": ok(&self.mcp_tasks()) }),
        };
        wait.call.reply(response);
    }
}
