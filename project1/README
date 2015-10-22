# Socket Basics
## Development
The development of this project was quite straight forward. We first parsed
command line arguments using argparse, a Rust crate inspired by python's module
of the same name. We used the results to open a TCP connection and began 
transmitting messages back and forth, parsing math problems and formatting 
answers along the way.

## Issues
We ran into a number of issues along the way, some due to the CCS machines, some due to the instability of Rust in its current state.

* TCP Streams in Rust by default use a `read` call that blocks until `EOF`. However, they do provide a `Chars` iterator, which produces single chars as they become available.
* The `Chars` iterator however is marked as unstable, and thus our code using it would not compile on the lab computers. The only option this left us was the `Bytes` iterator. Luckily, we recognized that the first 127 code points of unicode correspond to the ASCII character set, and thus interpreting one byte at a time as Unicode letters was appropriate.
* For some reason, the Rust package manager (`cargo`) on the CCS computers is malfunctioning. It believed that it had compiled the right supporting libraries for our project, but the binary was completely missing a symbol table, and lead to linker errors. Luckily, `cargo` is quite extensible and we were able to override the dependency with a github link and revision number, and the correct version was used.
* Rust is hard, man. 

## Testing
We used a few methodologies for testing. We wrote a few manual tests for our solver to ensure it behaved predictably. Additionally, we wrote a TCP server in Python to debug our connection issues and feed predictable data. But we mostly relied on the safety guarantees that the Rust language provides on compilation.
