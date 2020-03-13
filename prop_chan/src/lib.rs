use std::error::Error;
use std::fmt;
use std::sync::{Arc, LockResult, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use wait_group::WaitGroup;

// The Theory:
// In the spirit of Propegators and Lattice-Variables, the notion of a repeatable/cached future.
// Similar to PiChan, permits only one sender.
// Dissimilar to PiChan, this sender does not block and there can be many recievers.
// The recievers are halted or sent away (if sampling) if the send has not yet occured.
// The result is a guarded form of the RwLock provided by the standard lib, but not permitting anyone to write.
// Provides a read function which returns the lock guard from the underlying lock, making the borrow-checker happy.
// The lock will never be contested because the sole writer must conclude before the first reader reads.

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

// A thread-safe read-only instance of a variable deposited by send.
// Exposes read, which returns the RwLockReadGuard of the underlying RwLock
// Other arcitecture prevents a reader from reading before the sole write,
// Or the writer holding on to the lock for longer than the time to write,
// so the read function should never be contested, contain a poisoned err, or block.
// AND the borrow checker is happy.
#[derive(Clone)]
pub struct PropResult<T>(Arc<RwLock<Option<T>>>);

impl<T> PropResult<T> {
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, Option<T>>> {
        self.0.read()
    }

    pub fn clone(&self) -> PropResult<T> {
        PropResult::<T>(Arc::clone(&self.0))
    }

    fn write(&mut self) -> LockResult<RwLockWriteGuard<'_, Option<T>>> {
        self.0.write()
    }

    fn new(a: Arc<RwLock<Option<T>>>) -> PropResult<T> {
        PropResult::<T>(a)
    }
}

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
    val: Arc<Mutex<PropResult<T>>>,
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
            val: Arc::new(Mutex::new(PropResult::new(Arc::new(RwLock::new(None))))),
            send_guard: Arc::new(Mutex::new(false)),
            recv_wg: recv_wg,
        }))
    }

    // Check the state of a given PropChan
    pub fn state(&self) -> PropChanState {
        match self.check_init() {
            false => PropChanState::Unintialized,
            _ => match self.check_send_used() {
                false => PropChanState::Open,
                _ => PropChanState::Complete,
            },
        }
    }

    // Attempt to deposit a value into the channel.
    // If not the first, or if the channel was not built using new(), returns an error.
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
                        let mut wrapper = self.0.val.lock().unwrap();
                        let mut data = wrapper.write().unwrap();
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

    // recv on an initialized channel returns a PropResult which can freely be read from across threads.
    // Blocks until PropResult is ready.
    pub fn recv(&mut self) -> Result<PropResult<T>, PropChanError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            true => {
                self.0.recv_wg.wait();
                Ok(self.0.val.lock().unwrap().clone())
            }
        }
    }

    // Since we have relaxed Pi Calculus' rendezvous requirement, PropChan allow sampling.
    // Like recieve, but non-blocking. Instead immediately returns a tuple
    // The first element is a bool indicating if send has occured, and the second element is
    // Either a clone of the Arc<Mutex<Option<TargetValue>>>, or a wrapper around a none.
    pub fn sample(&mut self) -> Result<(bool, PropResult<T>), PropChanError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized channel through spectacular means.
            false => Err(PropChanError::UninitializedChanError),
            _ => {
                // Check if used and block other recvers.
                let is_complete = self.0.send_guard.lock().unwrap();

                match *is_complete {
                    false => Ok((false, PropResult::<T>::new(Arc::new(RwLock::new(None))))),
                    _ => {
                        // We might be right alongside the sender.
                        // In practice, should not block.
                        self.0.recv_wg.wait();

                        Ok((true, self.0.val.lock().unwrap().clone()))
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_prop_chan() {
        let mut p1 = PropChan::<usize>::new();
        let mut q1 = p1.clone();
        let (intresting, _) = p1.sample().unwrap();
        match intresting {
            true => panic!("Recieved true from sample when it should've returned false"),
            _ => println!("Sample returned with the expected -- false -- result"),
        };

        let open_state = p1.state();
        match open_state {
            PropChanState::Open => println!(""),
            _ => println!("Unexpected state in open p1!"),
        };

        let h = thread::spawn(move || {
            let r = q1.recv();
            match r {
                Err(x) => panic!("Oh No! Err in Q1 recieve: {}", x),
                Ok(a) => match *a.read().unwrap() {
                    None => panic!("Got 'None' in Q1 recieve"),
                    Some(b) => println!("Got {} in Q1 recieve", b),
                },
            };

            let complete_state = q1.state();
            match complete_state {
                PropChanState::Complete => println!(""),
                _ => panic!("Unexpected state in complete q1"),
            };

            let (etre, result) = q1.sample().expect("Error In Post-Send Sample!");
            match etre {
                true => println!(""),
                _ => panic!("Sample failed after send event."),
            };

            match *result.read().unwrap() {
                None => panic!("Got 'None' in Q1 recieve"),
                Some(b) => println!("Got {} in Q1 recieve", b),
            };
        });

        p1.send(1).unwrap();
        let e = p1.send(2);
        match e {
            Err(x) => println!("Got expected err: {}", x),
            Ok(_) => panic!("Got unexpected success in second send!?"),
        }

        let should_be_one = p1.recv();
        match should_be_one {
            Err(x) => panic!("Oh No! Err in P1 recieve: {}", x),
            Ok(a) => match *a.read().unwrap() {
                None => panic!("Got 'None' in P1 recieve"),
                Some(b) => println!("Got {} in P1 recieve", b),
            },
        };

        println!("Will wait for thread 2");
        h.join().expect("Failed to Join Threads!");
    }
}
