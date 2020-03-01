use std::sync::{Arc, Condvar, Mutex};

pub type Guard = Arc<(Mutex<bool>, Condvar)>;

#[derive(Debug)]
pub struct UsedChanErr;

pub struct PiChan<T>(Arc<PiC<T>>);
pub struct PiC<T> {
    guard: Guard,
    val: Option<T>,
}

// (Send, Recv) = Chan := Send(Value) -> ReplyRecv := Recv() -> (Value, ReplySender)
// TODO: Implement fancier conversation types.
impl<T> PiChan<T> {
    pub fn new() -> PiChan<T> {
        PiChan(Arc::new(PiC::<T> {
            guard: Arc::new((Mutex::new(false), Condvar::new())),
            val: None,
        }))
    }


    pub fn send(&mut self, t: T) -> Result<(), UsedChanErr> {
        self.0.val = Some(t);

        let (m, c) = self.0.guard.as_ref();
        let used = *m.lock().unwrap();
        if used {
            Err(UsedChanErr)
        } else {
            *m.lock().unwrap() = true;
            c.notify_one();
            Ok(())
        }
    }

    pub fn recv(&mut self) -> Option<T> {
        let (m, c) = self.0.guard.as_ref();

        let _ok = c.wait(m.lock().unwrap()).unwrap();
        self.0.val.take()
    }
}


