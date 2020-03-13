use pi_chan::{PiChan, PiChanState};
use prop_chan::PropChan;

// IN PROGRESS.
// use spark;

use std::thread;

fn main() {
    println!("Hello, from Thread 1!");
    run_pi_chan();
    pi_chan_state();
    test_prop_chan();
}

// fn run_wait_group() {
//     let wg = wait_group::WaitGroup::new();

//     for _ in 0..4 {
//         wg.add(1);
//         let wg = wg.clone();
//         thread::spawn(move || {
//             // do some work.
//             println!("Hello from Thread 2!");
//             // And now call done.
//             wg.done();
//         });
//     }

//     wg.wait();
//     println!("Goodbye from Thread 1!");
// }

fn run_pi_chan() {
    let mut c1 = PiChan::<usize>::new();
    let mut c_i = c1.clone();

    let mut c2 = PiChan::<usize>::new();
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
    let mut p1 = PiChan::<usize>::new();
    let mut q1 = p1.clone();

    let mut p2 = PiChan::<usize>::new();
    let mut q2 = p2.clone();

    let un_init = p1.state();
    match un_init {
        PiChanState::Open => println!("P1 is open as expected"),
        _ => println!("P1 was in unexpected state! {}", un_init),
    }

    let un_init = p2.state();
    match un_init {
        PiChanState::Open => println!("P2 is open as expected"),
        _ => println!("P2 was in unexpected state! {}", un_init),
    }

    let h = thread::spawn(move || {
        let non_determ = q1.state();
        match non_determ {
            PiChanState::Open => println!("Q1 is open, we are ahead of the main thread"),
            PiChanState::AwaitRecv => {
                println!("Q1 is awaiting a reciever, we are behind the main thread")
            }
            _ => println!("Q1 is in an unexpected state!, {}", non_determ),
        }
        q1.recv().expect("ERR IN Q1");

        let used = q1.state();
        match used {
            PiChanState::Used => println!("Q1 is used as expected"),
            _ => println!("Q1 was in unexpected state! {}", used),
        }

        let non_determ = q2.state();
        match non_determ {
            PiChanState::Open => println!("Q2 is open, we are ahead of the main thread"),
            PiChanState::AwaitSend => {
                println!("Q2 is awaiting a sender, we are behind the main thread")
            }
            _ => println!("Q1 is in an unexpected state!, {}", non_determ),
        }

        q2.send(1).expect("ERR IN Q2");

        let used = q2.state();
        match used {
            PiChanState::Used => println!("Q2 is used as expected"),
            _ => println!("Q2 was in unexpected state! {}", used),
        }

        println!("Goodbye from thread 2");
    });

    p1.send(1).expect("ERR IN P1");
    p2.recv().expect("ERR IN P2");

    let used = p1.state();
    match used {
        PiChanState::Used => println!("P1 is used as expected"),
        _ => println!("P1 was in unexpected state! {}", un_init),
    }
    h.join().expect("Failed to Join Threads!");
    println!("Goodbye from thread 1");
}

fn test_prop_chan() {
    let mut p1 = PropChan::<usize>::new();
    let mut q1 = p1.clone();

    let (intresting, _) = p1.sample().unwrap();
    match intresting {
        true => println!("Recieved true from sample when it should've returned false"),
        _ => println!("Sample returned with the expected -- false -- result")
    };


    let h = thread::spawn(move || {
        let r = q1.recv();
        match r {
            Err(x) => println!("Oh No! Err in Q1 recieve: {}", x),
            Ok(a) => match *a.read().unwrap() {
                None => println!("Got 'None' in Q1 recieve"),
                Some(b) => println!("Got {} in Q1 recieve", b),
            },
        };

        println!("One more time from thread 2!!!");
        let r = q1.recv();
        match r {
            Err(x) => println!("Oh No! Err in Q1 recieve: {}", x),
            Ok(a) => match *a.read().unwrap() {
                None => println!("Got 'None' in Q1 recieve"),
                Some(b) => println!("Got {} in Q1 recieve", b),
            },
        };
    });

    p1.send(1).unwrap();
    let e = p1.send(2);
    match e {
        Err(x) => println!("Got expected err: {}", x),
        Ok(_) => println!("Got unexpected success in second send!?"),
    }

    let should_be_one = p1.recv();
    match should_be_one {
        Err(x) => println!("Oh No! Err in P1 recieve: {}", x),
        Ok(a) => match *a.read().unwrap() {
            None => println!("Got 'None' in P1 recieve"),
            Some(b) => println!("Got {} in P1 recieve", b),
        },
    };

    println!("Will wait for thread 2");
    h.join().expect("Failed to Join Threads!");
    println!("Goodbye from thread 1");
}

// fn test_spark() {
// }
// fn contrived_add(x: usize, y: usize) -> usize {
// }
