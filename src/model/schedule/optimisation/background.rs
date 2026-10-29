//! Work Improve runs beside its solve: a relaxation bound, a background
//! plan, a watcher that interrupts SCIP. On native builds each is a thread
//! of its own. The browser cannot start threads, so there each is a task of
//! the shared rayon pool (web workers), and [`Background::join`] runs queued
//! pool tasks while it waits: a run never stalls the pool on work it is
//! itself waiting for, however few workers the browser gives it. A watcher
//! there runs only once a worker is free, so nothing may depend on it to
//! stop a solve.

use std::time::Duration;

/// Work started by [`spawn`], finished or not.
pub(crate) struct Background<T> {
    #[cfg(not(target_arch = "wasm32"))]
    handle: std::thread::JoinHandle<T>,
    #[cfg(target_arch = "wasm32")]
    slot: std::sync::Arc<std::sync::Mutex<Option<T>>>,
}

/// Start `work` beside the caller.
pub(crate) fn spawn<T: Send + 'static>(name: &str, work: impl FnOnce() -> T + Send + 'static) -> std::io::Result<Background<T>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let handle = std::thread::Builder::new().name(name.to_owned()).spawn(work)?;
        Ok(Background { handle })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        let filled = std::sync::Arc::clone(&slot);
        rayon::spawn(move || {
            let value = work();
            *filled.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(value);
        });
        Ok(Background { slot })
    }
}

impl<T: Send + 'static> Background<T> {
    /// Work that has already finished with `value`.
    pub(crate) fn done(value: T) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                handle: std::thread::spawn(move || value),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                slot: std::sync::Arc::new(std::sync::Mutex::new(Some(value))),
            }
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.handle.is_finished()
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some()
        }
    }

    /// Wait for the work and take what it returned; `None` if it panicked.
    pub(crate) fn join(self) -> Option<T> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.handle.join().ok()
        }
        #[cfg(target_arch = "wasm32")]
        {
            loop {
                if let Some(value) = self.slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() {
                    return Some(value);
                }
                // The work may still be queued behind this very task: run
                // queued work here rather than wait on a worker.
                if rayon::yield_now() != Some(rayon::Yield::Executed) {
                    pause(Duration::from_millis(1));
                }
            }
        }
    }
}

/// Run `main` here and `watch` beside it until both have returned; `watch`
/// may borrow what `main` does not.
pub(crate) fn beside<R>(watch: impl FnOnce() + Send, main: impl FnOnce() -> R) -> R {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::scope(|scope| {
            scope.spawn(watch);
            main()
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        rayon::in_place_scope(|scope| {
            scope.spawn(|_| watch());
            main()
        })
    }
}

/// Wait `duration`, as a polling loop does between looks. A web worker
/// sleeps as a thread does.
pub(crate) fn pause(duration: Duration) {
    std::thread::sleep(duration);
}
