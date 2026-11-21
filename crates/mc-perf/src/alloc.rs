//! Allocation counting for the "zero allocations per tick / per section" budgets.
//!
//! Install [`CountingAllocator`] as the binary's `#[global_allocator]`, backed by a [`Counters`]
//! value (usually [`GLOBAL_COUNTERS`]), and enable counting with `MC_ALLOC_COUNT=1` or
//! [`Counters::set_enabled`]. Disabled by default, so a normal build pays nothing. `realloc` counts
//! as a free of the old size plus an allocation of the new one (so a growing `Vec` shows as churn).
//!
//! Counters are per-[`Counters`] instance (atomics), so tests can use a private set with no shared
//! state.

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocSnapshot {
    pub allocations: u64,
    pub allocated_bytes: u64,
    pub frees: u64,
    pub freed_bytes: u64,
}

impl AllocSnapshot {
    /// Bytes allocated less bytes freed since the last reset.
    pub fn net_bytes(&self) -> i64 {
        self.allocated_bytes as i64 - self.freed_bytes as i64
    }
}

/// A set of allocation counters with its own enable flag.
pub struct Counters {
    enabled: AtomicBool,
    allocations: AtomicU64,
    allocated_bytes: AtomicU64,
    frees: AtomicU64,
    freed_bytes: AtomicU64,
}

impl Counters {
    pub const fn new() -> Self {
        Counters {
            enabled: AtomicBool::new(false),
            allocations: AtomicU64::new(0),
            allocated_bytes: AtomicU64::new(0),
            frees: AtomicU64::new(0),
            freed_bytes: AtomicU64::new(0),
        }
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.allocations.store(0, Ordering::Relaxed);
        self.allocated_bytes.store(0, Ordering::Relaxed);
        self.frees.store(0, Ordering::Relaxed);
        self.freed_bytes.store(0, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> AllocSnapshot {
        AllocSnapshot {
            allocations: self.allocations.load(Ordering::Relaxed),
            allocated_bytes: self.allocated_bytes.load(Ordering::Relaxed),
            frees: self.frees.load(Ordering::Relaxed),
            freed_bytes: self.freed_bytes.load(Ordering::Relaxed),
        }
    }

    #[inline]
    fn count_alloc(&self, size: usize) {
        if self.enabled() {
            self.allocations.fetch_add(1, Ordering::Relaxed);
            self.allocated_bytes
                .fetch_add(size as u64, Ordering::Relaxed);
        }
    }

    #[inline]
    fn count_free(&self, size: usize) {
        if self.enabled() {
            self.frees.fetch_add(1, Ordering::Relaxed);
            self.freed_bytes.fetch_add(size as u64, Ordering::Relaxed);
        }
    }
}

impl Default for Counters {
    fn default() -> Self {
        Counters::new()
    }
}

/// Process-wide counters for the production allocator.
pub static GLOBAL_COUNTERS: Counters = Counters::new();

/// Turn counting on/off on [`GLOBAL_COUNTERS`].
pub fn set_alloc_counting(on: bool) {
    GLOBAL_COUNTERS.set_enabled(on);
}

pub fn alloc_counting() -> bool {
    GLOBAL_COUNTERS.enabled()
}

/// Enable if `MC_ALLOC_COUNT=1`.
pub fn init_alloc_from_env() {
    if std::env::var("MC_ALLOC_COUNT").as_deref() == Ok("1") {
        GLOBAL_COUNTERS.set_enabled(true);
    }
}

pub fn reset_alloc_stats() {
    GLOBAL_COUNTERS.reset();
}

pub fn alloc_snapshot() -> AllocSnapshot {
    GLOBAL_COUNTERS.snapshot()
}

/// A `GlobalAlloc` wrapper that counts into a [`Counters`] set.
///
/// ```ignore
/// static COUNTERS: mc_perf::alloc::Counters = mc_perf::alloc::Counters::new();
/// #[global_allocator]
/// static A: mc_perf::alloc::CountingAllocator<std::alloc::System> =
///     mc_perf::alloc::CountingAllocator::new(std::alloc::System, &COUNTERS);
/// ```
pub struct CountingAllocator<A> {
    inner: A,
    counters: &'static Counters,
}

impl<A> CountingAllocator<A> {
    pub const fn new(inner: A, counters: &'static Counters) -> Self {
        CountingAllocator { inner, counters }
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { self.inner.alloc(layout) };
        if !p.is_null() {
            self.counters.count_alloc(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.counters.count_free(layout.size());
        unsafe { self.inner.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { self.inner.alloc_zeroed(layout) };
        if !p.is_null() {
            self.counters.count_alloc(layout.size());
        }
        p
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.counters.count_free(layout.size());
        let p = unsafe { self.inner.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            self.counters.count_alloc(new_size);
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::System;

    fn local_counters() -> &'static Counters {
        Box::leak(Box::new(Counters::new()))
    }

    #[test]
    fn counts_direct_allocation_and_free() {
        let counters = local_counters();
        counters.set_enabled(true);

        let a = CountingAllocator::new(System, counters);
        let layout = Layout::from_size_align(64, 8).unwrap();
        let ptr = unsafe { a.alloc(layout) };
        assert!(!ptr.is_null());
        unsafe { a.dealloc(ptr, layout) };

        let s = counters.snapshot();
        assert_eq!(s.allocations, 1);
        assert_eq!(s.allocated_bytes, 64);
        assert_eq!(s.frees, 1);
        assert_eq!(s.freed_bytes, 64);
        assert_eq!(s.net_bytes(), 0);
    }

    #[test]
    fn disabled_counts_nothing() {
        let counters = local_counters();
        counters.set_enabled(false);

        let a = CountingAllocator::new(System, counters);
        let layout = Layout::from_size_align(32, 8).unwrap();
        let ptr = unsafe { a.alloc(layout) };
        unsafe { a.dealloc(ptr, layout) };

        assert_eq!(counters.snapshot(), AllocSnapshot::default());
    }
}
