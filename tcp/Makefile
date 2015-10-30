RUST_DYLD := /home/jamcnam//rust/lib/rustlib/x86_64-unknown-linux-gnu/lib
export PATH := /home/jamcnam/rust/bin:$(PATH)
export DYLD_LIBRARY_PATH:=$(RUST_DYLD):$(DYLD_LIBRARY_PATH)
export LD_LIBRARY_PATH:=/usr/local/lib:/home/jamcnam/rust/lib

go: 
	cargo build --release 
	cp target/release/sender 3700send
	cp 3700send 3700recv
