//! The browser's side of linked geophysics files, none ever held whole. An
//! index pass reads through a pump (`ChunkReader`): the page thread pulls
//! `Blob` slices and hands them to the job's worker over a channel, since a
//! `web_sys::File` cannot leave the page thread and the page must never
//! block. A hole's runs are read as slices on the page thread, then parsed
//! on the job queue.

use std::{
    io::{BufReader, Read},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
};

use crate::{
    app::{App, geophysics::Indexed, memory::MemoryReservation},
    i18n::{tr, tr_format},
    model::{
        drill_hole::DrillHoleId,
        formats::{
            csv_drill_hole::{CsvDrillColumnRole, CsvDrillError, CsvDrillFileMapping},
            csv_geophysics::{self, ColumnSource, IndexedFile},
        },
        geophysics::{FileIdentity, GeophysicsLink, LinkState, LinkedColumn},
    },
    userspace_log, userspace_warn,
};

/// Bytes pulled from the browser per chunk.
const PUMP_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
/// Read buffer wrapping the pump, so most `Read` calls (CSV records) are
/// served from memory rather than a channel `recv`.
const PUMP_BUFFER_BYTES: usize = 1024 * 1024;
/// Chunks let ahead of the reader before the page thread waits: bounds how
/// much of the file the channel can hold at once.
const PUMP_MAX_IN_FLIGHT: usize = 4;
/// How long the page thread waits before checking backpressure again.
const PUMP_BACKPRESSURE_MS: i32 = 10;

impl<'a> App<'a> {
    /// Take up a saved or restored link: read from the files picked this
    /// session when they match it, or ask for the first one that does not.
    pub(super) fn resolve_browser_link(&mut self, id: DrillHoleId, link: Arc<GeophysicsLink>) {
        match link.files.iter().find(|file| !self.geophysics_files.iter().any(|(held, _)| held.matches(&file.identity))) {
            Some(missing) => {
                let file = missing.identity.name.clone();
                self.restart_geophysics(id, Some(&link), LinkState::NeedsPick { file });
            }
            None => {
                self.restart_geophysics(id, Some(&link), LinkState::Ready);
            }
        }
    }

    /// Files picked for a dataset (from the dataset menu or the
    /// inspector's "Pick" button): identify them on the page thread, then
    /// either hold them for an already-known link or index them as a new one.
    pub(crate) fn link_geophysics_files(&mut self, id: DrillHoleId, files: Vec<web_sys::File>) {
        let Some(proxy) = self.web_event_loop_proxy.clone() else { return };
        wasm_bindgen_futures::spawn_local(async move {
            let mut identified = Vec::with_capacity(files.len());
            for file in files {
                let identity = identify_browser_file(&file).await;
                identified.push((file, identity));
            }
            let _ = proxy.send_event(crate::app::AppEvent::GeophysicsFileIdentified { dataset: id, files: identified });
        });
    }

    /// Picked files' identities, back from the page thread. One that cannot
    /// be identified is left out; the rest are held when every one is a file
    /// of the current link, else indexed as a new link.
    pub(super) fn handle_geophysics_file_identified(&mut self, id: DrillHoleId, files: Vec<(web_sys::File, std::result::Result<FileIdentity, String>)>) {
        let several = files.len() > 1;
        let mut identified = Vec::with_capacity(files.len());
        for (file, identity) in files {
            match identity {
                Ok(identity) => identified.push((file, identity)),
                Err(error) => {
                    if several {
                        left_out(&file.name(), &error);
                    } else {
                        userspace_warn!("{}", tr_format!(literal = "Could not read '%name%': %error%", name = file.name(), error = error));
                    }
                }
            }
        }
        if identified.is_empty() {
            return;
        }
        let current = self.geophysics_link(id);
        if let Some(link) = current.clone()
            && identified.iter().all(|(_, identity)| link.files.iter().any(|linked| linked.identity.matches(identity)))
        {
            for (file, identity) in identified {
                let name = link
                    .files
                    .iter()
                    .find(|linked| linked.identity.matches(&identity))
                    .map_or_else(|| identity.name.clone(), |linked| linked.identity.name.clone());
                self.hold_geophysics_file(identity, file, id);
                userspace_log!("{}", tr_format!(literal = "'%name%' is used for this session's downhole geophysics", name = name));
            }
            self.resolve_browser_link(id, link);
            return;
        }
        if self.known_holes(id).is_none() {
            userspace_warn!("{}", tr!(literal = "Load the drillhole dataset before linking geophysics to it"));
            return;
        }
        let generation = self.restart_geophysics(id, current.as_ref(), LinkState::Indexing);
        let previous = current.as_ref().and_then(|link| link.files.first()).map(|linked| linked.columns.clone());
        let tasks = identified
            .into_iter()
            .map(|(file, identity)| (Columns::Previous(previous.clone()), file, identity))
            .collect();
        self.spawn_browser_index(id, generation, tasks);
    }

