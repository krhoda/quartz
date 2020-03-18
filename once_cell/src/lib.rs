use std::cmp::PartialEq;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, LockResult, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use wait_group::WaitGroup;

// TODOS:
// 1 -- check_init / check_send_guard need to return errs 
// 2 -- Poison errs need to bubble
// 3 -- hide the None -> Option<T> transformation, since it cannot be None.
// 4 -- Add ImpossibleState err to support above.
    

// Functions as a Multi-Writer, Single-Value, Multi-Consumer channel.
// No redezvous.
// Multiple writers can write to the same OnceCell, provided they are writing the same value
// Different values are an error
// Using read a caller awaits the write event
// Using sample a caller recieves either
// (false, Arc<Mutex<None>>) before the write event and
// (true, Arc<Mutex<Some<TargetValue>>>) after the write event
// It is best to think of this as a future that was run (at least) once then cached.
#[derive(Clone, Debug)]
pub struct OnceCell<T>(Arc<OnceCellMachine<T>>)
where
    T: PartialEq;

impl<T: PartialEq> PartialEq for OnceCell<T> {
    fn eq(&self, other: &Self) -> bool {
        let res1 = self.sample();
        let res2 = other.sample();

        match res1 {
            Err(_) => match res2 {
                Err(_) => true,
                Ok(_) => false,
            },

            Ok(_) => match res2 {
                Err(_) => false,
                Ok(_) => {
                    // Safe to unwrap.
                    let (a, b) = res1.unwrap();
                    let (x, y) = res2.unwrap();

                    // If both false, both contain None
                    match (false == a) && (false == x) {
                        true => true,
                        _ => match a == x {
                            // If mismatched, one contains None the other Some(T)
                            false => false,
                            // Truly compare the existant values.
                            _ => {
                                let data1 = b.read();
                                let data2 = y.read();
                                &*data1 == &*data2
                            }
                        },
                    }
                }
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct OnceVal<T>(Arc<RwLock<Option<T>>>)
where
    T: PartialEq;

// The read half of a RWLock who's write half is now inaccessible making it essetially lock free and threadsafe.
impl<T: PartialEq> OnceVal<T> {
    pub fn read(&self) -> RwLockReadGuard<'_, Option<T>> {
        // NOTE: the lock can never be poisoned at this point, thus the unchecked unwrap.
        // Panicking while holding the write lock would mean never writing a variable.
        // The OnceVal could not be read without the lock being "unpoisonable"
        self.0.read().unwrap()
    }

    pub fn clone(&self) -> OnceVal<T> {
        OnceVal::<T>(Arc::clone(&self.0))
    }

    fn write(&mut self) -> LockResult<RwLockWriteGuard<'_, Option<T>>> {
        self.0.write()
    }

    fn new(a: Arc<RwLock<Option<T>>>) -> OnceVal<T> {
        OnceVal::<T>(a)
    }
}

impl<T: PartialEq> PartialEq for OnceVal<T> {
    fn eq(&self, other: &Self) -> bool {
        &*self.read() == &*other.read()
    }
}

#[derive(Debug)]
pub enum OnceCellState {
    Unintialized, // Shouldn't happen, but who knows what someone will do.
    Empty,        // The Write has not occured.
    Filled,       // A transfer was made, now is a place to retrieve refs.
}

impl fmt::Display for OnceCellState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OnceCellState::Unintialized => write!(f, "Uninitalized (ILLEGAL, USE OnceCell::<T>::new)"),
            OnceCellState::Empty => write!(f, "Empty"),
            OnceCellState::Filled => write!(f, "Filled"),
        }
    }
}

#[derive(Debug)]
struct OnceCellMachine<T>
where
    T: PartialEq,
{
    init: Arc<Mutex<bool>>,
    val: Arc<Mutex<OnceVal<T>>>,
    send_guard: Arc<Mutex<bool>>,
    recv_wg: WaitGroup,
}

impl<T: PartialEq> OnceCell<T> {
    pub fn new() -> OnceCell<T> {
        let recv_wg = WaitGroup::new();
        recv_wg.add(1);

        // The readers will wait on the wait group.
        // The first writer will deposit the value into val.
        // The first writer will call done on the waitgroup.
        // The readers gain a clone of the Arc surrounding val.
        // Any following writers will be given a read lock to compare
        // against the value of their write, and an error will be
        // raised if the values mismatch.

        OnceCell::<T>(Arc::new(OnceCellMachine::<T> {
            init: Arc::new(Mutex::new(true)),
            val: Arc::new(Mutex::new(OnceVal::new(Arc::new(RwLock::new(None))))),
            send_guard: Arc::new(Mutex::new(false)),
            recv_wg: recv_wg,
        }))
    }

    // Check the state of a given OnceCell
    pub fn state(&self) -> OnceCellState {
        match self.check_init() {
            false => OnceCellState::Unintialized,
            _ => match self.check_send_used() {
                false => OnceCellState::Empty,
                _ => OnceCellState::Filled,
            },
        }
    }

