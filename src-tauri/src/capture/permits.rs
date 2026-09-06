//! A permit pool that bounds how many screenshot captures run at once.
//!
//! Every input event spawns its own capture thread, and each capture holds a
//! full-screen RGBA buffer plus the copies a resize makes. Unbounded, a burst
//! of rapid clicking put one of those in memory per click simultaneously --
//! roughly 66 MB apiece on two 4K displays before this change also narrowed
//! captures to a single monitor. Captures that fail under that pressure leave
//! gaps in the step numbering, which is where the 2026-09-03 recordings lost
//! their steps.
//!
//! A permit pool rather than a worker thread: capture and PNG encode are
//! separate costs that pipeline usefully, so serialising outright would be
//! slower on every machine to fix a problem that only appears under a burst.
//! The step number is assigned before a thread ever asks for a permit, so
//! queueing here cannot reorder the recording.
//!
//! `Condvar` rather than `tokio::sync::Semaphore`: these are OS threads spawned
//! from the input-hook callback, with no async runtime in reach, and
//! `blocking_acquire` from inside one would need a handle threaded through the
//! hook for no gain.

use std::sync::{Condvar, Mutex};

/// How many captures may be in progress at once.
///
/// Two rather than one because the work pipelines -- one thread can be encoding
/// a PNG while another captures -- and rather than more because each additional
/// permit is another full-screen buffer resident at the same time, which is the
/// pressure being removed.
pub const DEFAULT_CAPTURE_PERMITS: usize = 2;

/// A bounded pool of capture permits.
pub struct CapturePermits {
    available: Mutex<usize>,
    released: Condvar,
}

impl CapturePermits {
    pub fn new(permits: usize) -> Self {
        Self {
            available: Mutex::new(permits),
            released: Condvar::new(),
        }
    }

    /// Block until a permit is free, then hold it until the guard is dropped.
    ///
    /// The guard releases on drop, so a capture that panics returns its permit
    /// rather than shrinking the pool for the rest of the recording.
    pub fn acquire(&self) -> CapturePermit<'_> {
        let mut available = self.available.lock().unwrap_or_else(|e| e.into_inner());
        while *available == 0 {
            available = self
                .released
                .wait(available)
                .unwrap_or_else(|e| e.into_inner());
        }
        *available -= 1;
        CapturePermit { pool: self }
    }
}

impl Default for CapturePermits {
    fn default() -> Self {
        Self::new(DEFAULT_CAPTURE_PERMITS)
    }
}

/// One permit, held for as long as this value lives.
pub struct CapturePermit<'a> {
    pool: &'a CapturePermits,
}

impl Drop for CapturePermit<'_> {
    fn drop(&mut self) {
        // `unwrap_or_else(into_inner)` throughout: a capture thread that
        // panicked while holding the lock must not poison the pool and wedge
        // every later capture of the recording.
        let mut available = self
            .pool
            .available
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *available += 1;
        drop(available);
        self.pool.released.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// The property the pool exists for: a burst never has more captures
    /// resident than the limit, and none of them is dropped.
    #[test]
    fn a_burst_never_exceeds_the_limit_and_loses_nothing() {
        let pool = Arc::new(CapturePermits::new(2));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..20)
            .map(|_| {
                let (pool, in_flight, peak, completed) = (
                    pool.clone(),
                    in_flight.clone(),
                    peak.clone(),
                    completed.clone(),
                );
                std::thread::spawn(move || {
                    let _permit = pool.acquire();
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    // Long enough that overlapping work would be observed if
                    // the pool let it through.
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    completed.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert!(
            peak.load(Ordering::SeqCst) <= 2,
            "peak concurrency was {}",
            peak.load(Ordering::SeqCst),
        );
        assert_eq!(completed.load(Ordering::SeqCst), 20, "every capture must run");
    }

    /// A capture that panics has to give its permit back. Without this the pool
    /// drains one permit per failure and the recording eventually stops
    /// capturing altogether -- a worse version of the bug being fixed.
    #[test]
    fn a_panicking_capture_returns_its_permit() {
        let pool = Arc::new(CapturePermits::new(1));

        let poisoner = {
            let pool = pool.clone();
            std::thread::spawn(move || {
                let _permit = pool.acquire();
                panic!("capture failed hard");
            })
        };
        assert!(poisoner.join().is_err(), "the thread was supposed to panic");

        // If the permit leaked, this blocks forever; the join below is the
        // assertion.
        let after = {
            let pool = pool.clone();
            std::thread::spawn(move || {
                let _permit = pool.acquire();
            })
        };
        after.join().unwrap();
    }

    /// `stop_recording` and the generate path both drain by polling the
    /// in-flight counter to zero. That is only correct if the counter is raised
    /// *before* a capture queues for a permit, not when it starts running --
    /// otherwise a burst leaves captures waiting that the drain cannot see, and
    /// generation starts against a folder still being written. This models that
    /// discipline: raise, queue, run, lower.
    #[test]
    fn draining_the_in_flight_counter_waits_for_queued_captures_too() {
        let pool = Arc::new(CapturePermits::new(2));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let written = Arc::new(AtomicUsize::new(0));

        for _ in 0..20 {
            let (pool, in_flight, written) = (pool.clone(), in_flight.clone(), written.clone());
            // Raised on the caller's thread, before the worker exists.
            in_flight.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let _permit = pool.acquire();
                std::thread::sleep(std::time::Duration::from_millis(2));
                written.fetch_add(1, Ordering::SeqCst);
                in_flight.fetch_sub(1, Ordering::SeqCst);
            });
        }

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while in_flight.load(Ordering::SeqCst) > 0 {
            assert!(std::time::Instant::now() < deadline, "drain did not finish");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        assert_eq!(
            written.load(Ordering::SeqCst),
            20,
            "the drain returned before every queued capture had been written",
        );
    }

    #[test]
    fn a_single_permit_serialises_completely() {
        let pool = Arc::new(CapturePermits::new(1));
        let peak = Arc::new(AtomicUsize::new(0));
        let in_flight = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (pool, peak, in_flight) = (pool.clone(), peak.clone(), in_flight.clone());
                std::thread::spawn(move || {
                    let _permit = pool.acquire();
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }
}