    /// A freshly loaded CSV bundle's geophysics mappings: identify each
    /// picked file, then index them into one new link. A file that cannot
    /// be identified is left out, as one that fails to index is.
    pub(super) fn link_bundle_geophysics(&mut self, id: DrillHoleId, geophysics: Vec<(CsvDrillFileMapping, web_sys::File)>) {
        let Some(proxy) = self.web_event_loop_proxy.clone() else { return };
        let generation = self.restart_geophysics(id, None, LinkState::Indexing);
        wasm_bindgen_futures::spawn_local(async move {
            let several = geophysics.len() > 1;
            let mut identified = Vec::with_capacity(geophysics.len());
            let mut failed = None;
            for (mapping, file) in geophysics {
                match identify_browser_file(&file).await {
                    Ok(identity) => identified.push((mapping, file, identity)),
                    Err(error) => {
                        if several {
                            left_out(&file.name(), &error);
                        }
                        failed = Some(error);
                    }
                }
            }
            let result = match failed {
                Some(error) if identified.is_empty() => Err(error),
                _ => Ok(identified),
            };
            let _ = proxy.send_event(crate::app::AppEvent::GeophysicsBundleIdentified { dataset: id, generation, result });
        });
    }

    /// A bundle's geophysics files' identities, back from the page thread:
    /// index them all into the dataset's first link.
    pub(super) fn handle_geophysics_bundle_identified(
        &mut self,
        id: DrillHoleId,
        generation: u64,
        result: std::result::Result<Vec<(CsvDrillFileMapping, web_sys::File, FileIdentity)>, String>,
    ) {
        if self.well_logs.generation(id) != Some(generation) {
            return;
        }
        match result {
            Ok(identified) => {
                let files = identified
                    .into_iter()
                    .map(|(mapping, file, identity)| (Columns::Mapped(mapping.columns), file, identity))
                    .collect();
                self.spawn_browser_index(id, generation, files);
            }
            Err(error) => self.finish_geophysics_index(id, generation, None, Err(anyhow::anyhow!(error))),
        }
    }

