use quartz;
use std::thread;

fn main() {
    println!("Hello, from Thread 1!");

    // let mut c1 = quartz::PiChan::<usize>::new();
    // let mut c2 = quartz::PiChan::<usize>::new();
    // thread::spawn(move || {
    //     println!("Hello, from Thread 2!");
    //     let z: usize = 8;
    //     let x = c1.recv();
    //     match x {
    //         Some(y) => {
    //             println!("Thread 2: Heard {}", y);
    //             c2.send(z).expect("Send on used channel for c2?");
    //         }
    //         None => {
    //             println!("Thread 2: Heard Err Listening to c1");
    //             c2.send(z).expect("Send on used channel for c2?");
    //         }
    //     }
    // });

    // let a: usize = 1;
    // c1.send(a).expect("Send on used channel for c1?");
    // let b = c2.recv();
    // match b {
    //     Some(c) => {
    //         println!("Thread 1: Heard {}", c);
    //     }
    //     None => {
    //         println!("Thread 2: Heard Err Listening to c1");
    //     }
    // }

}
