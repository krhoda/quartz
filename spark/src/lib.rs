/*
use std::sync::{Arc};
use std::marker::PhantomData;
use pi_chan::PiChan;
use std::thread;

pub struct Spark<'a, T, U>
where
    T: Send,
    U: Send,
{

    arg: PhantomData<&'a T>,
    pop: PiChan<U>,
}

impl<'a, T, U> Spark<'a, T, U> 
where
    T: Send,
    U: Send + Clone,
{
    pub fn new(f: Box<dyn FnOnce(T) -> U + Send>, arg: T) -> Spark<'a, T, U> 
    {
        let mut pop = PiChan::<U>::new();
        let mut fuse = pop.clone();

        thread::spawn(move || {
            let x = f(arg);
            fuse.send(x);
            println!("hello spark")
        });

        Spark::<T, U> { 
            arg: PhantomData,
            pop: pop 
        }
    }

    pub fn get(&mut self) -> Option<U> {
        self.pop.recv()
    }
}
*/