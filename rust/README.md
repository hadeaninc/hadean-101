Rust
====

 - [`simple-send-recv.rs`](simple-send-recv.rs) -
     simple spawn, send and receive example
 - [`ga.rs`](ga.rs) -
     genetic algorithm using spawn, send and receive
 - [`list-and-hashmap.rs`](list-and-hashmap.rs) -
     demonstration of the capabilities of `List<T>` and `HashMap<K, V>`
     (currently in alpha)

You can run `simple-send-recv.rs` and `ga.rs` with
```
$ cargo run --bin simple-send-recv
$ cargo run --bin ga -- 5
```
Note that`ga.rs` takes a number as an argument to specify the number of workers to use.
