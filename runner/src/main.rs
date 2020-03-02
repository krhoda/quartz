use quartz;
use std::thread;

fn main() {
    println!("Hello, from Thread 1!");
    run_pi_chan();
    run_wait_group()
}

fn run_wait_group() {
    let wg = quartz::WaitGroup::new();

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
    let mut c1 = quartz::PiChan::<usize>::new();
    let mut cI = c1.clone();

    let mut c2 = quartz::PiChan::<usize>::new();
    let mut cII = c2.clone();

    thread::spawn(move || {
        println!("Hello, from Thread 2!");
        let z: usize = 8;
        let x = cI.recv();
        match x {
            Some(y) => {
                println!("Thread 2: Heard {}", y);
                cII.send(z).expect("Send on used channel for c2?");
            }
            None => {
                println!("Thread 2: Heard Err Listening to c1");
                cII.send(z).expect("Send on used channel for c2?");
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
