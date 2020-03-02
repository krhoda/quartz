use std::sync::{Arc, Condvar, Mutex};
use std::fmt;

pub type Guard = Arc<(Mutex<bool>, Condvar)>;

#[derive(Debug)]
pub struct UsedChanErr;

#[derive(Clone)]
pub struct PiChan<T>(Arc<PiMachine<T>>);

struct PiMachine<T> {
    init: bool,
    used: bool,
    val: Arc<Mutex<Option<T>>>,
    send_guard: Arc<Mutex<bool>>,
    send_wg: WaitGroup,
    recv_guard: Arc<Mutex<bool>>,
    recv_wg: WaitGroup,
}

impl<T> PiChan<T> {
    pub fn new() -> PiChan<T> {
        let swg = WaitGroup::new();
        let rwg = WaitGroup::new();

        swg.add(1);
        rwg.add(1);

        PiChan::<T>(Arc::new(PiMachine::<T> {
            init: true,
            used: false,
            val: Arc::new(Mutex::new(None)),
            send_guard: Arc::new(Mutex::new(false)),
            send_wg: swg,
            recv_guard: Arc::new(Mutex::new(false)),
            recv_wg: rwg,
        }))
    }

    pub fn is_alive(&self) -> bool {
        self.0.init && !self.0.used
    }

    fn is_dead(&self) -> bool {
        !self.is_alive()
    }

    pub fn send(&mut self, t: T) -> Result<(), UsedChanErr> {
        // Prevent other senders, hold the lock.
        let mut lockhold = self.0.send_guard.lock().unwrap();

        // TODO: Return UsedChan here if applicable. 

        // Detect Recieve.
        self.0.recv_wg.wait();

        // Block next Sender Recieve. (Remove After Used Implementation)
        self.0.recv_wg.add(1);

        // finally.
        let mut data = self.0.val.lock().unwrap();
        *data = Some(t);

        // Inform recieve we exist
        self.0.send_wg.done(); 

        *lockhold = true; // Hold the lock until now.
        // Weaken references to self?
        // If so, one here.
        Ok(())
    }

    pub fn recv(&mut self) -> Option<T> {
        let mut lockhold = self.0.recv_guard.lock().unwrap();

        // TODO: Return UsedChan here if applicable. 

        self.0.recv_wg.done(); // Alert the sender.

        self.0.send_wg.wait(); // Await the sender.

        self.0.send_wg.add(1); // Block Next Reciever.

        *lockhold = true; // Hold the lock until now.

        // Weaken references to self?
        self.0.val.lock().unwrap().take()
        // If so, one here after assigning take.
        // Then return the assigned take.
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