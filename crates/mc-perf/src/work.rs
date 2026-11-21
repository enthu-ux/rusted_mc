//! Deterministic parallel helpers (the `horizon_threads::work` idea: split independent work, no
//! shared mutable state across parts). No Horizon code is copied.
//!
//! `par_chunks_mut` splits a slice into contiguous parts and runs `f` on each on a scoped thread.
//! It is only sound for **order-independent** work: results must not depend on the order parts run
//! (the same rule as `plan.md` §7's propose-parallel / commit-serial).

use std::sync::OnceLock;

/// Worker threads this process should use: `MC_WORKERS` if set to a positive number, otherwise
/// `available_parallelism`.
pub fn worker_count() -> usize {
    static COUNT: OnceLock<usize> = OnceLock::new();
    *COUNT.get_or_init(|| {
        std::env::var("MC_WORKERS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
    })
}

/// Run `f` over contiguous parts of `data` on scoped threads. `parts` is clamped to
/// `1..=data.len()`; with one part (or empty/short data) it runs on the caller thread.
///
/// The closure must be deterministic and independent per part.
pub fn par_chunks_mut<T, F>(data: &mut [T], parts: usize, f: F)
where
    T: Send,
    F: Fn(&mut [T]) + Sync,
{
    let n = parts.max(1).min(data.len().max(1));
    if n <= 1 || data.len() <= 1 {
        f(data);
        return;
    }

    let per = data.len().div_ceil(n);
    std::thread::scope(|scope| {
        let f = &f;
        let mut rest = data;
        while !rest.is_empty() {
            let take = per.min(rest.len());
            let (head, tail) = rest.split_at_mut(take);
            rest = tail;
            scope.spawn(move || f(head));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_matches_serial() {
        let mut par: Vec<u64> = (0..1000).collect();
        par_chunks_mut(&mut par, 4, |part| {
            for x in part.iter_mut() {
                *x = x.wrapping_mul(3).wrapping_add(1);
            }
        });

        let serial: Vec<u64> = (0..1000u64)
            .map(|x| x.wrapping_mul(3).wrapping_add(1))
            .collect();
        assert_eq!(par, serial);
    }

    #[test]
    fn one_part_runs_on_caller() {
        let mut data: Vec<u64> = (0..17).collect();
        par_chunks_mut(&mut data, 1, |part| {
            for x in part.iter_mut() {
                *x += 1;
            }
        });
        assert_eq!(data, (1..=17).collect::<Vec<u64>>());
    }

    #[test]
    fn empty_slice_is_fine() {
        let mut data: Vec<u64> = Vec::new();
        par_chunks_mut(&mut data, 8, |part| {
            for x in part.iter_mut() {
                *x += 1;
            }
        });
        assert!(data.is_empty());
    }

    #[test]
    fn worker_count_is_positive() {
        assert!(worker_count() >= 1);
    }
}
