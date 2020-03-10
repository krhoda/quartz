use pi_chan;

// IN PROGRESS.
// use spark;

use std::thread;
use wait_group;

fn main() {
    println!("Hello, from Thread 1!");
    run_pi_chan();
    run_wait_group();
    pi_chan_state();
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
        let x = c_i.recv().expect("Used Chan Err Heard");
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
    let b = c2.recv().expect("Used Chan Err Heard");
    match b {
        Some(c) => {
            println!("Thread 1: Heard {}", c);
        }
        None => {
            println!("Thread 2: Heard Err Listening to c1");
        }
    }
}

fn pi_chan_state() {
    let mut p1 = pi_chan::PiChan::<usize>::new();
    let mut q1 = p1.clone();

    let mut p2 = pi_chan::PiChan::<usize>::new();
    let mut q2 = p2.clone();

    let un_init = p1.state();
    match un_init {
        pi_chan::PiChanState::Open => println!("P1 is open as expected"),
        _ => println!("P1 was in unexpected state! {}", un_init),
    }

    let un_init = p2.state();
    match un_init {
        pi_chan::PiChanState::Open => println!("P2 is open as expected"),
        _ => println!("P2 was in unexpected state! {}", un_init),
    }

    let h = thread::spawn(move || {
        let non_determ = q1.state();
        match non_determ {
            pi_chan::PiChanState::Open => println!("Q1 is open, we are ahead of the main thread"),
            pi_chan::PiChanState::AwaitRecv => {
                println!("Q1 is awaiting a reciever, we are behind the main thread")
            }
            _ => println!("Q1 is in an unexpected state!, {}", non_determ),
        }
        q1.recv().expect("ERR IN Q1");

        let used = q1.state();
        match used {
            pi_chan::PiChanState::Used => println!("Q1 is used as expected"),
            _ => println!("Q1 was in unexpected state! {}", used),
        }

        let non_determ = q2.state();
        match non_determ {
            pi_chan::PiChanState::Open => println!("Q2 is open, we are ahead of the main thread"),
            pi_chan::PiChanState::AwaitSend => println!("Q2 is awaiting a sender, we are behind the main thread"),
            _ => println!("Q1 is in an unexpected state!, {}", non_determ),
        }

        q2.send(1).expect("ERR IN Q2");

        let used = q2.state();
        match used {
            pi_chan::PiChanState::Used => println!("Q2 is used as expected"),
            _ => println!("Q2 was in unexpected state! {}", used),
        }

        println!("Goodbye from thread 2");
    });

    p1.send(1).expect("ERR IN P1");
    p2.recv().expect("ERR IN P2");

    let used = p1.state();
    match used {
        pi_chan::PiChanState::Used => println!("P1 is used as expected"),
        _ => println!("P1 was in unexpected state! {}", un_init),
    }
    h.join();
    println!("Goodbye from thread 1");
}

// fn test_spark() {
// }
// fn contrived_add(x: usize, y: usize) -> usize {
// }
