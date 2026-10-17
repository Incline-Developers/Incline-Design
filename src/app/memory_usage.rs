//! How much of the machine's memory is in use, and how busy each processor
//! core is, for the readouts in the status bar.
//!
//! Sampled on a timer rather than every frame: reading either is a system
//! call (a `/proc` read on Linux), and the readouts are something to glance
//! at, not to watch tick.

use web_time::{Duration, Instant};

/// How often the readout is refreshed.
pub(crate) const SAMPLE_PERIOD: Duration = Duration::from_secs(2);

/// One reading: what is in use and the capacity it is measured against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemoryUsage {
    /// Bytes in use. Natively, by the whole system - every process, not just
    /// this one. A browser does not say what the machine is doing, so there
    /// it is the size of the app's own wasm heap.
    pub(crate) used: u64,
    /// The machine's physical memory natively; in the browser, the wasm32
    /// address space the heap can grow to.
    pub(crate) total: u64,
}

impl MemoryUsage {
    pub(crate) fn fraction(self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.used as f64 / self.total as f64).clamp(0.0, 1.0) as f32
        }
    }
}

/// Takes readings no more often than [`SAMPLE_PERIOD`].
pub(crate) struct MemorySampler {
    #[cfg(not(target_arch = "wasm32"))]
    system: sysinfo::System,
    last: Option<Instant>,
}

impl MemorySampler {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            system: sysinfo::System::new(),
            last: None,
        }
    }

    /// A fresh reading once the period has run out since the last, `None`
    /// in between or when the platform cannot say.
    pub(crate) fn poll(&mut self) -> Option<MemoryUsage> {
        let now = Instant::now();
        if self.last.is_some_and(|last| now.duration_since(last) < SAMPLE_PERIOD) {
            return None;
        }
        self.last = Some(now);
        self.read()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read(&mut self) -> Option<MemoryUsage> {
        self.system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
        let total = self.system.total_memory();
        let used = self.system.used_memory().min(total);
        (total > 0).then_some(MemoryUsage { used, total })
    }

    #[cfg(target_arch = "wasm32")]
    fn read(&mut self) -> Option<MemoryUsage> {
        use wasm_bindgen::JsCast;
        // The memory is imported and shared between the workers, so its
        // buffer is a SharedArrayBuffer rather than an ArrayBuffer.
        let memory = wasm_bindgen::memory().dyn_into::<js_sys::WebAssembly::Memory>().ok()?;
        let buffer = memory.buffer();
        let used = match buffer.dyn_ref::<js_sys::SharedArrayBuffer>() {
            Some(shared) => shared.byte_length(),
            None => buffer.dyn_ref::<js_sys::ArrayBuffer>()?.byte_length(),
        };
        // `--max-memory` in `.cargo/config.toml`: the whole wasm32 address space.
        const ADDRESS_SPACE: u64 = 4 * 1024 * 1024 * 1024;
        Some(MemoryUsage {
            used: u64::from(used),
            total: ADDRESS_SPACE,
        })
    }
}

/// How often the per-core readout is refreshed: quicker than memory, which
/// creeps, since processor load comes and goes in bursts.
pub(crate) const CPU_SAMPLE_PERIOD: Duration = Duration::from_secs(1);

/// Takes per-core processor readings no more often than [`CPU_SAMPLE_PERIOD`].
/// Native only: a browser says how many cores there are, never how busy.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct CpuSampler {
    system: sysinfo::System,
    last: Option<Instant>,
}

#[cfg(not(target_arch = "wasm32"))]
impl CpuSampler {
    pub(crate) fn new() -> Self {
        Self {
            system: sysinfo::System::new(),
            last: None,
        }
    }

    /// Each core's load, `0.0..=1.0`, once the period has run out since the
    /// last reading; `None` in between. Load is measured between two
    /// refreshes, so the first reading is all zeros.
    pub(crate) fn poll(&mut self) -> Option<Vec<f32>> {
        let now = Instant::now();
        if self.last.is_some_and(|last| now.duration_since(last) < CPU_SAMPLE_PERIOD) {
            return None;
        }
        self.last = Some(now);
        self.system.refresh_cpu_usage();
        Some(self.system.cpus().iter().map(|cpu| (cpu.cpu_usage() / 100.0).clamp(0.0, 1.0)).collect())
    }
}
