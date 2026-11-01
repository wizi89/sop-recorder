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
use std::time::{Duration, Instant};

/// How many steps may hold a screenshot in memory at once: buffer slots, held
/// from before a step's pixels are read until its PNG is written
/// (privacy-capture D3).
///
/// Six rather than the two this used to be, because a step now spends most of
/// its life in the password check after its pixels are taken, and a slow check
/// must not hold up the next click's pixels. Still a fixed bound: the unbounded
/// buffers of a burst are what the pool was introduced for. In active window
/// mode a held buffer is the cropped area, not the monitor.
pub const DEFAULT_CAPTURE_PERMITS: usize = 6;

/// How many monitor pixel reads may run at once. Two, as the whole capture was
/// before D3: the read is the expensive moment for memory, and two pipeline.
pub const PIXEL_READ_PERMITS: usize = 2;

/// A slot wait shorter than this is ordinary scheduling, not a queue worth a
/// log line.
const SLOT_WAIT_WORTH_LOGGING: Duration = Duration::from_millis(20);

/// Run one step's two phases (D3): `read` under a buffer slot and a pixel-read
/// permit, then `process` under the slot alone.
///
/// The slot is taken first and held across both, so at most `slots` images
/// exist at once; the read permit is released as soon as the pixels are in
/// memory, so a slow `process` delays no later step's pixels until every slot
/// is taken. A wait for a slot is logged with its length.
pub fn two_phase<T, R>(
    slots: &CapturePermits,
    readers: &CapturePermits,
    read: impl FnOnce() -> T,
    process: impl FnOnce(T) -> R,
) -> R {
    let asked = Instant::now();
    let _slot = slots.acquire();
    let waited = asked.elapsed();
    if waited >= SLOT_WAIT_WORTH_LOGGING {
        log::warn!(
            "Step waited {} ms for a buffer slot: earlier steps are still being checked",
            waited.as_millis()
        );
    }
    let held = {
        let _reader = readers.acquire();
        read()
    };
    process(held)
}

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
        assert_eq!(
            completed.load(Ordering::SeqCst),
            20,
            "every capture must run"
        );
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

    /// Time from each step's input event to its pixel read, for steps spawned
    /// `gap` apart whose `process` takes `slow`.
    fn read_delays(steps: usize, gap: u64, slow: u64) -> (Vec<Duration>, usize, usize) {
        let slots = Arc::new(CapturePermits::new(DEFAULT_CAPTURE_PERMITS));
        let readers = Arc::new(CapturePermits::new(PIXEL_READ_PERMITS));
        let held = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let saved = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..steps)
            .map(|_| {
                let event = Instant::now();
                let (slots, readers, held, peak, saved) = (
                    slots.clone(),
                    readers.clone(),
                    held.clone(),
                    peak.clone(),
                    saved.clone(),
                );
                let handle = std::thread::spawn(move || {
                    two_phase(
                        &slots,
                        &readers,
                        || {
                            let now = held.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            let delay = event.elapsed();
                            std::thread::sleep(Duration::from_millis(5)); // the pixel read
                            delay
                        },
                        |delay| {
                            std::thread::sleep(Duration::from_millis(slow)); // the check
                            saved.fetch_add(1, Ordering::SeqCst);
                            held.fetch_sub(1, Ordering::SeqCst);
                            delay
                        },
                    )
                });
                std::thread::sleep(Duration::from_millis(gap));
                handle
            })
            .collect();
        let delays = handles.into_iter().map(|h| h.join().unwrap()).collect();
        (
            delays,
            peak.load(Ordering::SeqCst),
            saved.load(Ordering::SeqCst),
        )
    }

    /// Five clicks within a second against a check that takes its full budget:
    /// every screenshot is taken at its own click.
    #[test]
    fn a_slow_check_does_not_delay_the_next_screenshots() {
        let (delays, _, saved) = read_delays(5, 150, 500);
        assert_eq!(saved, 5);
        for (step, delay) in delays.iter().enumerate() {
            assert!(
                *delay < Duration::from_millis(100),
                "step {} waited {:?} for its pixels",
                step + 1,
                delay
            );
        }
    }

    /// Twenty quick clicks while every check runs long: never more than six
    /// screenshots held, and every click still produces a saved step.
    #[test]
    fn a_long_burst_holds_at_most_six_buffers_and_saves_every_step() {
        let (_, peak, saved) = read_delays(20, 5, 200);
        assert!(
            peak <= DEFAULT_CAPTURE_PERMITS,
            "{peak} buffers held at once"
        );
        assert_eq!(
            peak, DEFAULT_CAPTURE_PERMITS,
            "the burst should fill every slot"
        );
        assert_eq!(saved, 20);
    }

    /// Stop drains the in-flight counter, which drops only after phase B: a
    /// step blocked in a slow check is waited for, not raced past.
    #[tokio::test]
    async fn stop_waits_for_a_step_still_in_its_check() {
        let slots = Arc::new(CapturePermits::new(DEFAULT_CAPTURE_PERMITS));
        let readers = Arc::new(CapturePermits::new(PIXEL_READ_PERMITS));
        let in_flight = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let written = Arc::new(AtomicUsize::new(0));

        in_flight.fetch_add(1, Ordering::SeqCst);
        {
            let (in_flight, written) = (in_flight.clone(), written.clone());
            std::thread::spawn(move || {
                two_phase(
                    &slots,
                    &readers,
                    || (),
                    |()| {
                        std::thread::sleep(Duration::from_millis(400));
                        written.fetch_add(1, Ordering::SeqCst);
                    },
                );
                in_flight.fetch_sub(1, Ordering::SeqCst);
            });
        }

        let left = crate::commands::recording::wait_for_captures(
            &in_flight,
            crate::commands::recording::CAPTURE_DRAIN_TIMEOUT,
        )
        .await;
        assert_eq!(left, 0);
        assert_eq!(
            written.load(Ordering::SeqCst),
            1,
            "stop returned before the step was written"
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
