use std::error::Error;
use std::fmt;
use std::sync::{Arc, Barrier, Mutex};

// AS WITH PropChan CONSIDER FOR DEADLOCK FREEDOM:
// Breaking into sender + reciever, killing a wait if the other drops from exisitence.

// This is a single-use rendezvous channel, obeying the laws of Pi Calculus.
// Lack of additional locks makes it more performant and lower profile than
// it's closely related cousin PropChan. Simplicity vs. Vercitility.
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
            PiChanState::AwaitSend => write!(f, "AwaitSend"),
            PiChanState::AwaitRecv => write!(f, "AwaitRecv"),
            PiChanState::Used => write!(f, "Used"),
        }
    }
}

struct PiMachine<T> {
    init: Arc<Mutex<bool>>,
    val: Arc<Mutex<Option<T>>>,
    send_guard: Arc<Mutex<bool>>,
    send_bar: Arc<Barrier>,
    recv_guard: Arc<Mutex<bool>>,
    recv_bar: Arc<Barrier>,
}

impl<T> PiChan<T> {
    pub fn new() -> PiChan<T> {
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

        PiChan::<T>(Arc::new(PiMachine::<T> {
            init: Arc::new(Mutex::new(true)),
            val: Arc::new(Mutex::new(None)),
            send_guard: Arc::new(Mutex::new(false)),
            send_bar: send_barrier,
            recv_guard: Arc::new(Mutex::new(false)),
            recv_bar: recv_barrier,
        }))
    }

    pub fn state(&self) -> PiChanState {
        match self.check_init() {
            false => PiChanState::Unintialized,
            _ => match self.check_send_used() {
                true => match self.check_recv_used() {
                    true => PiChanState::Used,
                    _ => PiChanState::AwaitRecv,
                },
                _ => match self.check_recv_used() {
                    true => PiChanState::AwaitSend,
                    _ => PiChanState::Open,
                },
            },
        }
    }

    pub fn send(&mut self, t: T) -> Result<(), PiChanError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized channel through spectacular means.
            false => Err(PiChanError::UninitializedChanError),
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

    pub fn recv(&mut self) -> Result<Option<T>, PiChanError> {
        match self.check_init() {
            // We have come into the possession of an uninitialized channel through spectacular means.
            false => Err(PiChanError::UninitializedChanError),
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

    fn set_send_used(&mut self) -> Result<(), PiChanError> {
        let mut is_used = self.0.send_guard.lock().unwrap();

        match *is_used {
            true => Err(PiChanError::UsedSendChanError),
            false => {
                *is_used = true;
                Ok(())
            }
        }
    }

    fn set_recv_used(&mut self) -> Result<(), PiChanError> {
        let mut is_used = self.0.recv_guard.lock().unwrap();

        match *is_used {
            true => Err(PiChanError::UsedRecvChanError),
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
pub enum PiChanError {
    UsedSendChanError,
    UsedRecvChanError,
    UninitializedChanError,
}

impl fmt::Display for PiChanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PiChanError::UsedSendChanError => {
                write!(f, "This instance of PiChan already has a sender")
            }
            PiChanError::UsedRecvChanError => {
                write!(f, "This instance of PiChan already has a reciever")
            }
            PiChanError::UninitializedChanError => {
                write!(f, "PiChan must be initialized to use safely")
            }
        }
    }
}

impl Error for PiChanError {
    fn description(&self) -> &str {
        match self {
            PiChanError::UsedSendChanError => "This instance of PiChan already has a sender",
            PiChanError::UsedRecvChanError => "This instance of PiChan already has a reciever",
            PiChanError::UninitializedChanError => "PiChan must be initialized to use safely",
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
    fn test_pi_chan() {
        let mut p1 = PiChan::<bool>::new();
        let mut q1 = p1.clone();

        let un_init = p1.state();
        match un_init {
            PiChanState::Open => println!(""),
            _ => panic!("P1 was in unexpected state! {}", un_init),
        };

        let mut p2 = PiChan::<bool>::new();
        let mut q2 = p2.clone();

        let h = thread::spawn(move || {
            let non_determ = q1.state();
            match non_determ {
                PiChanState::Open => println!("Q1 is open, we are ahead of the main thread"),
                PiChanState::AwaitRecv => {
                    println!("Q1 is awaiting a reciever, we are behind the main thread")
                }
                _ => panic!("Q1 is in an unexpected state!, {}", non_determ),
            };

            let q1_result = q1.recv().expect("Used Chan Err Heard");
            match q1_result {
                Some(y) => {
                    assert!(y);
                }
                None => {
                    panic!("Thread 2: Heard Err Listening to c1");
                }
            };

            let used = q1.state();
            match used {
                PiChanState::Used => println!("Q1 is used as expected"),
                _ => panic!("Q1 was in unexpected state! {}", used),
            };

            let non_determ = q2.state();
            match non_determ {
                PiChanState::Open => println!("Q2 is open, we are ahead of the main thread"),
                PiChanState::AwaitSend => {
                    println!("Q2 is awaiting a sender, we are behind the main thread")
                }
                _ => panic!("Q2 is in an unexpected state!, {}", non_determ),
            };

            q2.send(true).expect("Send on used channel for c2?");

            let used = q2.state();
            match used {
                PiChanState::Used => println!("Q2 is used as expected"),
                _ => panic!("Q2 was in unexpected state! {}", used),
            };
        });

        p1.send(true).expect("Send on used channel for p1");
        let p2_result = p2.recv().expect("Used Chan Err Heard");
        match p2_result {
            Some(y) => {
                assert!(y);
            }
            None => {
                panic!("Thread 2: Heard Err Listening to c1");
            }
        }

        h.join().expect("Failed to Join Threads!");

        let err1 = p1.send(true);
        match err1 {
            Err(_) => println!(""),
            Ok(_) => panic!("Send allowed on closed channel")
        }

        let err2 = p2.recv();
        match err2 {
            Err(_) => println!(""),
            Ok(_) => panic!("Recv allowed on closed channel")
        }
    }
}
