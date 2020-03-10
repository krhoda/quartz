/*
use std::sync::{Arc};
use std::marker::PhantomData;
use std::pin::{Pin};
use pi_chan::PiChan;
use std::thread;

pub struct Spark<T, U>
where
    T: Send,
    U: Send,
{

    // arg: T, // TODO: Learn "Phantom Data"
    arg: PhantomData<*const T>,
    pop: PiChan<U>,
}

impl<T, U> Spark<T, U> 
where
    T: Send,
    U: Send + Clone,
{
    pub fn new(f: Box<dyn Fn(T) -> U + Send>, arg: T) -> Spark<T, U> {
        let mut pop = PiChan::<U>::new();
        let mut pop2 = pop.clone();

        thread::spawn(move || {
            let x = f(arg);
            pop2.send(x);
        });

        Spark::<T, U> { 
            arg: PhantomData,
            pop: pop 
        }
    }

    pub fn get(&self) -> Option<U> {
        self.pop.recv()
    }
}
*/