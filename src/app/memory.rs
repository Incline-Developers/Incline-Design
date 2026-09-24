//! Central limits for allocations that can approach the WASM32 address space.

#[cfg(target_arch = "wasm32")]
use std::sync::Arc;
#[cfg(any(target_arch = "wasm32", test))]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_arch = "wasm32")]
pub(crate) const WORKING_SET_LIMIT: usize = 3 * 1024 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
pub(crate) const INPUT_FILE_LIMIT: usize = 2 * 1024 * 1024 * 1024;
// Ceilings on the up-front cost of building one block-model volume, not
// hardware limits: the cell backing is mmap-backed and streamed to a bounded
// GPU pool, and the GPU-side storage-buffer checks (which now see the
// adapter's real binding size, see `graphics::init`) are the true backstop,
// with a graceful cube-render fallback when they trip. Kept generous on native
// so large translucent models still reach the volume raycaster instead of the
// far slower instanced-cube path; they only bound worst-case build time and
// the temp-file/mmap footprint. `VOLUME_CELL_LIMIT` caps packed 16-bit cell
// bytes (~4M occupied bricks native); `VOLUME_METADATA_LIMIT` caps dense
// per-brick tables at ~64 B/brick (~32M total grid bricks native). On wasm32
// `usize` is 32-bit and the whole working set is 3 GiB, so both stay well
// under that (and under 4 GiB) and are further gated by `reserve`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const VOLUME_CELL_LIMIT: usize = 4 * 1024 * 1024 * 1024;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const VOLUME_METADATA_LIMIT: usize = 2 * 1024 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
pub(crate) const VOLUME_CELL_LIMIT: usize = 1024 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
pub(crate) const VOLUME_METADATA_LIMIT: usize = 512 * 1024 * 1024;
#[cfg(target_arch = "wasm32")]
pub(crate) const FILE_CHUNK_BYTES: usize = 64 * 1024 * 1024;

#[cfg(target_arch = "wasm32")]
static RESERVED: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub(crate) struct MemoryReservation {
    _reservation: Arc<Reservation>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug)]
pub(crate) struct MemoryReservation;

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
struct Reservation {
    bytes: AtomicUsize,
}

#[cfg(target_arch = "wasm32")]
impl Drop for Reservation {
    fn drop(&mut self) {
        let bytes = *self.bytes.get_mut();
        if bytes != 0 {
            RESERVED.fetch_sub(bytes, Ordering::AcqRel);
        }
    }
}

impl MemoryReservation {
    pub(crate) fn untracked() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                _reservation: Arc::new(Reservation { bytes: AtomicUsize::new(0) }),
            }
        }
    }

    /// Grow this reservation to `bytes`, refused past the budget as
    /// [`reserve`] is. A reservation already that large is left alone.
    ///
    /// Race-free against other clones of the same reservation growing or
    /// shrinking it at once: the claim against the budget and the update of
    /// this reservation's own counter are tied together with a
    /// compare-and-swap, so a losing racer gives its claim back and retries
    /// instead of leaving the budget over-claimed.
    pub(crate) fn grow_to(&self, bytes: usize, description: &str) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (bytes, description);
            Ok(())
        }
        #[cfg(target_arch = "wasm32")]
        {
            grow_counted(&RESERVED, &self._reservation.bytes, bytes, WORKING_SET_LIMIT, description)
        }
    }

    /// Shrink this reservation to `bytes`, handing the rest back to the
    /// budget. A reservation already that small is left alone.
    ///
    /// Race-free the same way as [`Self::grow_to`]: the counter update and
    /// the refund to the budget are tied together with a compare-and-swap.
    pub(crate) fn shrink_to(&self, bytes: usize) {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = bytes;
        #[cfg(target_arch = "wasm32")]
        {
            shrink_counted(&RESERVED, &self._reservation.bytes, bytes);
        }
    }
}

pub(crate) fn reserve(bytes: usize, description: &str) -> Result<MemoryReservation, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (bytes, description);
        Ok(MemoryReservation::untracked())
    }

    #[cfg(target_arch = "wasm32")]
    {
        claim_counted(&RESERVED, bytes, WORKING_SET_LIMIT, description)?;
        Ok(MemoryReservation {
            _reservation: Arc::new(Reservation { bytes: AtomicUsize::new(bytes) }),
        })
    }
}

/// Add `bytes` to `used`, or refuse them if that would push `used` past
/// `limit`. Free of the reservation type so it can run under a plain test
/// counter as well as the real wasm budget.
#[cfg(any(target_arch = "wasm32", test))]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code, reason = "the wasm budget uses these; native builds them only for tests"))]
fn claim_counted(used: &AtomicUsize, bytes: usize, limit: usize, description: &str) -> Result<(), String> {
    loop {
        let current = used.load(Ordering::Acquire);
        let requested = current.checked_add(bytes).ok_or_else(|| format!("{description} memory estimate overflows"))?;
        if requested > limit {
            return Err(format!(
                "{description} needs {bytes} bytes, but only {} bytes remain in Incline Design's 3 GiB browser working-set budget",
                limit.saturating_sub(current)
            ));
        }
        if used.compare_exchange_weak(current, requested, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            return Ok(());
        }
    }
}

/// Grow `held` to `bytes`, claiming the difference against `used`/`limit`.
/// Left alone if `held` is already at least `bytes`. Safe for two clones of
/// the same reservation to call at once: a racing clone that also grew (or
/// shrunk) `held` between the load and the claim loses the compare-and-swap,
/// gives its claim back, and retries against the new value.
#[cfg(any(target_arch = "wasm32", test))]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code, reason = "the wasm budget uses these; native builds them only for tests"))]
fn grow_counted(used: &AtomicUsize, held: &AtomicUsize, bytes: usize, limit: usize, description: &str) -> Result<(), String> {
    loop {
        let current = held.load(Ordering::Acquire);
        if bytes <= current {
            return Ok(());
        }
        claim_counted(used, bytes - current, limit, description)?;
        if held.compare_exchange(current, bytes, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            return Ok(());
        }
        used.fetch_sub(bytes - current, Ordering::AcqRel);
    }
}

/// Shrink `held` to `bytes`, refunding the difference to `used`. Left alone
/// if `held` is already at most `bytes`. Safe for concurrent callers on the
/// same reservation for the same reason as [`grow_counted`].
#[cfg(any(target_arch = "wasm32", test))]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code, reason = "the wasm budget uses these; native builds them only for tests"))]
fn shrink_counted(used: &AtomicUsize, held: &AtomicUsize, bytes: usize) {
    loop {
        let current = held.load(Ordering::Acquire);
        if bytes >= current {
            return;
        }
        if held.compare_exchange(current, bytes, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            used.fetch_sub(current - bytes, Ordering::AcqRel);
            return;
        }
    }
}
