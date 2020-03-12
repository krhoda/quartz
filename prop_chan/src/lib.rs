use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};
use wait_group::WaitGroup;

// The Theory:
// In the spirit of Propegators and Lattice-Variables, the notion of a repeatable/cached future.
// Similar to PiChan, permits only one sender.
// Dissimilar to PiChan, this sender does not block and there can be many recievers.
// The recievers are halted or sent away (if sampling) if the send has not yet occured.
// Once the send has occured, an Arc<Mutex<Option<T>>> is cloned per-reciever.

// AS WITH PiChan CONSIDER FOR DEADLOCK FREEDOM:
// Breaking into sender + reciever, killing a wait if the other drops from exisitence.

// Single-Sender, Multi-Consumer channel. 
// Sender sends non-blocking.
// Using recv a caller awaits the send event
// Using sample a caller recieves either 
// (false, Arc<Mutex<None>>) before the send event and
// (true, Arc<Mutex<Some<TargetValue>>>) after the send event 
// It is best to think of this as a future that was run once then cached.
#[derive(Clone)]
pub struct PropChan<T>(Arc<PropMachine<T>>);

#[derive(Debug)]
pub enum PropChanState {
    Unintialized, // Shouldn't happen, but who knows what someone will do.
    Open,         // The Send has not occured.
    Complete,     // A transfer was made, now is a place to retrieve refs.
}

impl fmt::Display for PropChanState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropChanState::Unintialized => {
                write!(f, "Uninitalized (ILLEGAL, USE PropChan::<T>::new)")
            }
            PropChanState::Open => write!(f, "Open"),
            PropChanState::Complete => write!(f, "Complete"),
        }
    }
}

struct PropMachine<T> {
    init: Arc<Mutex<bool>>,
    val: Arc<Mutex<Option<T>>>,
    send_guard: Arc<Mutex<bool>>,
    recv_wg: WaitGroup,
}

impl<T> PropChan<T> {
    pub fn new() -> PropChan<T> {
        let recv_wg = WaitGroup::new();
        recv_wg.add(1);

        // The recievers will wait on the wait group.
        // The sender will deposit the value into val.
        // The sender will call done on the waitgroup.
        // The recievers gain a clone of the Arc surrounding val.
        // No more senders are permitted, but the value is cloned freely.

        PropChan::<T>(Arc::new(PropMachine::<T> {
            init: Arc::new(Mutex::new(true)),
            val: Arc::new(Mutex::new(None)),
            send_guard: Arc::new(Mutex::new(false)),
            recv_wg: recv_wg,
        }))
    }

    pub fn state(&self) -> PropChanState {
        match self.check_init() {
            false => PropChanState::Unintialized,
            _ => match self.check_send_used() {
                false => PropChanState::Complete,
                _ => PropChanState::Open,
            },
        }
    }

    pub fn send(&mut self, t: T) -> Result<(), PropChanError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            true => {
                let r = self.set_send_used();

                match r {
                    // We are not the winning sender, the channel has been used.
                    Err(x) => Err(x),

                    Ok(()) => {
                        let mut data = self.0.val.lock().unwrap();
                        *data = Some(t);

                        self.0.recv_wg.done();

                        // Weaken references to self?
                        // If so, one here.
                        Ok(())
                    }
                }
            }
        }
    }

    pub fn recv(&mut self) -> Result<Arc<Mutex<Option<T>>>, PropChanError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            true => {
                self.0.recv_wg.wait();
                Ok(self.0.val.clone())
            }
        }
    }

    // Since we have relaxed Pi Calculus' rendezvous requirement, PropChan allow sampling.
    // Like recieve, but non-blocking. Instead immediately returns a tuple
    // The first element is a bool indicating if send has occured, and the second element is
    // Either a clone of the Arc<Mutex<Option<TargetValue>>>, or a wrapper around a none.
    pub fn sample(&mut self) -> Result<(bool, Arc<Mutex<Option<T>>>), PropChanError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            _ => {
                // Check if used and block other recvers.
                let is_complete = self.0.send_guard.lock().unwrap();

                match *is_complete {
                    false => Ok((false, Arc::new(Mutex::new(None)))),
                    _ => {
                        // We might be right alongside the sender.
                        // In practice, should not block.
                        self.0.recv_wg.wait();

                        Ok((true, self.0.val.clone()))
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

    fn check_send_used(&self) -> bool {
        *self.0.send_guard.lock().unwrap()
    }

    fn check_init(&self) -> bool {
        *self.0.init.lock().unwrap()
    }
}

#[derive(Debug)]
pub enum PropChanError {
    UsedSendChanError,
    UninitializedChanError,
}

impl fmt::Display for PropChanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropChanError::UsedSendChanError => {
                write!(f, "This instance of PropChan already has a sender")
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
            PropChanError::UninitializedChanError => "PropChan must be initialized to use safely",
        }
    }

    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
