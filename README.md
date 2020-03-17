# Quartz: Crystal Clear Parallelism through Concurrency -- Experimental.

Quartz aims to bring the discoveries of applicable mathematics to the world of concurrent and parallel abrstractions, a world currently dominated by invention. Quartz's internal approach seeks to make no trade off between safety and speed. Quartz's API seeks to be simple, composable, and above all else, reduce the number of "effects" (lock-contention, memory leaks, and deadlocks) a developer needs to reason about when dealing with parallelism. It accomplishes this through the useage of flexible inter-"process"/"thread"/"task" structures that can composed. 

The long term goal is something like a more flexible, barebones [Haxl](https://github.com/facebook/Haxl) for Rust, but clearly, a different approach will be required (and is alluded to in the theory sections).

As it stands, it is a set of a la carte thread-safe communication mechanisms/value containers.

It is currently a work-in-progress and much is left to be achieved.

Learning from the dangers of naming concepts `Monads`, we're going to give these structures names relevant to their useage in day to day computing, but we're also going to outline their mathematical/research heritage. The reason for this isn't trivia, but a formal proof of why these structures should have worthwhile properties, such as being impossible to leak or deadlock free. Additionally, by declaring these structures in concrete, practical, and mathematical terms, we avoid the confusion that ill-defined terms have caused, such as the exact distinction of (actor and channels)[https://core.ac.uk/download/pdf/84869002.pdf]. Questions of bug or feature become easier to navigate as well.

### TODO REFACTOR THE LIBS TO MEET THIS CONVENTION.

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

### Structures:

#### OnceCell -- Thread-Safe Write-Once Variable 
##### In Practice:
A `OnceCell<T>` (where `T: PartialEq`) is useful because it acts like a future that after being fulfilled once, is cached. In practice, it is a variable which is either unwritten to, or is of type `T`. The variable is only transformed once -- from unwritten to `T`. To avoid rejecting deterministic programs, `write` can be called more than once, and only if the subsequent value does not match the first write's value, an error is raised.

A reader can access the value inside of a (cloned or original) `OnceCell` by calling it's methods `read` or `sample`. The first is blocking and returns a `OnceVal<T>` (described below), the second returns a `<Option<OnceVal<T>>>`, with `None` in cases before `write` was concluded.

The `OnceVal<T>` returned by `read` or a successful `sample` in turn also has a `read` method which, unlike the `OnceCell` wrapper, is non-blocking, even though it returns a `RwLockGuard`. How? For usage purposes, it's not important (though do read on in the other sections if you're curious).

The power of this structure is that it can be shared by many reading and writing threads without any contention over locks, only the synchronization of the first write concluding before the first read could be viewed as blocking. The value is availble as soon as it is ready, thread-safe, and compiler-enforced immutable. For more information on situations where multiple concurrent redundant writes might be useful, [here is a relink from above](http://composition.al/blog/2013/09/22/some-example-mvar-ivar-and-lvar-programs-in-haskell/).

`OnceCell` also implements `PartialEq` so `OnceCell`s can contain `OnceCell`s.

##### Implementation and Theory:
By enforcing the condition that only the same thing can written to the `OnceCell`, any subsequent writes can be converted into another read. Thus we have a mechanism by which there is one write then many reads, which nicely pairs with an abstraction over a `RwLock`. Once we know the write lock will never be held again, `read`s of a written `OnceVal` will never block across `n` threads. Only the `OnceCell` has access to the write mechanism, and even then, can only gain it once.

The theory is similiar to the [haskell implementation of IVars](), but includes the relaxation for multiple concurrent writes, it becomes closer to [LVish]() style `LVars`, but without the ability to "grow" -- we will address that idea later.

#### PiChan -- Pi Calculus Channel
Don't let the name intimidate you, it is a simplified `golang` channel that more closely aligns with the Rust borrow checker's line of thinking. 

This structure, along with the definition of channel in Pi Calculus, is a single-use send, single-use recieve, rendezvous channel. Because of it's single-use principle, it is easier prevent leaks and deadlocks. The rendezvous aspect allows it to be used as a synchronizer between threads as well. The single recieve allows the passed value to be `take`n from the `Option`, making this very performant, and the channel disposable.

#### PropCell -- Propagator Cell:
COMING SOON!

#### Spark -- NOT IMPLEMENTED
If determined to be valuable, will resemble the concept of a spark utilized by the [Haskell runtime](https://simonmar.github.io/bib/papers/threadscope.pdf). It would be a structure that accepted a variable of type `T`, a function from types `T -> U`, and exposed a method to retrieve a `U`. 

The act of creating a spark begins the execution of the function acting on the variable asyncronously. The sole attempt to retrieve the value of the spark will either block until the asynchronous function concludes and the `U` is available, or immediately return the precomputed `U`.

Unlike futures offered by the standard library, execution begins with the declaration of the spark, and the function which declares the spark can continue parallel execution without interaction with the spark until calling `read`.

#### WaitGroup -- Thank you Golang and BurntSushi
Source unaltered from: [this abandoned project](https://github.com/BurntSushi/chan/blob/master/src/wait_group.rs)
Tests added.

This is the same concurrency construct (API and all) available in [Golang](https://gobyexample.com/waitgroups). It is similar to a `barrier` available in the standard library but with the act of lowering the `barrier`'s count is now separate from waiting on it. Such a thing becomes very covienent for a dynamic async batching, or async communication between (sets of) threads.

NOTE: This is the only thing in the project that panics -- if the WaitGroup goes below 0 -- which matches the `golang` API. Not neccessarily sold on this implementation.