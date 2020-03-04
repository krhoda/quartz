use wait_group::WaitGroup;
use std::sync::{Arc, Condvar, Mutex};

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

        // Preparting for each thread to decrement the other's group, creating a barrier.
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

    fn is_in_flight(&self) -> bool {
        self.0.init && !self.0.used
    }

    pub fn is_used(&self) -> bool {
        !self.is_in_flight()
    }

    // TODO: Optimize. Destroying channel == less locks.
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
