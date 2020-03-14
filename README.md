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
* The [works](https://users.soe.ucsc.edu/~lkuper/papers/lvars-fhpc13.pdf) and [libraries](https://hackage.haskell.org/package/lvish) of Lindsey Kuper and Ryan Newton surrounding lattice variables (and their [near cousins](http://composition.al/blog/2013/09/22/some-example-mvar-ivar-and-lvar-programs-in-haskell/))
* The [experimental library](https://github.com/ekmett/guanxi) and [talks](https://www.youtube.com/watch?v=s2dknG7KryQ) of Edward Kmett involving basically all of the above.

### Structure:

#### Overview: 
The subfolders in the workspace are a la carte concurrency structures, complete with module level testing. Currently, they work only in a threaded concurrency model. Futures will eventually be addressed. Mix, match, pay only for what you need.

The below is a simple explanation of each construct, both practically and theoretically. API/Implementation details will be provided in autogen'd docs one day soon.

#### IVar -- Immutable (Runtime Instantiated) Variable 
Regardless of the opaque name, `IVar`s are a powerful structure required to compose determinisitic results out of non-deterministic execution. From a practical perspective, an `IVar` is future with a cached result which is thread-safe and read-only once established. Only a single mutation occurs in the `IVar::<T>`'s internal state, going from `None` to `Some(T)`. 

No reader is permitted access to the value when it is `None`. No write occurs after the first transformation. If a subsequent write is attempted, the value of the later write is checked against the initial write and if a difference is detected, only then an error is raised. This allows multiple fulfillers and consumers of the `IVar`, as long as all fulfillers are consistent in their final result. Other than the initial synchronzation of the first write occuring before the first read, the data structure is essentially lockless.

Before the first `write` occurs, attempts to to `read` the `IVar` will block. A non-blocking varient `sample` returns a tuple with the first element a boolean answer to whether the write event has occured, and the latter an `IVal` containing `None` or the result of the `write`.

Once the `write` has occurred, all prospective `read`ers recieve an `IVal` which is a thread-safe, read-only wrapper around the value written. The value can be accessed through the `IVal`'s read method, which returns read lock guard from a `RwLock`.Subsequent `write`s only acquire this read access and compare the inner value against their attempt.

Because there is only one write which occurs before any read lock can be distributed, the lock only acts as a promise of immutability, and will never be contested.

Since `IVar` has `ParitalEq` implemented, `IVar`s can contain `IVar`s.

#### PiChan -- Pi Calculus Channel
Don't let the name intimidate you, it is a simplified `golang` channel that more closely aligns with the Rust borrow checker's line of thinking. 

This structure, along with the definition of channel in Pi Calculus, is a single-use send, single-use recieve, rendezvous channel. Because of it's single-use principle, it is easier prevent leaks and deadlocks. The rendezvous aspect allows it to be used as a synchronizer between threads as well. The single recieve allows the passed value to be `take`n from the `Option`, making this very performant, and the channel disposable.

#### PropCell -- Propagator Cell:
COMING SOON!

#### Spark -- NOT IMPLEMENTED
If determined to be valuable, will resemble the concept of a spark utilized by the [Haskell runtime](https://simonmar.github.io/bib/papers/threadscope.pdf). It would be a structure that accepted a variable of type `T`, a function from types `T -> U`, and exposed a method to retrieve a `U`. 

The act of creating a spark begins the execution of the function acting on the variable asyncronously. The sole attempt to retrieve the value of the spark will either block until the asynchronous function concludes and the `U` is available, or immediately return the precomputed `U`.

Unlike futures offered by the standard library, execution begins with the declaration of the spark, and the function which declares the spark can continue parallel execution without interaction with the spark until calling `read`.

#### WaitGroup -- Thank you BurntSushi
Source unaltered from: [this abandoned project](https://github.com/BurntSushi/chan/blob/master/src/wait_group.rs)
Tests added.

This is the same concurrency construct (API and all) available in [Golang](https://gobyexample.com/waitgroups). It is similar to a `barrier` available in the standard library but with the act of lowering the `barrier`'s count is now separate from waiting on it. Such a thing becomes very covienent for a dynamic async batching, or async communication between (sets of) threads.

NOTE: This is the only thing in the project that panics -- if the WaitGroup goes below 0 -- which matches the `golang` API. Not neccessarily sold on this implementation.