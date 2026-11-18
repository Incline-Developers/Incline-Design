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
    /// Whether it also waits for the Solids, Haulage and Schedule Setup
    /// pipelines to settle: a step run asked for there may be taken up by a
    /// run already under way rather than start a task of its own.
    pipelines: bool,
    /// How many times a planning run it asked for has been started again
    /// after an edit stopped it partway.
    reruns: u32,
    /// For a calculation asked for: the run serial before it, so an attempt
    /// that published nothing after it is known to be this call's.
    calculation_since: Option<u64>,
}

/// How many times a planning run stopped by an edit is started again before
/// the call is told it did not finish.
const PLANNING_RERUNS: u32 = 3;

/// The tools whose work is the pipelines', waited on to the end of them.
const PIPELINE_TOOLS: [&str; 2] = ["run_planning", "run_schedule"];

impl App<'_> {
    /// The tasks running now.
    pub(crate) fn running_task_tickets(&self) -> HashSet<BackgroundTaskTicket> {
        self.background_tasks.reported.iter().map(|task| task.ticket).collect()
    }

    /// Whether the Solids, Haulage or Schedule Setup pipeline has a step
    /// queued or running, or a schedule calculation is asked for or under way.
    fn pipelines_busy(&self) -> bool {
        self.planning_pipeline.as_ref().is_some_and(|pipeline| pipeline.is_running())
            || self.haulage_pipeline.as_ref().is_some_and(|pipeline| pipeline.is_running())
            || self.schedule_pipeline.as_ref().is_some_and(|pipeline| pipeline.is_running())
            || self.mcp_pending_schedule_run.is_some()
            // The calculation runs as a job of its own, not a reported task.
            || self.any_job_pending(|key| matches!(key, crate::app::jobs::JobKey::ScheduleRun { .. }))
    }

    /// Whether a schedule calculation asked for now would be turned away
    /// because what it reads (the Setup steps, the Solids run behind them)
    /// is still being worked out.
    pub(crate) fn schedule_setup_running(&self) -> bool {
        matches!(self.schedule_run_inputs(), Err(crate::app::schedule_pipeline::ScheduleNotReady::Running(_)))
    }

    /// Answer `call` with `result`, or, if it asked to hear how the work it
    /// started goes and it started some, hold it until that is done.
    /// `before` is what was running when it came in.
    pub(crate) fn reply_or_hold(&mut self, call: Call, result: Value, before: HashSet<BackgroundTaskTicket>) {
        let pipelines = call.request["params"]["name"].as_str().is_some_and(|name| PIPELINE_TOOLS.contains(&name));
        let calculation_since = (call.request["params"]["name"] == "run_schedule"
            && matches!(call.request["params"]["arguments"]["what"].as_str(), Some("period" | "all" | "improve")))
        .then_some(self.mcp_schedule_serial_asked);
        let started = self.running_task_tickets().difference(&before).next().is_some() || (pipelines && self.pipelines_busy());
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
            pipelines,
            reruns: 0,
            calculation_since,
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
            pipelines: false,
            reruns: 0,
            calculation_since: None,
        });
        self.poll_mcp_waits();
    }

    /// Tell the held calls how their work goes, and answer those whose work
    /// is done. Returns whether any are still waiting, to keep the event
    /// loop coming back.
    pub(crate) fn poll_mcp_waits(&mut self) -> bool {
        // A calculation asked for while the Setup steps ran starts once they
        // have settled; the calls holding for it wait on its task from here.
        if let Some(mode) = self.mcp_pending_schedule_run
            && !self.schedule_setup_running()
        {
            self.mcp_pending_schedule_run = None;
            self.start_schedule_run(mode);
        }
        if self.mcp_waits.is_empty() {
            return false;
        }
        let now = Instant::now();
        let mut waiting = Vec::new();
        for mut wait in std::mem::take(&mut self.mcp_waits) {
            let quiet = !self.background_tasks.reported.iter().any(|task| !wait.before.contains(&task.ticket));
            let settled = quiet && !(wait.pipelines && self.pipelines_busy());
            if settled && let Some(problem) = self.planning_unfinished(&mut wait) {
                match problem {
                    // Started again: wait on.
                    None => {
                        waiting.push(wait);
                        continue;
                    }
                    Some(problem) => {
                        self.answer_wait(wait, Some(problem));
                        continue;
                    }
                }
            }
            if settled && let Some(problem) = self.calculation_failed(&wait) {
                self.answer_wait(wait, Some(problem));
                continue;
            }
            if settled || wait.until.is_some_and(|until| now >= until) {
                self.answer_wait(wait, None);
                continue;
            }
            let tasks: Vec<_> = self.background_tasks.reported.iter().filter(|task| !wait.before.contains(&task.ticket)).collect();
            if let Some(token) = wait.call.progress_token().cloned() {
                let fractions: Vec<f64> = tasks
                    .iter()
                    .map(|task| task.progress.snapshot().map_or(0.0, |progress| f64::from(progress.fraction)))
                    .collect();
                // Only a pipeline step may be under way, with no task to
                // measure: nothing is told as done until one is.
                let fraction = if fractions.is_empty() {
                    0.0
                } else {
                    fractions.iter().sum::<f64>() / fractions.len() as f64
                };
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

    /// For a held `run_planning` of every step whose work has settled
    /// without measuring the reserves: `Some(None)` when it has been started
    /// again (an edit made as the run began, such as a new solid's benching
    /// being filled in, stops it partway, where the window would wait for
    /// Run to be pressed again), `Some(Some(why))` when it will not finish,
    /// and `None` when there is nothing to say.
    fn planning_unfinished(&mut self, wait: &mut PendingWait) -> Option<Option<String>> {
        use crate::app::commands::solids_view::PlanningNotReady;

        let params = &wait.call.request["params"];
        if params["name"] != "run_planning" || params["arguments"]["step"].is_string() {
            return None;
        }
        let arguments = params["arguments"].clone();
        match self.planning_snapshot() {
            Ok(_) | Err(PlanningNotReady::Running { .. } | PlanningNotReady::NoProject) => None,
            Err(PlanningNotReady::NotRun { .. } | PlanningNotReady::Stale { .. } | PlanningNotReady::Incomplete { .. }) if wait.reruns < PLANNING_RERUNS => {
                wait.reruns += 1;
                match self.mcp_run_planning(&arguments) {
                    Ok(_) => Some(None),
                    Err(error) => Some(Some(error)),
                }
            }
            Err(not_ready) => Some(Some(format!("The planning steps did not finish: {}", super::plain(&not_ready.describe())))),
        }
    }

    /// Why the calculation a held `run_schedule` asked for published nothing,
    /// if it did not: refused as it was captured, found no schedule, or
    /// failed. The window shows this as a warning; the caller is told it as
    /// an error rather than a note beside a result that reads as success.
    fn calculation_failed(&self, wait: &PendingWait) -> Option<String> {
        let since = wait.calculation_since?;
        let attempt = self.schedule_run_diagnostics.as_ref().filter(|attempt| attempt.serial > since)?;
        let headline = crate::app::schedule_run::attempt_headline(attempt);
        let mut text = super::plain(&headline);
        for message in attempt.messages.iter().filter(|message| !headline.contains(message.as_str())) {
            text.push('\n');
            text.push_str(&super::plain(message));
        }
        Some(text)
    }

    /// Answer a held call: the tool's result with what was logged while it
    /// waited, or the tasks. `problem` turns the result into an error saying
    /// why the work did not do what was asked.
    fn answer_wait(&mut self, wait: PendingWait, problem: Option<String>) {
        let id = wait.call.request["id"].clone();
        let response = match wait.result {
            Some(mut response) => {
                // Whatever went wrong in the work is in the log; it goes in
                // the answer, as it does for any call.
                let notes: Vec<String> = self.mcp_new_warnings().into_iter().collect();
                if let Some(problem) = problem {
                    response["result"]["isError"] = json!(true);
                    if let Some(content) = response["result"]["content"].as_array_mut() {
                        content.push(json!({ "type": "text", "text": problem }));
                    }
                } else if let Some(content) = response["result"]["content"].as_array_mut() {
                    content.push(json!({ "type": "text", "text": "The work it started has finished." }));
                }
                if let Some(content) = response["result"]["content"].as_array_mut() {
                    content.extend(notes.into_iter().map(|text| json!({ "type": "text", "text": text })));
                }
                response
            }
            None => json!({ "jsonrpc": "2.0", "id": id, "result": ok(&self.mcp_tasks()) }),
        };
        wait.call.reply(response);
    }
}
