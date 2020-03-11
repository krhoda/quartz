use std::error::Error;
use std::fmt;
use std::sync::{Arc, Barrier, Mutex};
use wait_group::WaitGroup;

// AS WITH PiChan CONSIDER FOR DEADLOCK FREEDOM:
// Breaking into sender + reciever, killing a wait if the other drops from exisitence.

// This is a single-use quasi-rendezvous channel
// Functionally, obeyes the laws of Pi Calculus with additional propreties.
// These properties come with the cost of (mostly uncontested) locks.
// The sender always awaits the reciever.
// The reciever can sample -- not block in the presence of no sender.
// The above enables a "choice" mechanism.
// Maybe merged with PiChan if no extra locks are required.
#[derive(Clone)]
pub struct PropChan<T>(Arc<PropMachine<T>>);

#[derive(Debug)]
pub enum PropChanState {
    Unintialized, // Shouldn't happen, but who knows what someone will do.
    Open,         // Neither Send or Recieve is Used.
    AwaitSend,    // A listener is waiting.
    AwaitRecv,    // A sender is waiting.
    Used,         // A transfer was made.
}

impl fmt::Display for PropChanState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropChanState::Unintialized => {
                write!(f, "Uninitalized (ILLEGAL, USE PiChan::<T>::new)")
            }
            PropChanState::Open => write!(f, "Open"),
            PropChanState::AwaitSend => write!(f, "AwaitSend"),
            PropChanState::AwaitRecv => write!(f, "AwaitRecv"),
            PropChanState::Used => write!(f, "Used"),
        }
    }
}

struct PropMachine<T> {
    init: Arc<Mutex<bool>>,
    val: Arc<Mutex<Option<T>>>,
    send_guard: Arc<Mutex<bool>>,
    send_bar: Arc<Barrier>,
    recv_guard: Arc<Mutex<bool>>,
    recv_bar: Arc<Barrier>,
}

impl<T> PropChan<T> {
    pub fn new() -> PropChan<T> {
        let send_barrier = Arc::new(Barrier::new(2));
        let recv_barrier = Arc::new(Barrier::new(2));

        // Each thread will decrement each barrier once.
        // The recv barrier is lifted by both parties.
        // The send barrier is lifted by the reciever.
        // The sender places the value.
        // The send barrier is lifted by the sender.
        // The reciever extracts the value.

        // If this were not rendezvous or were long lived
        // WaitGroups would be needed to "lock the door" behind you.

        PropChan::<T>(Arc::new(PropMachine::<T> {
            init: Arc::new(Mutex::new(true)),
            val: Arc::new(Mutex::new(None)),
            send_guard: Arc::new(Mutex::new(false)),
            send_bar: send_barrier,
            recv_guard: Arc::new(Mutex::new(false)),
            recv_bar: recv_barrier,
        }))
    }

    pub fn state(&self) -> PropChanState {
        match self.check_init() {
            false => PropChanState::Unintialized,
            _ => match self.check_send_used() {
                true => match self.check_recv_used() {
                    true => PropChanState::Used,
                    _ => PropChanState::AwaitRecv,
                },
                _ => match self.check_recv_used() {
                    true => PropChanState::AwaitSend,
                    _ => PropChanState::Open,
                },
            },
        }
    }

    pub fn send(&mut self, t: T) -> Result<(), PropChanError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            true => {
                let r = self.set_send_used();

                match r {
                    // We are not the winning sender, the channel has been used.
                    Err(x) => Err(x),

                    Ok(()) => {
                        // Detect Recieve.
                        self.0.recv_bar.wait();

                        // finally.
                        let mut data = self.0.val.lock().unwrap();
                        *data = Some(t);

                        // Inform recieve we exist
                        self.0.send_bar.wait();

                        // Weaken references to self?
                        // If so, one here.
                        Ok(())
                    }
                }
            }
        }
    }

    pub fn recv(&mut self) -> Result<Option<T>, PropChanError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            true => {
                let r = self.set_recv_used();
                match r {
                    Err(x) => Err(x),
                    Ok(()) => {
                        self.0.recv_bar.wait(); // Alert the sender.

                        self.0.send_bar.wait(); // Await the sender.

                        // Weaken references to self?
                        Ok(self.0.val.lock().unwrap().take())
                        // If so, one here after assigning take.
                        // Then return the assigned take.
                    }
                }
            }
        }
    }

    pub fn sample(&mut self) -> Result<(bool, Option<T>), PropChanError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            _ => {
                // Check if used and block other recvers.
                let mut is_used = self.0.recv_guard.lock().unwrap();

                match *is_used {
                    true => Err(PropChanError::UsedRecvChanError),
                    _ => {
                        let has_sender = self.0.send_guard.lock().unwrap();
                        match *has_sender {
                            false => Ok((false, None)),
                            _ => {
                                self.0.recv_bar.wait(); // Alert the sender.

                                self.0.send_bar.wait(); // Await the sender.

                                // Weaken references to self?
                                Ok((true, self.0.val.lock().unwrap().take()))
                                // If so, one here after assigning take.
                                // Then return the assigned take.
                            }
                        }
                    }
                }
            }
        }
    }

    fn set_send_used(&mut self) -> Result<(), PropChanError> {
        let mut is_used = self.0.send_guard.lock().unwrap();

        match *is_used {
            true => Err(PropChanError::UsedSendChanError),
            false => {
                *is_used = true;
                Ok(())
            }
        }
    }

    fn set_recv_used(&mut self) -> Result<(), PropChanError> {
        let mut is_used = self.0.recv_guard.lock().unwrap();

        match *is_used {
            true => Err(PropChanError::UsedRecvChanError),
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

#[derive(Debug)]
pub enum PropChanError {
    UsedSendChanError,
    UsedRecvChanError,
    UninitializedChanError,
}

impl fmt::Display for PropChanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropChanError::UsedSendChanError => {
                write!(f, "This instance of PropChan already has a sender")
            }
            PropChanError::UsedRecvChanError => {
                write!(f, "This instance of PropChan already has a reciever")
            }
            PropChanError::UninitializedChanError => {
                write!(f, "PropChan must be initialized to use safely")
            }
        }
    }
}

impl Error for PropChanError {
    fn description(&self) -> &str {
        match self {
            PropChanError::UsedSendChanError => "This instance of PropChan already has a sender",
            PropChanError::UsedRecvChanError => "This instance of PropChan already has a reciever",
            PropChanError::UninitializedChanError => "PropChan must be initialized to use safely",
        }
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
