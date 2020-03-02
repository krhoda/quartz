use std::sync::{Arc, Condvar, Mutex};
use std::fmt;

pub type Guard = Arc<(Mutex<bool>, Condvar)>;

#[derive(Debug)]
pub struct UsedChanErr;

pub struct PiChan<T> {
    guard: Guard,
    val: Arc<Mutex<Option<T>>>,
}

// (Send, Recv) = Chan := Send(Value) -> ReplyRecv := Recv() -> (Value, ReplySender)
// TODO: Implement fancier conversation types.
impl<T> PiChan<T> {
    pub fn new() -> Arc<PiChan<T>> {
        Arc::new(PiChan::<T> {
            guard: Arc::new((Mutex::new(false), Condvar::new())),
            val: Arc::new(Mutex::new(None)),
        })
    }

    pub fn send(&mut self, t: T) -> Result<(), UsedChanErr> {
        let mut data = self.val.lock().unwrap();
        *data = Some(t);

        let (m, c) = self.guard.as_ref();
        let used = *m.lock().unwrap();
        if used {
            Err(UsedChanErr)
        } else {
            *m.lock().unwrap() = true;
            c.notify_one();
            Ok(())
        }
    }

    pub fn recv(&mut self) -> Option<T> {
        let (m, c) = self.guard.as_ref();

        let _ok = c.wait(m.lock().unwrap()).unwrap();
        self.val.lock().unwrap().take()
    }
}

// NOTE: THIS IS LIFTED WHOLE-SALE FROM THE 
// AMAZING ABANDONED SAFE CODE OF: 
// https://github.com/BurntSushi/chan/blob/master/src/wait_group.rs
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