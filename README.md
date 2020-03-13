# Quartz: Crystal Clear Concurrency -- Experimental.

To paraphrase Philip Wadler, some tools are invented, others are discovered. Quartz aims to bring the discoveries of applicable mathmatics to the world of concurrent abrstractions, a world currently dominated by invention. Quartz's internal approach (via Process Calculi and Propagator Networks) seeks to make no trade off between safety and speed.

It is currently a work-in-progress and much is left to be achieved.

As it stands, it is a set of a la carte thread-safe communication mechanisms.
If all goes well, it might be a Rust Propagator Framework for low/zero-cost futures.

### By The Power of Math and Enthusiasm, I hope to provide:
1) SAFE Concurrency Constructs/Primatives. (So far, 0 lines of `unsafe` code and 0 library dependencies)
2) Freedom from deadlocks. (Idea in Progress, but it boils down to process calculi using ARCs or some sort of propagator framework)
3) Zero-Cost Abastraction Futures. (By way of Lattice Variables and Propagator Networks -- And I will settle for Low-Cost if I have to).
4) Perpetual Motion because if we get 2 and 3, we've got to be on a roll.

### The shoulders of giants:
Implementation and exposed API influences include: 
* BurntSushi's archived [chan library](https://github.com/BurntSushi/chan/) (including the whole-sale copy-paste of the `wait_group` module, getting him top billing) 
* The [Golang concurrency model](https://golang.org/ref/mem) and technique of Rob Pike's myriad of languages.
* The Erlang distributed concurrency model, best introduced through the author of Erlang's [extremely readable PHD thesis](https://www.cs.otago.ac.nz/coursework/cosc461/armstrong_thesis_2003.pdf)
* The Haskell concurrency model, particularly the [Par Monad](https://simonmar.github.io/bib/papers/monad-par.pdf) and in general, the work of Simon Marlow.

Theoretical underpinnings of the unseen guts include:
* The [process calculus](http://usingcsp.com/cspbook.pdf) of Sir Tony Hoare and [others](https://www.researchgate.net/publication/220368672_A_Reflective_Higher-order_Calculus/fulltext/0ffc60670cf255165fc81be2/A-Reflective-Higher-order-Calculus.pdf), including [Pi Calculus](https://en.wikipedia.org/wiki/%CE%A0-calculus)
* The [various](http://groups.csail.mit.edu/genesis/papers/radul%202009.pdf) [works](https://groups.csail.mit.edu/mac/users/gjs/6.945/readings/art.pdf) on [propagators](https://groups.csail.mit.edu/mac/users/gjs/propagators/revised-html.html) as a [model of computation](https://github.com/namin/propagators) of Alexey Radul and Gerrald Sussman
* The [works](https://users.soe.ucsc.edu/~lkuper/papers/lvars-fhpc13.pdf) and [libraries](https://hackage.haskell.org/package/lvish) of Lindsey Kuper and Ryan Newton surrounding lattice variables (and their near cousins joined-semi-lattices)
* The [experimental library](https://github.com/ekmett/guanxi) and [talks](https://www.youtube.com/watch?v=s2dknG7KryQ) of Edward Kmett involving basically all of the above.

### Structure:

#### Overview: 
The subfolders in the workspace are a la carte concurrency structures. Currently, they work only in a threaded concurrency model. Futures will eventually be addressed. Mix, match, pay only for what you need.

The exception is runner, which is a poor excuse for a test suite and will be replaced with proper testing shortly.

The below is a simple explanation of each construct, both practically and theoretically. API/Implementation details will be provided in autogen'd docs one day soon.

#### PiChan -- Pi Calculus Channel
Don't let the name intimidate you, it is a simplified `golang` channel that more closely aligns with the Rust borrow checker's line of thinking. 
This structure, along with the definition of channel in Pi Calculus, is a single-use send, single-use recieve, rendezvous channel. Because of it's single-use principle, it is easier prevent leaks and deadlocks. The rendezvous aspect allows it to be used as a synchronizer between threads as well. The single recieve allows the passed value to be `take`n from the `Option`, making this very performant, and the channel disposable.

#### PropCell -- Propagator Cell:
COMING SOON!

#### PropChan -- Propagator Channel
Named because of its relationship to the work done with Propagator Networks.
A powerful structure required to compose mathematically sound `non-derminisitic execution -> determinisitic result` systems is a future that can be requested more than once, but once fulfilled, will always return the same result -- which is this structure in practice. 

In implementation, it resembles a relaxed version of `PiChan`, retaining the restriction that there is only one sender and one value, but permitting multiple read-only recievers. The send and recieve is also asyncronous -- the sender deposits the value, unblocks any future (or current) reciever, dissuades any future senders, and carries on. `PropChan` retains the `PiChan`'s `recv` behavior, of blocking until the send event occurs, but it permits multiple simultanious listeners.

It also features the non-blocking `sample` which returns a two element tuple: a boolean value indicating whether the send event has occured, and if so, the same value as if you had waited for `recv`, otherwise the second value is a `None`. 

The value that emerges from the `PropChan`, `PropResult`, is essentially the read half of a `RwLock` surrounding the deposited value. As the only writer has already written before the first reader is able to call `read`, it should never block or contain a poison error. Still, the error is exposed through `Result` of `read` in case of freak accident. Thus the result of the async sender's operation is now thread safe, immutable, quasi-lock-less once created, and blocked until created.

#### Spark -- NOT IMPLEMENTED
If determined to be valuable, will resemble the concept of a spark utilized by the [Haskell runtime](https://simonmar.github.io/bib/papers/threadscope.pdf). It would be a structure that accepted a variable of type `T`, a function from types `T -> U`, and exposed a method to retrieve a `U`. 

The act of creating a spark begins the execution of the function acting on the variable asyncronously. The sole attempt to retrieve the value of the spark will either block until the asynchronous function concludes and the `U` is available, or immediately return the precomputed `U`.

Unlike futures offered by the standard library, execution begins with the declaration of the spark, and the function which declares the spark can continue parallel execution without interaction with the spark until calling `read`.

#### WaitGroup -- Thank you BurntSushi
Unaltered from: [this abandoned project](https://github.com/BurntSushi/chan/blob/master/src/wait_group.rs)

This is the same concurrency construct (API and all) available in [Golang](https://gobyexample.com/waitgroups). It is similar to a `barrier` available in the standard library but with the act of lowering the `barrier`'s count is now separate from waiting on it. Such a thing becomes very covienent for a dynamic async batching, or async communication between (sets of) threads.