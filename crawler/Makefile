RUST_DYLD := /home/jamcnam/rust/lib/rustlib/x86_64-unknown-linux-gnu/lib
export PATH := /home/jamcnam/rust/bin:$(PATH)
export DYLD_LIBRARY_PATH:=$(RUST_DYLD):$(DYLD_LIBRARY_PATH)
export LD_LIBRARY_PATH:=/usr/local/lib:/home/jamcnam/rust/lib

default:
	cargo build --release
	mv target/release/webcrawler .

run: default
	./webcrawler 000539023 EA3GGNHC

test: default
	cargo test
