//! Temporary backing for unloaded project data. Handles own storage, never bytes.
//! Reads and writes run on workers; dropping the last handle removes the copy.

use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context;
use anyhow::Result;

#[derive(Clone, Debug)]
pub(crate) struct Backing(Arc<BackingFile>);

/// What [`Backing::open`] reads from.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type BackingData = std::fs::File;
#[cfg(target_arch = "wasm32")]
pub(crate) type BackingData = Vec<u8>;

#[derive(Debug)]
struct BackingFile {
    #[cfg(not(target_arch = "wasm32"))]
    path: tempfile::TempPath,
    #[cfg(target_arch = "wasm32")]
    key: String,
    #[cfg(target_arch = "wasm32")]
    len: usize,
}

impl Backing {
    /// A new backing file for `write` to fill, without its contents ever being
    /// in memory whole.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn write_with(write: impl FnOnce(&mut std::fs::File) -> Result<()>) -> Result<Self> {
        use std::io::Write;
        let mut file = tempfile::Builder::new()
            .prefix("asset-")
            .tempfile_in(native::directory()?)
            .context("create asset backing file")?;
        write(file.as_file_mut())?;
        file.flush().context("flush asset backing file")?;
        Ok(Self(Arc::new(BackingFile { path: file.into_temp_path() })))
    }

    /// A private copy of the file at `path`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_of(path: &std::path::Path) -> Result<Self> {
        Self::write_with(|file| {
            std::io::copy(&mut std::fs::File::open(path).with_context(|| format!("read {}", path.display()))?, file).with_context(|| format!("read {}", path.display()))?;
            Ok(())
        })
    }

    pub(crate) fn write(bytes: &[u8]) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::io::Write;
            Self::write_with(|file| file.write_all(bytes).context("write asset backing file"))
        }
        #[cfg(target_arch = "wasm32")]
        {
            let key = uuid::Uuid::new_v4().to_string();
            browser::write(&key, bytes).map_err(|error| anyhow::anyhow!("store unloaded asset: {error:?}"))?;
            Ok(Self(Arc::new(BackingFile { key, len: bytes.len() })))
        }
    }

    /// Open the copy for reading parts of it. Natively that is the file
    /// itself, so pulling one array out doesn't read the whole archive; the
    /// browser store can only hand back everything.
    pub(crate) fn open(&self) -> Result<BackingData> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::File::open(&self.0.path).context("open unloaded asset")
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.read()
        }
    }

    /// Whether both handles name the same copy.
    pub(crate) fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn read(&self) -> Result<Vec<u8>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            std::fs::read(&self.0.path).context("read unloaded asset")
        }
        #[cfg(target_arch = "wasm32")]
        {
            browser::read(&self.0.key, self.0.len).map_err(|error| anyhow::anyhow!("read unloaded asset: {error:?}"))
        }
    }
}

/// Claim this run's backing folder and clear out the ones runs that died left
/// behind. Call once at startup, off the UI thread: the first unload waits
/// for it, and clearing a crashed run's copies can take a moment.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn initialize_native() {
    if let Err(error) = native::directory() {
        log::warn!("asset backing unavailable: {error:#}");
    }
}

