//! Bounded transient actions shared with the editor.
//!
//! `Once` is an act, not a value: it has no parameter id and is absent from presets and CLAP
//! state. The GUI producer and audio consumer communicate only through atomics. Sequentially
//! consistent counters give reset/panic and a concurrent submission one total order: submissions
//! ordered before cancellation are retired, submissions ordered after it remain pending.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const CAPACITY: u64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Full;

#[derive(Debug, Default)]
pub struct TransientActions {
    /// Accepted requests not yet consumed or cancelled. Submission, consumption and cancellation
    /// all linearize on this one atomic, so a race cannot count one request as both fired and
    /// cancelled.
    pending: AtomicU64,
    fired: AtomicU64,
    cancelled: AtomicU64,
    rejected: AtomicU64,
}

impl TransientActions {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Submit one distinct Once activation. A full path reports rejection instead of coalescing or
    /// silently dropping the act.
    pub fn submit_once(&self) -> Result<(), Full> {
        loop {
            let pending = self.pending.load(Ordering::SeqCst);
            if pending >= CAPACITY {
                self.rejected.fetch_add(1, Ordering::SeqCst);
                return Err(Full);
            }
            match self.pending.compare_exchange(
                pending,
                pending + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return Ok(()),
                Err(_) => continue,
            }
        }
    }

    /// Editor producer: acceptance and the non-parameter process wake are one operation. A rejected
    /// request must not wake a host for work that does not exist.
    pub fn submit_once_from_editor(&self, wake: &dyn Fn()) -> Result<(), Full> {
        self.submit_once()?;
        wake();
        Ok(())
    }

    /// Audio-thread consumer. Exactly one accepted act is returned per call.
    pub(crate) fn take_once(&self) -> bool {
        loop {
            let pending = self.pending.load(Ordering::SeqCst);
            if pending == 0 {
                return false;
            }
            match self.pending.compare_exchange(
                pending,
                pending - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    self.fired.fetch_add(1, Ordering::SeqCst);
                    return true;
                }
                Err(_) => continue,
            }
        }
    }

    /// Cancel every request ordered before this boundary. Returns how many were cancelled.
    pub(crate) fn cancel_pending(&self) -> u64 {
        let count = self.pending.swap(0, Ordering::SeqCst);
        self.cancelled.fetch_add(count, Ordering::SeqCst);
        count
    }

    pub fn fired(&self) -> u64 {
        self.fired.load(Ordering::Relaxed)
    }
    pub fn cancelled(&self) -> u64 {
        self.cancelled.load(Ordering::Relaxed)
    }
    pub fn rejected(&self) -> u64 {
        self.rejected.load(Ordering::Relaxed)
    }
    pub fn pending(&self) -> u64 {
        self.pending.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_acts_neither_coalesce_nor_duplicate() {
        let q = TransientActions::default();
        q.submit_once().unwrap();
        q.submit_once().unwrap();
        assert!(q.take_once());
        assert!(q.take_once());
        assert!(!q.take_once());
        assert_eq!(q.fired(), 2);
    }

    #[test]
    fn a_full_path_reports_rejection() {
        let q = TransientActions::default();
        for _ in 0..CAPACITY {
            q.submit_once().unwrap();
        }
        assert_eq!(q.submit_once(), Err(Full));
        assert_eq!(q.rejected(), 1);
    }

    #[test]
    fn editor_submission_wakes_exactly_when_an_action_was_accepted() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let q = TransientActions::default();
        let wakes = AtomicUsize::new(0);
        let wake = || {
            wakes.fetch_add(1, Ordering::Relaxed);
        };
        for _ in 0..CAPACITY {
            q.submit_once_from_editor(&wake).unwrap();
        }
        assert_eq!(wakes.load(Ordering::Relaxed), CAPACITY as usize);
        assert_eq!(q.submit_once_from_editor(&wake), Err(Full));
        assert_eq!(wakes.load(Ordering::Relaxed), CAPACITY as usize);
    }

    #[test]
    fn cancellation_retires_only_the_requests_before_its_boundary() {
        let q = TransientActions::default();
        q.submit_once().unwrap();
        q.submit_once().unwrap();
        assert_eq!(q.cancel_pending(), 2);
        assert!(!q.take_once());
        q.submit_once().unwrap();
        assert!(
            q.take_once(),
            "a later act survives the earlier cancellation"
        );
        assert_eq!(q.cancelled(), 2);
    }

    #[test]
    fn a_race_counts_an_act_as_fired_or_cancelled_never_both() {
        for _ in 0..1_000 {
            let q = TransientActions::shared();
            q.submit_once().unwrap();
            std::thread::scope(|scope| {
                let consume = Arc::clone(&q);
                let cancel = Arc::clone(&q);
                scope.spawn(move || {
                    consume.take_once();
                });
                scope.spawn(move || {
                    cancel.cancel_pending();
                });
            });
            assert_eq!(q.pending(), 0);
            assert_eq!(q.fired() + q.cancelled(), 1);
        }
    }
}