    // Attempt to deposit a value into the OnceCell.
    // If the OnceCell is not initialized, or if the value is neither the first
    // nor matches the existing value, an error is raised.
    pub fn write(&mut self, t: T) -> Result<(), OnceCellError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized OnceCell through spectacular means.
            false => Err(OnceCellError::Uninitialized),

            true => {
                let res1 = self.0.send_guard.lock();
                match res1 {
                    // TODO: Bubble up poison err:
                    Err(_) => Err(OnceCellError::PosionWriteGuard),

                    // If the first lock works, check the second.
                    Ok(mut is_used) => match self.0.val.lock() {
                        // TODO: Bubble up poison err:
                        Err(_) => Err(OnceCellError::PosionValueGuard),

                        // Assuming all locks are good, and they should be
                        // Either write or compare.
                        Ok(mut wrapper) => match *is_used {
                            true => {
                                // NO BLOCKING!
                                let data = wrapper.read();
                                match &*data {
                                    Some(x) => match &t == x {
                                        true => Ok(()),
                                        _ => Err(OnceCellError::ValueMismatch),
                                    },

                                    // This would only trip in the frightening
                                    // "Someone panics holding the write lock" situation.
                                    // This could result in a deadlock, but interestingly is detectable.
                                    // (&*data == None && *is_used) = deadlock_for_readers.
                                    // This could be transmitted to the recievers and they return with a (documented) error.
                                    // Not implementing because I'm not convinced anyone could panic holding the write lock
                                    // Short of hardware failure.
                                    // Leaving this comment because I could very well be wrong.
                                    None => Err(OnceCellError::ValueMismatch),
                                }
                            }
                            false => {
                                // prevents any futher use of the write lock.
                                // note, we do this before checking the write lock,
                                // prefering a detectable deadlock to runtime panic.
                                *is_used = true;

                                // the only use of the write lock.
                                let res1 = wrapper.write();
                                match res1 {
                                    // TODO: PASS THE WRITE LOCK ERR AS SOURCE.
                                    Err(_) => Err(OnceCellError::PosionWriteLock),
                                    Ok(mut data) => {
                                        *data = Some(t);
                                        self.0.recv_wg.done();
                                        Ok(())
                                    }
                                }
                            }
                        },
                    },
                }
            }
        }
    }

    // read on an initialized OnceCell returns a OnceVal which can freely be read from across threads.
    // Blocks until OnceVal is ready.
    pub fn read(&self) -> Result<OnceVal<T>, OnceCellError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized OnceCell through spectacular means.
            false => Err(OnceCellError::Uninitialized),
            true => {
                self.0.recv_wg.wait();

                // TODO: Bubble up poison err.
                match self.0.val.lock() {
                    Err(_) => Err(OnceCellError::PosionValueGuard),
                    Ok(x) => Ok(x.clone()),
                }
            }
        }
    }

    // Since we have relaxed Pi Calculus' rendezvous requirement, OnceCell allow sampling.
    // Like recieve, but non-blocking. Instead immediately returns a tuple
    // The first element is a bool indicating if send has occured, and the second element is
    // Either a clone of the Arc<Mutex<Option<TargetValue>>>, or a wrapper around a none.
    pub fn sample(&self) -> Result<(bool, OnceVal<T>), OnceCellError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized OnceCell through spectacular means.
            false => Err(OnceCellError::Uninitialized),
            _ => {
                // Check if used and block other recvers.
                let res1 = self.0.send_guard.lock();
                match res1 {
                    // TODO: Bubble up err
                    Err(_) => Err(OnceCellError::PosionWriteGuard),
                    Ok(is_complete) => match *is_complete {
                        false => Ok((false, OnceVal::<T>::new(Arc::new(RwLock::new(None))))),
                        _ => {
                            // We might be right alongside the sender.
                            // In practice, should not block.
                            self.0.recv_wg.wait();
                            match self.0.val.lock() {
                                // TODO: Bubble up err
                                Err(_) => Err(OnceCellError::PosionValueGuard),
                                Ok(x) => Ok((true, x.clone())),
                            }
                        }
                    },
                }
            }
        }
    }

    // TODO: CHECK THESE UNWRAPS:
    fn check_send_used(&self) -> bool {
        *self.0.send_guard.lock().unwrap()
    }

    fn check_init(&self) -> bool {
        *self.0.init.lock().unwrap()
    }
}

// TODO: BUBBLE UP LOCK ERRS:

#[derive(Debug)]
pub enum OnceCellError {
    PosionWriteLock,
    PosionWriteGuard,
    PosionValueGuard,
    Uninitialized,
    ValueMismatch,
}