    /// Index picked files into a new link for a dataset on the job queue,
    /// each streamed through a pump. A file that fails is left out and the
    /// others are linked, as on the desktop; only those are held.
    fn spawn_browser_index(&mut self, id: DrillHoleId, generation: u64, files: Vec<(Columns, web_sys::File, FileIdentity)>) {
        let Some(known) = self.known_holes(id) else {
            let error = tr!(literal = "Load the drillhole dataset before linking geophysics to it");
            self.finish_geophysics_index(id, generation, None, Err(anyhow::anyhow!(error)));
            return;
        };
        let label = tr_format!(literal = "Linking geophysics to %name%", name = self.drill_hole_name(id));
        let total = files.iter().map(|(_, _, identity)| identity.size.max(1)).sum::<u64>().max(1);
        let several = files.len() > 1;
        let mut tasks = Vec::with_capacity(files.len());
        let mut held = Vec::with_capacity(files.len());
        for (columns, file, identity) in files {
            let reader = spawn_pump(file.clone(), 0..identity.size);
            held.push((identity.clone(), file));
            tasks.push((columns, identity, reader));
        }
        let compute = move |cancel: &crate::app::jobs::CancelFlag, job_progress: &crate::model::progress::Progress| -> anyhow::Result<Vec<(usize, IndexedFile)>> {
            let mut done = Vec::with_capacity(tasks.len());
            let mut failed = None;
            let mut before = 0u64;
            for (index, (columns, identity, reader)) in tasks.into_iter().enumerate() {
                let (size, name) = (identity.size, identity.name.clone());
                let source = match &columns {
                    Columns::Mapped(roles) => ColumnSource::Mapped(roles),
                    Columns::Previous(previous) => previous.as_deref().map_or(ColumnSource::Guessed, ColumnSource::Previous),
                };
                let outcome = csv_geophysics::index_file(PathBuf::from(&name), identity, source, &known, reader, &|| cancel.is_cancelled(), &mut |bytes| {
                    job_progress.set_fraction(((before + bytes) as f64 / total as f64) as f32)
                });
                before += size;
                match outcome {
                    Ok(indexed) => done.push((index, indexed)),
                    Err(CsvDrillError::Cancelled) => return Err(CsvDrillError::Cancelled.into()),
                    Err(error) => {
                        if several {
                            left_out(&name, &error.to_string());
                        }
                        failed = Some(error);
                    }
                }
            }
            match failed {
                Some(error) if done.is_empty() => Err(error.into()),
                _ => Ok(done),
            }
        };
        let apply = move |app: &mut App<'a>, result: anyhow::Result<Vec<(usize, IndexedFile)>>| {
            if let Ok(done) = &result {
                for (index, _) in done {
                    let (identity, file) = held[*index].clone();
                    app.hold_geophysics_file(identity, file, id);
                }
            }
            let files = result.map(|done| done.into_iter().map(|(_, indexed)| (None, Indexed::Read(indexed))).collect());
            app.finish_geophysics_index(id, generation, None, files);
        };
        self.spawn_job_reporting_progress(label, Self::geophysics_keys(id, generation), compute, apply);
    }

    /// Read one hole's runs from the held files as `Blob` slices, without
    /// blocking the page, then parse them on the job queue.
    pub(super) fn read_browser_hole(&mut self, id: DrillHoleId, generation: u64, link: Arc<GeophysicsLink>, dhid: String, reservation: MemoryReservation) {
        let indexed_runs: Vec<(usize, [u64; 2])> = link.runs_of(&dhid).collect();
        let mut files = Vec::with_capacity(indexed_runs.len());
        for (index, _) in &indexed_runs {
            let Some(linked) = link.files.get(*index) else { return };
            match self.geophysics_files.iter().find(|(held, _)| held.matches(&linked.identity)) {
                Some((_, file)) => files.push(file.clone()),
                None => {
                    let file = linked.identity.name.clone();
                    self.well_logs.set_state(id, generation, LinkState::NeedsPick { file });
                    return;
                }
            }
        }
        let Some(proxy) = self.web_event_loop_proxy.clone() else { return };
        let runs: Vec<[u64; 2]> = indexed_runs.into_iter().map(|(_, run)| run).collect();
        wasm_bindgen_futures::spawn_local(async move {
            let mut bytes = Vec::with_capacity(files.len());
            let mut failed = None;
            for (file, [start, end]) in files.iter().zip(&runs) {
                match crate::model::input::read_browser_range(file, *start, *end).await {
                    Ok(chunk) => bytes.push(chunk),
                    Err(error) => {
                        failed = Some(error);
                        break;
                    }
                }
            }
            let result = if let Some(error) = failed { Err(error) } else { Ok(bytes) };
            let _ = proxy.send_event(crate::app::AppEvent::GeophysicsHoleRunsRead {
                dataset: id,
                generation,
                dhid,
                runs: result,
                reservation,
            });
        });
    }

    /// A hole's run bytes, back from the page thread: parse them on the job
    /// queue.
    pub(super) fn handle_geophysics_hole_runs_read(
        &mut self,
        id: DrillHoleId,
        generation: u64,
        dhid: String,
        runs: std::result::Result<Vec<Vec<u8>>, String>,
        reservation: MemoryReservation,
    ) {
        if self.well_logs.generation(id) != Some(generation) {
            return;
        }
        let runs = match runs {
            Ok(runs) => runs,
            Err(error) => {
                self.finish_hole_read(id, generation, dhid, Err(anyhow::anyhow!(error)));
                return;
            }
        };
        let Some(link) = self.geophysics_link(id) else { return };
        self.spawn_hole_parse(id, generation, link, dhid, reservation, move |_, _| Ok(runs));
    }

    /// Hold a picked file under its identity, unless an equal one is
    /// already held, then take up again every other dataset whose link
    /// was waiting for it to be picked.
    fn hold_geophysics_file(&mut self, identity: FileIdentity, file: web_sys::File, picked_for: DrillHoleId) {
        if self.geophysics_files.iter().any(|(held, _)| held.matches(&identity)) {
            return;
        }
        let waiting = self
            .drill_holes
            .iter()
            .filter(|item| item.id != picked_for && item.state.loaded)
            .filter(|item| matches!(self.well_logs.state(item.id), Some(LinkState::NeedsPick { .. })))
            .filter_map(|item| item.geophysics.clone().map(|link| (item.id, link)))
            .filter(|(_, link)| link.files.iter().any(|linked| linked.identity.matches(&identity)))
            .collect::<Vec<_>>();
        self.geophysics_files.push((identity, file));
        for (id, link) in waiting {
            self.resolve_browser_link(id, link);
        }
    }
}

