export PATH := /home/jamcnam/rust/bin:$(PATH)
export DYLD_LIBRARY_PATH:=$(rust_dyld):$(DYLD_LIBRARY_PATH)
export LD_LIBRARY_PATH:=/usr/local/lib:/home/jamcnam/rust/lib

go: 
	which cargo
	cargo build --release 
	cp target/release/sender 3700send
	cp 3700send 3700recv
