use std::cmp::PartialEq;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use wait_group::WaitGroup;

// Functions as a Multi-Writer, Single-Value, Multi-Consumer channel.
// No redezvous.
// Multiple write can write to the same IVar, provided they are writing the same value
// Different values are an error
// Using read a caller awaits the write event
// Using sample a caller recieves either
// (false, Arc<Mutex<None>>) before the write event and
// (true, Arc<Mutex<Some<TargetValue>>>) after the write event
// It is best to think of this as a future that was run (at least) once then cached.
#[derive(Clone, Debug)]
pub struct IVar<T>(Arc<IVarMachine<T>>)
where
    T: PartialEq;

impl<T: PartialEq> PartialEq for IVar<T> {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = self.sample().unwrap();
        let (x, y) = other.sample().unwrap();

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
}

#[derive(Clone, Debug)]
pub struct IVal<T>(Arc<RwLock<Option<T>>>)
where
    T: PartialEq;

// The read half of a RWLock who's write half is now inaccessible making it essetially lock free and threadsafe.
impl<T: PartialEq> IVal<T> {
    pub fn read(&self) -> RwLockReadGuard<'_, Option<T>> {
        self.0.read().unwrap()
    }

    pub fn clone(&self) -> IVal<T> {
        IVal::<T>(Arc::clone(&self.0))
    }

    fn write(&mut self) -> RwLockWriteGuard<'_, Option<T>> {
        self.0.write().unwrap()
    }

    fn new(a: Arc<RwLock<Option<T>>>) -> IVal<T> {
        IVal::<T>(a)
    }
}

impl<T: PartialEq> PartialEq for IVal<T> {
    fn eq(&self, other: &Self) -> bool {
        &*self.read() == &*other.read()
    }
}

#[derive(Debug)]
pub enum IVarState {
    Unintialized, // Shouldn't happen, but who knows what someone will do.
    Empty,        // The Write has not occured.
    Filled,       // A transfer was made, now is a place to retrieve refs.
}

impl fmt::Display for IVarState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IVarState::Unintialized => write!(f, "Uninitalized (ILLEGAL, USE IVar::<T>::new)"),
            IVarState::Empty => write!(f, "Empty"),
            IVarState::Filled => write!(f, "Filled"),
        }
    }
}

#[derive(Debug)]
struct IVarMachine<T>
where
    T: PartialEq,
{
    init: Arc<Mutex<bool>>,
    val: Arc<Mutex<IVal<T>>>,
    send_guard: Arc<Mutex<bool>>,
    recv_wg: WaitGroup,
}

impl<T: PartialEq> IVar<T> {
    pub fn new() -> IVar<T> {
        let recv_wg = WaitGroup::new();
        recv_wg.add(1);

        // The readers will wait on the wait group.
        // The first writer will deposit the value into val.
        // The first writer will call done on the waitgroup.
        // The readers gain a clone of the Arc surrounding val.
        // Any following writers will be given a read lock to compare
        // against the value of their write, and an error will be
        // raised if the values mismatch.

        IVar::<T>(Arc::new(IVarMachine::<T> {
            init: Arc::new(Mutex::new(true)),
            val: Arc::new(Mutex::new(IVal::new(Arc::new(RwLock::new(None))))),
            send_guard: Arc::new(Mutex::new(false)),
            recv_wg: recv_wg,
        }))
    }

    // Check the state of a given IVar
    pub fn state(&self) -> IVarState {
        match self.check_init() {
            false => IVarState::Unintialized,
            _ => match self.check_send_used() {
                false => IVarState::Empty,
                _ => IVarState::Filled,
            },
        }
    }

    // Attempt to deposit a value into the IVar.
    // If the IVar is not initialized, or if the value is neither the first
    // nor matches the existing value, an error is raised.
    pub fn write(&mut self, t: T) -> Result<(), IVarError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized IVar through spectacular means.
            false => Err(IVarError::Uninitialized),
            true => {
                let mut is_used = self.0.send_guard.lock().unwrap();
                let mut wrapper = self.0.val.lock().unwrap();

                match *is_used {
                    true => {
                        // NO BLOCKING!
                        let data = wrapper.read();
                        match &*data {
                            Some(x) => match &t == x {
                                true => Ok(()),
                                _ => Err(IVarError::ValueMismatch),
                            },
                            None => Err(IVarError::ValueMismatch),
                        }
                    }
                    false => {
                        let mut data = wrapper.write();
                        *is_used = true;
                        *data = Some(t);
                        self.0.recv_wg.done();
                        Ok(())
                    }
                }
            }
        }
    }

    // read on an initialized IVar returns a IVal which can freely be read from across threads.
    // Blocks until IVal is ready.
    pub fn read(&self) -> Result<IVal<T>, IVarError> {
        match self.check_init() {
            // We have come into the possesion of an uninitialized IVar through spectacular means.
            false => Err(IVarError::Uninitialized),
            true => {
                self.0.recv_wg.wait();
                Ok(self.0.val.lock().unwrap().clone())
            }
        }
    }

    // Since we have relaxed Pi Calculus' rendezvous requirement, IVar allow sampling.
    // Like recieve, but non-blocking. Instead immediately returns a tuple
    // The first element is a bool indicating if send has occured, and the second element is
    // Either a clone of the Arc<Mutex<Option<TargetValue>>>, or a wrapper around a none.
    pub fn sample(&self) -> Result<(bool, IVal<T>), IVarError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized IVar through spectacular means.
            false => Err(IVarError::Uninitialized),
            _ => {
                // Check if used and block other recvers.
                let is_complete = self.0.send_guard.lock().unwrap();

                match *is_complete {
                    false => Ok((false, IVal::<T>::new(Arc::new(RwLock::new(None))))),
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

    fn check_send_used(&self) -> bool {
        *self.0.send_guard.lock().unwrap()
    }

    fn check_init(&self) -> bool {
        *self.0.init.lock().unwrap()
    }
}

#[derive(Debug)]
pub enum IVarError {
    ValueMismatch,
    Uninitialized,
}

impl fmt::Display for IVarError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            IVarError::ValueMismatch => write!(f, "IVar recieved differing values on write, only one value may be written to a give IVar"),

            IVarError::Uninitialized => write!(f, "IVar must be initialized to use safely"),
        }
    }
}

impl Error for IVarError {
    fn description(&self) -> &str {
        match self {
            IVarError::ValueMismatch => "IVar recieved differing values on write, only one value may be written to a give IVar",
            IVarError::Uninitialized => "IVar must be initialized to use safely",
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
        let mut p1 = IVar::<usize>::new();
        let q1 = p1.clone();
        let (intresting, _) = p1.sample().unwrap();
        match intresting {
            true => panic!("Recieved true from sample when it should've returned false"),
            _ => println!("Sample returned with the expected -- false -- result"),
        };

        let open_state = p1.state();
        match open_state {
            IVarState::Empty => println!(""),
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
                IVarState::Filled => println!(""),
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
        let mut p1 = IVar::<usize>::new();
        let mut p2 = IVar::<IVar<usize>>::new();
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
            .expect("Failed to join threads in nested IVar test")
    }

    #[test]
    fn test_i_var_partial_eq() {
        let mut p1 = IVar::<usize>::new();
        let mut p2 = IVar::<usize>::new();
        let mut p3 = IVar::<usize>::new();

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
