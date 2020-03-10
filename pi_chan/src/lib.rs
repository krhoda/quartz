use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};
use wait_group::WaitGroup;

#[derive(Debug)]
pub enum ChanError {
    UsedSendChanError,
    UsedRecvChanError,
    UninitializedChanError,
}

impl fmt::Display for ChanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChanError::UsedSendChanError => write!(f, "Cannot Send on PiChan more than once"),
            ChanError::UsedRecvChanError => write!(f, "Cannot Recieve on PiChan more than once"),
            ChanError::UninitializedChanError => {
                write!(f, "PiChan must be initialized to use safely")
            }
        }
    }
}

impl Error for ChanError {
    fn description(&self) -> &str {
        match self {
            ChanError::UsedSendChanError => "Cannot Send on PiChan more than once",
            ChanError::UsedRecvChanError => "Cannot Recieve on PiChan more than once",
            ChanError::UninitializedChanError => "PiChan must be initialized to use safely",
        }
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[derive(Clone)]
pub struct PiChan<T>(Arc<PiMachine<T>>);

#[derive(Debug)]
pub enum PiChanState {
    Unintialized, // Shouldn't happen, but who knows what someone will do.
    Open,         // Neither Send or Recieve is Used.
    AwaitSend,    // A listener is waiting.
    AwaitRecv,    // A sender is waiting.
    Used,         // A transfer was made.
}

impl fmt::Display for PiChanState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PiChanState::Unintialized => write!(f, "Uninitalized (ILLEGAL, USE PiChan::<T>::new)"),
            PiChanState::Open => write!(f, "Open"),
            PiChanState::AwaitSend => write!(f, "AwaitngSend"),
            PiChanState::AwaitRecv => write!(f, "AwaitingRecv"),
            PiChanState::Used => write!(f, "Used"),
        }
    }
}

struct PiMachine<T> {
    init: Arc<Mutex<bool>>,
    val: Arc<Mutex<Option<T>>>,
    send_guard: Arc<Mutex<bool>>,
    send_wg: WaitGroup,
    recv_guard: Arc<Mutex<bool>>,
    recv_wg: WaitGroup,
}

impl<T> PiChan<T> {
    pub fn new() -> PiChan<T> {
        // TODO: Consider changing to a barrier?
        let swg = WaitGroup::new();
        let rwg = WaitGroup::new();

        // Preparing for each thread to decrement the other's group, creating a barrier.
        // This is the importance of the init field.
        swg.add(1);
        rwg.add(1);

        PiChan::<T>(Arc::new(PiMachine::<T> {
            init: Arc::new(Mutex::new(true)),
            val: Arc::new(Mutex::new(None)),
            send_guard: Arc::new(Mutex::new(false)),
            send_wg: swg,
            recv_guard: Arc::new(Mutex::new(false)),
            recv_wg: rwg,
        }))
    }

    pub fn state(&self) -> PiChanState {
        match self.check_init() {
            false => PiChanState::Unintialized,
            _ => match self.check_send_used() {
                true => match self.check_recv_used() {
                    true => PiChanState::Used,
                    _ => PiChanState::AwaitRecv
                }
                _ => match self.check_recv_used() {
                    true => PiChanState::AwaitSend,
                    _ => PiChanState::Open
                }
            }
        }
    }

    // TODO: Optimize. Destroying channel == less locks.
    pub fn send(&mut self, t: T) -> Result<(), ChanError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized channel through spectacular means.
            false => Err(ChanError::UninitializedChanError),
            true => {
                let r = self.set_send_used();

                match r {
                    // We are not the winning sender, the channel has been used.
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
        }
    }

    pub fn recv(&mut self) -> Result<Option<T>, ChanError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized channel through spectacular means.
            false => Err(ChanError::UninitializedChanError),
            true => {
                let r = self.set_recv_used();
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
        }
    }

    fn set_send_used(&mut self) -> Result<(), ChanError> {
        let mut is_used = self.0.send_guard.lock().unwrap();

        match *is_used {
            true => Err(ChanError::UsedSendChanError),
            false => {
                *is_used = true;
                Ok(())
            }
        }
    }

    fn set_recv_used(&mut self) -> Result<(), ChanError> {
        let mut is_used = self.0.recv_guard.lock().unwrap();

        match *is_used {
            true => Err(ChanError::UsedRecvChanError),
            false => {
                *is_used = true;
                Ok(())
            }
        }
    }

    fn check_send_used(&self) -> bool {
        *self.0.send_guard.lock().unwrap()
    }

    fn check_recv_used(&self) -> bool {
        *self.0.recv_guard.lock().unwrap()
    }

    fn check_init(&self) -> bool {
        *self.0.init.lock().unwrap()
    }
}
