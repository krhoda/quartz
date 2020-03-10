use pi_chan;

// IN PROGRESS.
// use spark;

use wait_group;
use std::thread;

fn main() {
    println!("Hello, from Thread 1!");
    run_pi_chan();
    run_wait_group()
}

fn run_wait_group() {
    let wg = wait_group::WaitGroup::new();

    for _ in 0..4 {
        wg.add(1);
        let wg = wg.clone();
        thread::spawn(move || {
            // do some work.
            println!("Hello from Thread 2!");
            // And now call done.
            wg.done();
        });
    }

    wg.wait();
    println!("Goodbye from Thread 1!");
}

fn run_pi_chan() {
    let mut c1 = pi_chan::PiChan::<usize>::new();
    let mut c_i = c1.clone();

    let mut c2 = pi_chan::PiChan::<usize>::new();
    let mut c_ii = c2.clone();

    thread::spawn(move || {
        println!("Hello, from Thread 2!");
        let z: usize = 8;
        let x = c_i.recv();
        match x {
            Some(y) => {
                println!("Thread 2: Heard {}", y);
                c_ii.send(z).expect("Send on used channel for c2?");
            }
            None => {
                println!("Thread 2: Heard Err Listening to c1");
                c_ii.send(z).expect("Send on used channel for c2?");
            }
        }
    });

    let a: usize = 1;
    c1.send(a).expect("Send on used channel for c1?");
    let b = c2.recv();
    match b {
        Some(c) => {
            println!("Thread 1: Heard {}", c);
        }
        None => {
            println!("Thread 2: Heard Err Listening to c1");
        }
    }

}

fn test_spark() {

}


fn contrived_add(x: usize, y: usize) -> usize {
    x + y
}