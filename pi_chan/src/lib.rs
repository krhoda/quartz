use std::sync::{Arc, Condvar, Mutex};
use wait_group::WaitGroup;

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
        let r = self.check_send_used();
        match r {
            Err(x) => Err(x),

            Ok(()) => {
                // Detect Recieve.
                self.0.recv_wg.wait();

                // finally.
                let mut data = self.0.val.lock().unwrap();
                *data = Some(t);

                // Inform recieve we exist
                self.0.send_wg.done();

                // Weaken references to self?
                // If so, one here.
                Ok(())
            }
        }
    }

    pub fn recv(&mut self) -> Result<Option<T>, UsedChanErr> {
        let r = self.check_recv_used();
        match r {
            Err(x) => Err(x),
            Ok(()) => {
                self.0.recv_wg.done(); // Alert the sender.

                self.0.send_wg.wait(); // Await the sender.

                // Weaken references to self?
                Ok(self.0.val.lock().unwrap().take())
                // If so, one here after assigning take.
                // Then return the assigned take.
            }
        }

    }

    fn check_send_used(&mut self) -> Result<(), UsedChanErr> {
        let mut is_used = self.0.send_guard.lock().unwrap();

        match *is_used {
            true => Err(UsedChanErr),
            false => {
                *is_used = true;
                Ok(())
            }
        }
    }

    fn check_recv_used(&mut self) -> Result<(), UsedChanErr> {
        let mut is_used = self.0.recv_guard.lock().unwrap();

        match *is_used {
            true => Err(UsedChanErr),
            false => {
                *is_used = true;
                Ok(())
            }
        }
    }
}
