# Quartz: Crystal Clear Concurrency.

### By The Power of Math and Enthusiasm, I hope to provide:
1) SAFE Concurrency Constructs/Primatives. (So far, 0 lines of `unsafe` code and 0 library dependencies)
2) Freedom from deadlocks. (Idea in Progress, but it boils down to process calculi using ARCs or some sort of propagator framework)
3) Zero-Cost Abastraction Futures. (By way of Lattice Variables and Propagator Networks -- And I will settle for Low-Cost if I have to).
4) Perpetual Motion because if we get 2 and 3, we've got to be on a roll.

### The shoulders of giants:
Heavily influenced by BurntSushi's archived [chan library](https://github.com/BurntSushi/chan/).
The source is a very valuable and informative read, and I suggest stealing from it. His `wait_group` lives on in this codebase.

### Structure:

#### Overview: 
The subfolders in the workspace are a la carte concurrency structures. Currently, they work only in a threaded concurrency model. Futures will eventually be addressed. Mix, match, pay only for what you need.

The exception is runner, which is a poor excuse for a test suite and will be replaced with proper testing shortly.

#### PiChan -- Pi Calculus Channel
Don't let the name intimidate you, it is a simplified `golang` channel that more closely aligns with the Rust borrow checker's line of thinking. 
This structure, along with the definition of channel in Pi Calculus, is a single-use send, single-use recieve, rendezvous channel. Because of it's single-use principle, it is easier prevent leaks and deadlocks. The rendezvous aspect allows it to be used as a synchronizer between threads as well. The single recieve allows the passed value to be `take`n from the `Option`, making this very performant, and the channel disposable.

#### PropCell -- Propagator Cell:
COMING SOON!

#### PropChan -- Propagator Channel
Named because of its relationship to the work done with Propagator Networks, Lattice Variables, and Edward Kmett's Guanxi.
A powerful structure required to compose mathematically sound `non-derminisitic execution -> determinisitic result` systems is a future that can be requested more than once, but once fulfilled, will always return the same result -- which is this structure in practice. 

In implementation, it resemebles a relaxed version of `PiChan`, retaining the restriction that there is only one sender and one value, but permitting multiple read-only recievers. The send and recieve is also asyncronous -- the sender deposits the value, unlocks any future (or current) reciever, blocks any future senders, and carries on. `PropChan` retains the `PiChan`'s `recv` behavior, of blocking until the send event occurs, but it permits multiple simultanious listeners.
It also features the non-blocking `sample` which returns a  boolean value indicating whether the send event has occured, and if so, the same value as if you had waited for `recv`. 

The value that emerges from the `PropChan`, `PropResult`, is essentially the read half of a RwLock surrounding the deposited value. As the only writer has already written before the first reader is able to call `read`, it should never block or contain a poison error. Still, the error is exposed through `Result` of `read` in case of freak accident. Thus the result of the async sender's operation is now thread safe, immutable, quasi-lock-less once created, and blocked until created.