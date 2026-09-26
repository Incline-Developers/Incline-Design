//! Downhole geophysics linked to drill-hole datasets: linking a file,
//! taking up a saved link, and reading a hole when the log shows it. The
//! readings are never all in memory; see [`crate::model::geophysics`].
//! The browser's side of each step is in `geophysics_web`.

use std::{collections::HashSet, sync::Arc};

use crate::{
    app::{App, jobs::JobKey, memory::MemoryReservation},
    i18n::tr_format,
    model::{
        drill_hole::DrillHoleId,
        formats::csv_geophysics::{self, IndexedFile},
        geophysics::{GeophysicsLink, HoleLogs, LinkState, LinkedFile, MAX_HOLE_BYTES, reserve_hole},
    },
    userspace_warn,
};

/// What reading a hole holds at its peak, as a multiple of its rows' bytes:
/// the bytes, their readings as f64, and a curve's readings paired with
/// their depths while it settles into a trace. A cell of a few bytes becomes
/// eight or sixteen.
const HOLE_READ_FACTOR: u64 = 6;

/// A linked file found changed while a hole was read from it. The browser
/// holds a picked file as it was, so only the desktop sees one change.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub(crate) struct FileChanged;

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Display for FileChanged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::i18n::tr!(literal = "The geophysics file changed since it was indexed"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::error::Error for FileChanged {}

/// One file of an index job's result: indexed afresh, or an unchanged
/// file's index kept.
pub(crate) enum Indexed {
    Read(IndexedFile),
    /// An unchanged file's index, kept. Desktop only: the browser keeps one
    /// before any job starts.
    #[cfg(not(target_arch = "wasm32"))]
    Kept(LinkedFile),
}

impl Indexed {
    fn into_file(self) -> LinkedFile {
        match self {
            Indexed::Read(indexed) => {
                indexed.report.log(&indexed.file);
                indexed.file
            }
            #[cfg(not(target_arch = "wasm32"))]
            Indexed::Kept(file) => file,
        }
    }
}

impl<'a> App<'a> {
    /// Take up the links the session has not seen: a dataset opened, loaded
    /// or linked since. Runs once a frame and does nothing when nothing
    /// changed.
    pub(crate) fn sync_geophysics(&mut self) {
        for id in self.well_logs.sync(&self.drill_holes) {
            self.check_geophysics_link(id);
        }
    }

    /// The link of a loaded dataset.
    pub(super) fn geophysics_link(&self, id: DrillHoleId) -> Option<Arc<GeophysicsLink>> {
        self.drill_holes.iter().find(|item| item.id == id && item.state.loaded)?.geophysics.clone()
    }

    pub(super) fn drill_hole_name(&self, id: DrillHoleId) -> String {
        self.drill_holes.iter().find(|item| item.id == id).map(|item| item.name.clone()).unwrap_or_default()
    }

    /// The hole ids of a loaded dataset, which a file is indexed against.
    pub(super) fn known_holes(&self, id: DrillHoleId) -> Option<HashSet<String>> {
        let dataset = self.drill_holes.iter().find(|item| item.id == id && item.state.loaded)?;
        Some(dataset.dataset.holes.iter().map(|hole| hole.dhid.clone()).collect())
    }

    /// Start over on a dataset's geophysics: work under the old generation
    /// is cancelled, and the session takes up `link` in `state`. Returns
    /// the generation new work runs under.
    pub(super) fn restart_geophysics(&mut self, id: DrillHoleId, link: Option<&Arc<GeophysicsLink>>, state: LinkState) -> u64 {
        self.cancel_jobs(|key| matches!(key, JobKey::Geophysics { dataset, .. } if *dataset == id));
        let placeholder;
        let link = match link {
            Some(link) => link,
            None => {
                placeholder = Arc::new(GeophysicsLink { files: Vec::new() });
                &placeholder
            }
        };
        self.redraw_requested = true;
        self.well_logs.adopt(id, link, state)
    }

    pub(super) fn geophysics_keys(id: DrillHoleId, generation: u64) -> Vec<JobKey> {
        vec![JobKey::DrillHole(id), JobKey::Geophysics { dataset: id, generation }]
    }

    /// Keep `link` as the dataset's, to be saved with it, and read from it.
    pub(super) fn keep_geophysics_link(&mut self, id: DrillHoleId, link: GeophysicsLink) {
        let link = Arc::new(link);
        let Some(dataset) = self.drill_holes.iter_mut().find(|item| item.id == id) else {
            return;
        };
        dataset.geophysics = Some(Arc::clone(&link));
        dataset.state.touch();
        self.touch_active_project_content();
        self.restart_geophysics(id, Some(&link), LinkState::Ready);
    }

    /// An index job's result on the UI thread: `files` replace those of
    /// `base` they stand for, or make a new link when there is no base.
    pub(super) fn finish_geophysics_index(&mut self, id: DrillHoleId, generation: u64, base: Option<Arc<GeophysicsLink>>, result: anyhow::Result<Vec<(Option<usize>, Indexed)>>) {
        if self.well_logs.generation(id) != Some(generation) {
            return;
        }
        match result {
            Ok(files) => {
                let mut link = base.map_or_else(|| GeophysicsLink { files: Vec::new() }, |base| (*base).clone());
                for (replaces, indexed) in files {
                    let file = indexed.into_file();
                    match replaces.and_then(|index| link.files.get_mut(index)) {
                        Some(slot) => *slot = file,
                        None => link.files.push(file),
                    }
                }
                self.keep_geophysics_link(id, link);
            }
            Err(error) => {
                let error = format!("{error:#}");
                userspace_warn!(
                    "{}",
                    tr_format!(
                        literal = "Downhole geophysics for '%name%' could not be linked: %error%",
                        name = self.drill_hole_name(id),
                        error = error.clone()
                    )
                );
                self.well_logs.set_state(id, generation, LinkState::Failed(error));
                // A relink that failed leaves the dataset's link as it was,
                // to be read as before.
                if base.is_none() && self.geophysics_link(id).is_some() {
                    self.check_geophysics_link(id);
                }
            }
        }
    }

    /// Start using a saved or restored link: its files must be there and
    /// unchanged before a hole is read from them.
    fn check_geophysics_link(&mut self, id: DrillHoleId) {
        let Some(link) = self.geophysics_link(id) else {
            return;
        };
        #[cfg(not(target_arch = "wasm32"))]
        self.check_native_link(id, link);
        #[cfg(target_arch = "wasm32")]
        self.resolve_browser_link(id, link);
    }

    /// Read a hole the log wants from the dataset's linked files, off the
    /// UI thread. Each hole is asked for once, until it is read or fails.
    pub(crate) fn read_hole_geophysics(&mut self, id: DrillHoleId, dhid: String) {
        let Some(dataset) = self.drill_holes.iter().find(|item| item.id == id && item.state.loaded) else {
            return;
        };
        let Some(link) = dataset.geophysics.clone() else {
            return;
        };
        let Some(generation) = self.well_logs.begin_read(dataset, &dhid) else {
            return;
        };
        let bytes = link.runs_of(&dhid).map(|(_, [start, end])| end.saturating_sub(start)).sum::<u64>();
        if bytes > MAX_HOLE_BYTES {
            let error = tr_format!(
                literal = "%hole% has %size% MiB of geophysics rows, more than a hole is read at",
                hole = dhid.clone(),
                size = bytes / (1024 * 1024)
            );
            self.well_logs.finish_read(id, generation, dhid, Err(error));
            return;
        }
        let reservation = match reserve_hole(&dhid, usize::try_from(bytes.saturating_mul(HOLE_READ_FACTOR)).unwrap_or(usize::MAX)) {
            Ok(reservation) => reservation,
            Err(error) => {
                self.well_logs.finish_read(id, generation, dhid, Err(error));
                return;
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        self.spawn_hole_parse(id, generation, link, dhid, reservation, native::read_runs);
        #[cfg(target_arch = "wasm32")]
        self.read_browser_hole(id, generation, link, dhid, reservation);
    }

    /// Parse a hole's runs on the job queue, `runs` giving their bytes
    /// there: read from disk, or handed over read already. `reservation`
    /// covers the bytes and the parse, and is let go when the parse ends.
    pub(super) fn spawn_hole_parse(
        &mut self,
        id: DrillHoleId,
        generation: u64,
        link: Arc<GeophysicsLink>,
        dhid: String,
        reservation: MemoryReservation,
        runs: impl FnOnce(&GeophysicsLink, &str) -> anyhow::Result<Vec<Vec<u8>>> + Send + 'static,
    ) {
        let label = tr_format!(literal = "Reading geophysics for %hole%", hole = dhid.clone());
        let hole = dhid.clone();
        let compute = move |_: &crate::app::jobs::CancelFlag| -> anyhow::Result<HoleLogs> {
            let _reservation = reservation;
            let runs = runs(&link, &hole)?;
            Ok(csv_geophysics::read_hole(&link, &hole, &runs)?)
        };
        let apply = move |app: &mut App<'a>, result| app.finish_hole_read(id, generation, dhid, result);
        self.spawn_job(label, Self::geophysics_keys(id, generation), compute, apply);
    }

    /// A hole read's result on the UI thread. A file found changed is
    /// checked again, and indexed again if it has.
    pub(super) fn finish_hole_read(&mut self, id: DrillHoleId, generation: u64, dhid: String, result: anyhow::Result<HoleLogs>) {
        #[cfg(not(target_arch = "wasm32"))]
        let changed = result.as_ref().is_err_and(|error| error.downcast_ref::<FileChanged>().is_some());
        if let Ok(logs) = &result {
            log::debug!("geophysics: {} curve(s) read for {dhid}", logs.traces().len());
        }
        self.well_logs.finish_read(id, generation, dhid, result.map_err(|error| format!("{error:#}")));
        log::debug!("geophysics: {} bytes of holes held", self.well_logs.memory_bytes());
        #[cfg(not(target_arch = "wasm32"))]
        if changed && self.well_logs.generation(id) == Some(generation) {
            self.check_geophysics_link(id);
        }
        self.redraw_requested = true;
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{
        io::{Read, Seek, SeekFrom},
        path::PathBuf,
        sync::Arc,
    };

    use super::{FileChanged, Indexed};
    use crate::{
        app::App,
        i18n::{tr, tr_format},
        model::{
            drill_hole::DrillHoleId,
            formats::csv_geophysics::{self, ColumnSource},
            geophysics::{FileIdentity, GeophysicsLink, LinkState, LinkedColumn, LinkedFile},
        },
        userspace_log, userspace_warn,
    };

    /// What a check found of one linked file.
    enum FileCheck {
        Unchanged,
        Changed,
        Unreadable(String),
    }

    /// A file for an index job: where it is, what it replaces in the link,
    /// an index to keep if the file is unchanged, and the columns an
    /// earlier index found.
    struct IndexTask {
        path: PathBuf,
        replaces: Option<usize>,
        reuse: Option<LinkedFile>,
        previous: Option<Vec<LinkedColumn>>,
    }

    impl<'a> App<'a> {
        /// Make sure each linked file is where the link says and unchanged;
        /// a changed file is indexed again.
        pub(super) fn check_native_link(&mut self, id: DrillHoleId, link: Arc<GeophysicsLink>) {
            let generation = self.restart_geophysics(id, Some(&link), LinkState::Checking);
            let files = link.files.iter().map(|file| (file.path.clone(), file.identity.clone())).collect::<Vec<_>>();
            let compute = move |_: &crate::app::jobs::CancelFlag| {
                Ok(files
                    .into_iter()
                    .map(|(path, identity)| match FileIdentity::of_path(&path) {
                        Ok(found) if found.matches(&identity) => FileCheck::Unchanged,
                        Ok(_) => FileCheck::Changed,
                        Err(error) => FileCheck::Unreadable(error.to_string()),
                    })
                    .collect::<Vec<_>>())
            };
            let apply = move |app: &mut App<'a>, result: anyhow::Result<Vec<FileCheck>>| app.finish_link_check(id, generation, link, result);
            self.spawn_job(tr!(literal = "Checking geophysics files"), Self::geophysics_keys(id, generation), compute, apply);
        }

        fn finish_link_check(&mut self, id: DrillHoleId, generation: u64, link: Arc<GeophysicsLink>, result: anyhow::Result<Vec<FileCheck>>) {
            let checks = match result {
                Ok(checks) => checks,
                Err(error) => {
                    self.well_logs.set_state(id, generation, LinkState::Failed(format!("{error:#}")));
                    return;
                }
            };
            let name = self.drill_hole_name(id);
            if let Some((file, error)) = link.files.iter().zip(&checks).find_map(|(file, check)| match check {
                FileCheck::Unreadable(error) => Some((file, error)),
                _ => None,
            }) {
                userspace_warn!(
                    "{}",
                    tr_format!(
                        literal = "The geophysics file linked to '%name%' cannot be read at %path% (%error%); link it again from the dataset's right-click menu",
                        name = name,
                        path = file.path.display().to_string(),
                        error = error.clone()
                    )
                );
                let file = file.identity.name.clone();
                self.well_logs.set_state(id, generation, LinkState::Missing { file });
                return;
            }
            let tasks = link
                .files
                .iter()
                .zip(&checks)
                .enumerate()
                .filter(|(_, (_, check))| matches!(check, FileCheck::Changed))
                .map(|(index, (file, _))| IndexTask {
                    path: file.path.clone(),
                    replaces: Some(index),
                    reuse: None,
                    previous: Some(file.columns.clone()),
                })
                .collect::<Vec<_>>();
            if tasks.is_empty() {
                self.well_logs.set_state(id, generation, LinkState::Ready);
                return;
            }
            userspace_log!(
                "{}",
                tr_format!(literal = "The geophysics linked to '%name%' changed since it was indexed; reading it again", name = name)
            );
            self.spawn_native_index(id, Some(link), tasks);
        }

        /// Link the files at `paths`, in that order, to a dataset in place
        /// of any link it has, keeping the old index when one file is picked
        /// and it is the same file unchanged.
        pub(crate) fn link_geophysics_paths(&mut self, id: DrillHoleId, paths: Vec<PathBuf>) {
            let Some(current) = self.drill_holes.iter().find(|item| item.id == id).map(|item| item.geophysics.clone()) else {
                return;
            };
            let previous = current.as_ref().and_then(|link| link.files.first()).map(|file| file.columns.clone());
            // Only a one-file link is kept whole: a pick replaces them all.
            let mut reuse = (paths.len() == 1)
                .then(|| current.as_ref().filter(|link| link.files.len() == 1).map(|link| link.files[0].clone()))
                .flatten();
            let tasks = paths
                .into_iter()
                .map(|path| IndexTask {
                    path,
                    replaces: None,
                    reuse: reuse.take(),
                    previous: previous.clone(),
                })
                .collect();
            self.spawn_native_index(id, None, tasks);
        }

        /// Index `tasks` against the dataset's holes on the job queue.
        fn spawn_native_index(&mut self, id: DrillHoleId, base: Option<Arc<GeophysicsLink>>, tasks: Vec<IndexTask>) {
            let Some(known) = self.known_holes(id) else {
                return;
            };
            let current = self.geophysics_link(id);
            let generation = self.restart_geophysics(id, current.as_ref(), LinkState::Indexing);
            let label = tr_format!(literal = "Linking geophysics to %name%", name = self.drill_hole_name(id));
            let compute = move |cancel: &crate::app::jobs::CancelFlag, progress: &crate::model::progress::Progress| {
                let mut done = Vec::new();
                for task in tasks {
                    let (identity, input) = csv_geophysics::open_for_index(&task.path)?;
                    if let Some(mut kept) = task.reuse.filter(|file| file.identity.matches(&identity)) {
                        kept.path = task.path;
                        kept.identity = identity;
                        done.push((task.replaces, Indexed::Kept(kept)));
                        continue;
                    }
                    let size = identity.size.max(1);
                    let columns = task.previous.as_deref().map_or(ColumnSource::Guessed, ColumnSource::Previous);
                    let indexed = csv_geophysics::index_file(task.path, identity, columns, &known, input, &|| cancel.is_cancelled(), &mut |bytes| {
                        progress.set_fraction((bytes as f64 / size as f64) as f32)
                    })?;
                    done.push((task.replaces, Indexed::Read(indexed)));
                }
                Ok(done)
            };
            let apply = move |app: &mut App<'a>, result| app.finish_geophysics_index(id, generation, base, result);
            self.spawn_job_reporting_progress(label, Self::geophysics_keys(id, generation), compute, apply);
        }
    }

    /// The bytes of each of `dhid`'s runs, in link order. A file whose size
    /// or time no longer match its identity is not read.
    pub(crate) fn read_runs(link: &GeophysicsLink, dhid: &str) -> anyhow::Result<Vec<Vec<u8>>> {
        let mut open: Vec<Option<std::fs::File>> = link.files.iter().map(|_| None).collect();
        let mut runs = Vec::new();
        for (index, [start, end]) in link.runs_of(dhid) {
            let linked = &link.files[index];
            let file = match &mut open[index] {
                Some(file) => file,
                slot => {
                    let file = std::fs::File::open(&linked.path)?;
                    let metadata = file.metadata()?;
                    if metadata.len() != linked.identity.size || FileIdentity::modified_millis(&metadata) != linked.identity.modified {
                        return Err(FileChanged.into());
                    }
                    slot.insert(file)
                }
            };
            file.seek(SeekFrom::Start(start))?;
            let mut bytes = Vec::new();
            file.take(end.saturating_sub(start)).read_to_end(&mut bytes)?;
            runs.push(bytes);
        }
        Ok(runs)
    }
}
