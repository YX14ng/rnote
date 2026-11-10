//! Keeping a second click from starting the same work twice: a double click on a card would
//! otherwise import a Xournal++ notebook, or create a notebook, two times.

use std::cell::Cell;
use std::rc::Rc;

/// Whether some work started from the library is still running.
#[derive(Debug, Clone, Default)]
pub(crate) struct Busy(Rc<Cell<bool>>);

/// Held while the work runs; dropping it lets new work start.
#[derive(Debug)]
pub(crate) struct BusyGuard(Rc<Cell<bool>>);

impl Busy {
    /// Starts work, or `None` when other work is still running.
    pub(crate) fn start(&self) -> Option<BusyGuard> {
        if self.0.replace(true) {
            return None;
        }
        Some(BusyGuard(Rc::clone(&self.0)))
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::Busy;

    #[test]
    fn a_second_start_is_refused_while_the_first_runs() {
        let busy = Busy::default();

        let first = busy.start();

        assert!(first.is_some());
        assert!(busy.start().is_none());
    }

    #[test]
    fn work_can_start_again_once_the_first_ends() {
        let busy = Busy::default();

        drop(busy.start());

        assert!(busy.start().is_some());
    }
}