/// Backing files live in a folder per run, under the application cache rather
/// than /tmp, which is commonly RAM-backed: unloading into tmpfs would keep the
/// same memory pressure. Each copy deletes itself when its last handle drops,
/// but a run that crashes, is killed or exits with work in flight never drops
/// them. So each run holds a lock on a file in its folder for as long as it
/// lives (the OS releases it however the process ends), and the next run to
/// start deletes every folder whose lock it can take.
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{
        fs::{self, File, OpenOptions},
        path::{Path, PathBuf},
        sync::OnceLock,
    };

    use anyhow::{Context, Result, anyhow};

    struct Session {
        directory: PathBuf,
        /// Locked for the life of the process; never read.
        _lock: File,
    }

    static SESSION: OnceLock<Result<Session, String>> = OnceLock::new();

    /// This run's folder, claimed on first use.
    pub(super) fn directory() -> Result<&'static Path> {
        match SESSION.get_or_init(|| claim().map_err(|error| format!("{error:#}"))) {
            Ok(session) => Ok(&session.directory),
            Err(error) => Err(anyhow!("{error}")),
        }
    }

    fn claim() -> Result<Session> {
        let root = dirs::cache_dir().context("locate asset cache directory")?.join("Incline_Design").join("unloaded-assets");
        fs::create_dir_all(&root).context("create asset cache directory")?;
        // Held while claiming and sweeping, so no run sweeps a folder another
        // has made but not yet locked. Locking is best effort throughout: on a
        // filesystem without it nothing is ever swept, which is the old
        // behaviour.
        let guard = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(root.join(".lock"))
            .context("open asset cache lock")?;
        let _ = guard.lock();
        let directory = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(&root)
            .context("create asset session directory")?
            .keep();
        let lock = File::create(directory.join("lock")).context("create asset session lock")?;
        let _ = lock.try_lock();
        sweep(&root, &directory);
        Ok(Session { directory, _lock: lock })
    }

    /// Delete everything in `root` but this run's folder, the guard and the
    /// folders of runs still alive. Loose files are copies from before
    /// backing folders were per run.
    fn sweep(root: &Path, own: &Path) {
        let Ok(entries) = fs::read_dir(root) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path == own || entry.file_name() == ".lock" {
                continue;
            }
            let removed = if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                if alive(&path) {
                    continue;
                }
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            if let Err(error) = removed {
                log::warn!("could not remove stale asset backing {}: {error}", path.display());
            }
        }
    }

    /// Whether a running process still holds the folder's lock. A folder with
    /// no lock file is a dead run's (the lock is made before anything else
    /// goes in); one whose lock cannot be checked is left alone.
    fn alive(directory: &Path) -> bool {
        match File::open(directory.join("lock")) {
            Ok(file) => !matches!(file.try_lock(), Ok(())),
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn initialize_browser() {
    browser::initialize();
}

#[cfg(target_arch = "wasm32")]
impl Drop for BackingFile {
    fn drop(&mut self) {
        browser::queue_remove(self.key.clone());
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn poll_browser() {
    browser::poll();
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::sync::{Mutex, atomic::AtomicI32};

    use wasm_bindgen::prelude::*;

    // Rayon workers cannot service JavaScript events while executing Rust.
    // A broker created on the UI thread polls a mailbox in shared WASM memory
    // and performs asynchronous IndexedDB I/O. Only compute workers wait.
    static MAILBOX: [AtomicI32; 6] = [const { AtomicI32::new(0) }; 6];
    static IO: Mutex<()> = Mutex::new(());
    static REMOVALS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    pub(super) fn initialize() {
        initialize_storage(wasm_bindgen::memory(), MAILBOX.as_ptr() as u32);
    }
    pub(super) fn write(key: &str, bytes: &[u8]) -> Result<(), JsValue> {
        if !on_worker() {
            return Err(JsValue::from_str("Asset I/O must run on a worker"));
        }
        let _guard = IO.lock().unwrap();
        exchange(
            wasm_bindgen::memory(),
            MAILBOX.as_ptr() as u32,
            1,
            key.as_ptr() as u32,
            key.len(),
            bytes.as_ptr() as u32,
            bytes.len(),
        )
    }
    pub(super) fn read(key: &str, len: usize) -> Result<Vec<u8>, JsValue> {
        if !on_worker() {
            return Err(JsValue::from_str("Asset I/O must run on a worker"));
        }
        let _guard = IO.lock().unwrap();
        let mut bytes = vec![0; len];
        exchange(
            wasm_bindgen::memory(),
            MAILBOX.as_ptr() as u32,
            2,
            key.as_ptr() as u32,
            key.len(),
            bytes.as_mut_ptr() as u32,
            len,
        )?;
        Ok(bytes)
    }
    pub(super) fn queue_remove(key: String) {
        REMOVALS.lock().unwrap().push(key);
    }
    pub(super) fn poll() {
        let keys = if let Ok(mut pending) = REMOVALS.try_lock() {
            std::mem::take(&mut *pending)
        } else {
            return;
        };
        for key in keys {
            remove(&key);
        }
    }

    #[wasm_bindgen(inline_js = r#"
let storageWorker;
export function initializeAssetStorage(memory, address) {
    if (storageWorker) return;
    const source = `
        let dbPromise;
        function database() {
            return dbPromise ||= new Promise((resolve, reject) => {
                const request = indexedDB.open('incline-asset-cache', 1);
                request.onupgradeneeded = () => request.result.createObjectStore('assets');
                request.onsuccess = () => resolve(request.result);
                request.onerror = () => reject(request.error);
            });
        }
        async function transact(op, key, bytes) {
            const db = await database();
            return new Promise((resolve, reject) => {
                const tx = db.transaction('assets', op === 2 ? 'readonly' : 'readwrite');
                let result;
                tx.oncomplete = () => resolve(result);
                tx.onerror = tx.onabort = () => reject(tx.error || new Error('Asset storage transaction failed'));
                const store = tx.objectStore('assets');
                if (op === 1) store.put(bytes, key);
                else if (op === 2) { const request = store.get(key); request.onsuccess = () => { result = request.result; }; }
                else store.delete(key);
            });
        }
        onmessage = async ({data}) => {
            if (data.remove) { await transact(3, data.remove); return; }
            const {memory, address} = data;
            const status = new Int32Array(memory.buffer, address, 6);
            for (;;) {
                if (Atomics.compareExchange(status, 0, 1, 3) === 1) {
                    try {
                        const op = status[1], ptr = status[4] >>> 0, len = status[5] >>> 0;
                        const key = new TextDecoder().decode(new Uint8Array(memory.buffer, status[2] >>> 0, status[3] >>> 0).slice());
                        // Copy before awaiting: shared WASM memory can grow.
                        const bytes = op === 1 ? new Uint8Array(memory.buffer, ptr, len).slice() : undefined;
                        const result = await transact(op, key, bytes);
                        if (op === 2) {
                            if (!result || result.length !== len) throw new Error('Missing asset backing');
                            new Uint8Array(memory.buffer, ptr, len).set(result);
                        }
                        Atomics.store(status, 0, 2);
                    } catch { Atomics.store(status, 0, -1); }
                    Atomics.notify(status, 0);
                }
                await new Promise(resolve => setTimeout(resolve, 10));
            }
        };`;
    const url = URL.createObjectURL(new Blob([source], {type: 'text/javascript'}));
    storageWorker = new Worker(url);
    storageWorker.onerror = () => {
        const status = new Int32Array(memory.buffer, address, 6);
        Atomics.store(status, 0, -2); Atomics.notify(status, 0);
    };
    storageWorker.postMessage({memory, address});
    URL.revokeObjectURL(url);
}
export function onAssetWorker() { return typeof document === 'undefined'; }
export function exchangeAsset(memory, address, op, key, keyLen, ptr, len) {
    const status = new Int32Array(memory.buffer, address, 6);
    if (Atomics.load(status, 0) === -2) throw new Error('Asset storage worker failed');
    status.set([op, key, keyLen, ptr, len], 1);
    Atomics.store(status, 0, 1);
    const deadline = Date.now() + 60000;
    for (;;) {
        const state = Atomics.load(status, 0);
        if (state !== 1 && state !== 3) break;
        // Cancel only an unclaimed request. Once claimed, keep its WASM
        // pointers alive until the broker finishes accessing them.
        if (Date.now() > deadline && Atomics.compareExchange(status, 0, 1, 0) === 1) throw new Error('Asset storage worker did not start');
        Atomics.wait(status, 0, state, 100);
    }
    const state = Atomics.load(status, 0);
    if (state !== -2) Atomics.store(status, 0, 0);
    if (state !== 2) throw new Error('Asset backing I/O failed (missing data, storage unavailable, or quota exceeded)');
}
export function removeAsset(key) { storageWorker.postMessage({remove: key}); }
"#)]
    extern "C" {
        #[wasm_bindgen(js_name = initializeAssetStorage)]
        fn initialize_storage(memory: JsValue, address: u32);
        #[wasm_bindgen(js_name = onAssetWorker)]
        fn on_worker() -> bool;
        #[wasm_bindgen(catch, js_name = exchangeAsset)]
        fn exchange(memory: JsValue, address: u32, op: u32, key: u32, key_len: usize, ptr: u32, len: usize) -> Result<(), JsValue>;
        #[wasm_bindgen(js_name = removeAsset)]
        fn remove(key: &str);
    }
}
