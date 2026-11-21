//! Lightweight, dependency-free measurement primitives for the two-gate workflow.
//!
//! Spans are opt-in and zero-cost when disabled (`MC_SPANS=1` or [`set_spans_enabled`]); `Stats`
//! gives min/median/p95/max over `Duration` samples. Nothing here allocates on the hot path unless
//! spans are enabled (and even then only into a thread-local `Vec`).

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

static SPANS_ENABLED: AtomicBool = AtomicBool::new(false);

/// Turn span capture on/off (process-wide).
pub fn set_spans_enabled(on: bool) {
    SPANS_ENABLED.store(on, Ordering::Relaxed);
}

pub fn spans_enabled() -> bool {
    SPANS_ENABLED.load(Ordering::Relaxed)
}

/// Enable spans if `MC_SPANS=1`.
pub fn init_from_env() {
    if std::env::var("MC_SPANS").as_deref() == Ok("1") {
        set_spans_enabled(true);
    }
}

/// One recorded span.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub name: &'static str,
    pub duration: Duration,
}

thread_local! {
    static SPANS: RefCell<Vec<Sample>> = const { RefCell::new(Vec::new()) };
}

/// RAII span: records its duration on drop when spans are enabled.
pub struct Span {
    name: &'static str,
    start: Option<Instant>,
}

impl Span {
    pub fn new(name: &'static str) -> Self {
        let start = if spans_enabled() {
            Some(Instant::now())
        } else {
            None
        };
        Span { name, start }
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let duration = start.elapsed();
            SPANS.with(|s| {
                s.borrow_mut().push(Sample {
                    name: self.name,
                    duration,
                })
            });
        }
    }
}

/// Start a span named `name`.
pub fn span(name: &'static str) -> Span {
    Span::new(name)
}

/// Drain the spans recorded on this thread.
pub fn take_spans() -> Vec<Sample> {
    SPANS.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

/// Time a closure and return its result with the elapsed time.
pub fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

/// A set of duration samples with the usual percentiles.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    samples: Vec<Duration>,
}

impl Stats {
    pub fn new() -> Self {
        Stats::default()
    }

    pub fn record(&mut self, d: Duration) {
        self.samples.push(d);
    }

    pub fn extend<I: IntoIterator<Item = Duration>>(&mut self, it: I) {
        self.samples.extend(it);
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn mean(&self) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let total: u128 = self.samples.iter().map(|d| d.as_nanos()).sum();
        Some(Duration::from_nanos(
            (total / self.samples.len() as u128) as u64,
        ))
    }

    pub fn min(&self) -> Option<Duration> {
        self.samples.iter().copied().min()
    }

    pub fn max(&self) -> Option<Duration> {
        self.samples.iter().copied().max()
    }

    /// Nearest-rank percentile, `p` in `0.0..=100.0`.
    pub fn percentile(&self, p: f64) -> Option<Duration> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let p = p.clamp(0.0, 100.0);
        let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
        let idx = rank.saturating_sub(1).min(sorted.len() - 1);
        Some(sorted[idx])
    }

    pub fn median(&self) -> Option<Duration> {
        self.percentile(50.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_known_values() {
        let mut s = Stats::new();
        for ms in [1u64, 2, 3, 4, 5] {
            s.record(Duration::from_millis(ms));
        }
        assert_eq!(s.len(), 5);
        assert_eq!(s.min(), Some(Duration::from_millis(1)));
        assert_eq!(s.max(), Some(Duration::from_millis(5)));
        assert_eq!(s.median(), Some(Duration::from_millis(3)));
        assert_eq!(s.percentile(100.0), Some(Duration::from_millis(5)));
        assert_eq!(s.mean(), Some(Duration::from_millis(3)));
    }

    #[test]
    fn empty_stats_are_none() {
        let s = Stats::new();
        assert!(s.is_empty());
        assert_eq!(s.mean(), None);
        assert_eq!(s.median(), None);
        assert_eq!(s.percentile(95.0), None);
    }

    #[test]
    fn time_reports_nonzero_work() {
        let (v, d) = time(|| {
            let mut x = 0u64;
            for i in 0..10_000 {
                x = x.wrapping_add(i);
            }
            x
        });
        assert_eq!(v, (0..10_000u64).fold(0u64, |a, b| a.wrapping_add(b)));
        assert!(d > Duration::ZERO);
    }

    #[test]
    fn spans_record_only_when_enabled() {
        set_spans_enabled(false);
        drop(span("off"));
        assert!(take_spans().is_empty());

        set_spans_enabled(true);
        drop(span("on"));
        let spans = take_spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].name, "on");
        set_spans_enabled(false);
    }
}