/// Where a browser index pass takes a file's columns from.
enum Columns {
    /// As the bundle import mapped them.
    Mapped(Vec<CsvDrillColumnRole>),
    /// As the index being replaced found them, or guessed without one.
    Previous(Option<Vec<LinkedColumn>>),
}

/// Say that one of several geophysics files is left out of a link.
fn left_out(file: &str, error: &str) {
    userspace_warn!(
        "{}",
        tr_format!(
            literal = "%file% was left out of the downhole geophysics: %error%",
            file = file.to_owned(),
            error = error.to_owned()
        )
    );
}

/// A file's identity: its size and modified time, and an `XxHash64` of its
/// head and tail, read as two `Blob` slices so the whole file is never held.
async fn identify_browser_file(file: &web_sys::File) -> std::result::Result<FileIdentity, String> {
    let size_f = file.size();
    if !size_f.is_finite() || size_f < 0.0 {
        return Err(tr_format!(literal = "%name% has no readable size", name = file.name()));
    }
    let size = size_f as u64;
    let [head_range, tail_range] = FileIdentity::end_ranges(size);
    let head = crate::model::input::read_browser_range(file, head_range.start, head_range.end).await?;
    let tail = crate::model::input::read_browser_range(file, tail_range.start, tail_range.end).await?;
    let modified = Some(file.last_modified() as i64);
    Ok(FileIdentity::new(file.name(), size, modified, &head, &tail))
}

/// Reads chunks pulled from a `Receiver`, fed by a page-thread pump. Runs on
/// the worker thread; `recv` blocks it until the next chunk arrives (or the
/// pump ends the stream by dropping its sender).
struct ChunkReader {
    rx: mpsc::Receiver<std::result::Result<Vec<u8>, String>>,
    in_flight: Arc<AtomicUsize>,
    buf: Vec<u8>,
    pos: usize,
}

impl Read for ChunkReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.buf.len() {
            match self.rx.recv() {
                Ok(Ok(chunk)) => {
                    self.in_flight.fetch_sub(1, Ordering::SeqCst);
                    self.buf = chunk;
                    self.pos = 0;
                }
                Ok(Err(message)) => return Err(std::io::Error::other(message)),
                // The pump ended the stream (end of file, or it gave up
                // after this reader was dropped by a cancelled job).
                Err(_disconnected) => return Ok(0),
            }
        }
        let available = self.buf.len() - self.pos;
        let n = out.len().min(available);
        out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Starts the page-thread pump for `range` of `file` and returns the
/// worker-side reader. The pump reads ahead of the reader up to
/// [`PUMP_MAX_IN_FLIGHT`] chunks, waiting on a timer otherwise, so a fast
/// worker cannot make the page hold much more than that of the file at
/// once.
fn spawn_pump(file: web_sys::File, range: std::ops::Range<u64>) -> BufReader<ChunkReader> {
    let (tx, rx) = mpsc::channel::<std::result::Result<Vec<u8>, String>>();
    let in_flight = Arc::new(AtomicUsize::new(0));
    let pump_in_flight = Arc::clone(&in_flight);
    wasm_bindgen_futures::spawn_local(async move {
        let mut pos = range.start;
        while pos < range.end {
            while pump_in_flight.load(Ordering::SeqCst) >= PUMP_MAX_IN_FLIGHT {
                // A reader dropped by a cancelled job never drains the
                // count, so the pump would otherwise wait forever.
                if Arc::strong_count(&pump_in_flight) == 1 || delay(PUMP_BACKPRESSURE_MS).await.is_err() {
                    return;
                }
            }
            let next = (pos + PUMP_CHUNK_BYTES).min(range.end);
            match crate::model::input::read_browser_range(&file, pos, next).await {
                Ok(bytes) => {
                    pump_in_flight.fetch_add(1, Ordering::SeqCst);
                    if tx.send(Ok(bytes)).is_err() {
                        return;
                    }
                }
                Err(message) => {
                    let _ = tx.send(Err(message));
                    return;
                }
            }
            pos = next;
        }
        // The sender drops here: the reader sees end of file.
    });
    BufReader::with_capacity(
        PUMP_BUFFER_BYTES,
        ChunkReader {
            rx,
            in_flight,
            buf: Vec::new(),
            pos: 0,
        },
    )
}

/// Waits about `ms` milliseconds, via a `setTimeout` wrapped in a promise.
async fn delay(ms: i32) -> std::result::Result<(), ()> {
    let window = web_sys::window().ok_or(())?;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.map(|_| ()).map_err(|_| ())
}
