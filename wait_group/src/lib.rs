use std::fmt;
use std::sync::{Arc, Condvar, Mutex};
// NOTE: SRC IS LIFTED WHOLE-SALE FROM THE
// AMAZING ABANDONED SAFE CODE OF:
// https://github.com/BurntSushi/chan/blob/master/src/wait_group.rs

// Tests are my addition.

#[derive(Clone)]
pub struct WaitGroup(Arc<WaitGroupInner>);

struct WaitGroupInner {
    cond: Condvar,
    count: Mutex<i32>,
}
impl WaitGroup {
    /// Create a new wait group.
    pub fn new() -> WaitGroup {
        WaitGroup(Arc::new(WaitGroupInner {
            cond: Condvar::new(),
            count: Mutex::new(0),
        }))
    }

    /// Add a new thread to the waitgroup.
    ///
    /// # Failure
    ///
    /// If the internal count drops below `0` as a result of calling `add`,
    /// then this function panics.
    pub fn add(&self, delta: i32) {
        let mut count = self.0.count.lock().unwrap();
        *count += delta;
        assert!(*count >= 0);
        self.0.cond.notify_all();
    }

    /// Mark a thread as having finished.
    ///
    /// (This is equivalent to calling `add(-1)`.)
    pub fn done(&self) {
        self.add(-1);
    }

    /// Wait until all threads have completed.
    ///
    /// This unblocks when the internal count is `0`.
    pub fn wait(&self) {
        let mut count = self.0.count.lock().unwrap();
        while *count > 0 {
            count = self.0.cond.wait(count).unwrap();
        }
    }
}

impl fmt::Debug for WaitGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let count = self.0.count.lock().unwrap();
        write!(f, "WaitGroup {{ count: {:?} }}", *count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::{Arc, Mutex};
    use std::panic;

    #[test]
    // tests standard usage
    // Implicit test of WG effectiveness by not calling join on the handlers.
    fn test_wait_group() {
        let wg = WaitGroup::new();
        let one = Arc::new(Mutex::new(false));
        let two = Arc::new(Mutex::new(false));

        wg.add(1);
        let wg1 = wg.clone();
        let one1 = one.clone();

        thread::spawn(move || {
            let mut x = one1.lock().unwrap();
            *x = true;
            wg1.done();
        });

        wg.add(1);
        let wg2 = wg.clone();
        let two2 = two.clone();
        thread::spawn(move || {
            let mut x = two2.lock().unwrap();
            *x = true;
            wg2.done();
        });


        wg.wait();

        assert!(*one.lock().unwrap());
        assert!(*two.lock().unwrap());
    }

    #[test]
    #[should_panic]
    fn test_wait_group_panic() {
            let wg = WaitGroup::new();
            wg.add(-1);
    }
}