impl fmt::Display for OnceCellError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OnceCellError::PosionWriteLock => write!(f, "Impossible poisoned write lock, this is should NEVER HAPPEN, PLEASE FILE A BUG REPORT: github krhoda quartz"),
            OnceCellError::PosionWriteGuard => write!(f, "A thread has panicked while holding the OnceCell's write guard, this cell is now inaccessible this error is likely from a healthy thread, this is should NEVER HAPPEN, PLEASE FILE A BUG REPORT: github krhoda quartz"),
            OnceCellError::PosionValueGuard => write!(f, "Some other operation has panicked while holding the OnceCells value guard, this cell is now inaccessible this is should NEVER HAPPEN, PLEASE FILE A BUG REPORT: github krhoda quartz"),
            OnceCellError::ValueMismatch => write!(f, "OnceCell recieved differing values on write, only one value may be written to a give OnceCell"),
            OnceCellError::Uninitialized => write!(f, "OnceCell must be initialized to use safely"),
        }
    }
}

impl Error for OnceCellError {
    fn description(&self) -> &str {
        match self {
            OnceCellError::PosionWriteLock =>  "Impossible poisoned write lock, this is should NEVER HAPPEN, PLEASE FILE A BUG REPORT: github krhoda quartz",
            OnceCellError::PosionWriteGuard => "A thread has panicked while holding the OnceCell's write guard, this cell is now inaccessible, this error is likely from a healthy thread, this is should NEVER HAPPEN, PLEASE FILE A BUG REPORT: github krhoda quartz",
            OnceCellError::PosionValueGuard => "Some other operation has panicked while holding the OnceCells value guard, this cell is now inaccessible this is should NEVER HAPPEN, PLEASE FILE A BUG REPORT: github krhoda quartz",
            OnceCellError::ValueMismatch => "OnceCell recieved differing values on write, only one value may be written to a give OnceCell",
            OnceCellError::Uninitialized => "OnceCell must be initialized to use safely",
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
    fn test_i_var() {
        let mut p1 = OnceCell::<usize>::new();
        let q1 = p1.clone();
        let (intresting, _) = p1.sample().unwrap();
        match intresting {
            true => panic!("Recieved true from sample when it should've returned false"),
            _ => println!("Sample returned with the expected -- false -- result"),
        };

        let open_state = p1.state();
        match open_state {
            OnceCellState::Empty => println!(""),
            _ => println!("Unexpected state in open p1!"),
        };

        let h = thread::spawn(move || {
            let r = q1.read();
            match r {
                Err(x) => panic!("Oh No! Err in Q1 recieve: {}", x),
                Ok(a) => match *a.read() {
                    None => panic!("Got 'None' in Q1 recieve"),
                    Some(b) => println!("Got {} in Q1 recieve", b),
                },
            };

            let filled_state = q1.state();
            match filled_state {
                OnceCellState::Filled => println!(""),
                _ => panic!("Unexpected state in complete q1"),
            };

            let (etre, result) = q1.sample().expect("Error In Post-Send Sample!");
            match etre {
                true => println!(""),
                _ => panic!("Sample failed after send event."),
            };

            match *result.read() {
                None => panic!("Got 'None' in Q1 recieve"),
                Some(b) => println!("Got {} in Q1 recieve", b),
            };
        });

        p1.write(1).unwrap();
        let e = p1.write(2);
        match e {
            Err(x) => println!("Got expected err in mistmatched send: {}", x),
            Ok(_) => panic!("Got unexpected success in second send!?"),
        };

        let no_e = p1.write(1);
        match no_e {
            Err(x) => panic!("Got unexpected err in matched send err: {}", x),
            Ok(_) => println!("Got expected success in matched send"),
        };

        let should_be_one = p1.read();
        match should_be_one {
            Err(x) => panic!("Oh No! Err in P1 recieve: {}", x),
            Ok(a) => match *a.read() {
                None => panic!("Got 'None' in P1 recieve"),
                Some(b) => println!("Got {} in P1 recieve", b),
            },
        };

        println!("Will wait for thread 2");
        h.join().expect("Failed to Join Threads!");
    }

    #[test]
    fn test_nested_i_var() {
        let mut p1 = OnceCell::<usize>::new();
        let mut p2 = OnceCell::<OnceCell<usize>>::new();
        let q2 = p2.clone();

        let h = thread::spawn(move || {
            let q1_val = q2.read().unwrap();
            let q1 = q1_val.read();

            match &*q1 {
                Some(c) => {
                    let d = c.read().unwrap();
                    match &*d.read() {
                        Some(e) => println!("Recieved contrived val {}", e),
                        None => panic!("Heard None in contrived value"),
                    };
                }
                None => panic!("Heard None in contrived value wrapper"),
            }
        });

        p1.write(22).unwrap();
        p2.write(p1).unwrap();

        h.join()
            .expect("Failed to join threads in nested OnceCell test")
    }

    #[test]
    fn test_i_var_partial_eq() {
        let mut p1 = OnceCell::<usize>::new();
        let mut p2 = OnceCell::<usize>::new();
        let mut p3 = OnceCell::<usize>::new();

        assert_eq!(p1, p2);
        assert_eq!(p1, p3);
        assert_eq!(p3, p2);

        p1.write(1).unwrap();

        assert_ne!(p1, p2);
        assert_ne!(p1, p3);
        assert_eq!(p3, p2);

        p2.write(1).unwrap();
        p3.write(2).unwrap();

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
        assert_ne!(p3, p2);
    }
}
