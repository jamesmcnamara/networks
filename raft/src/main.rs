#![feature(mpsc_select)]
#![feature(slice_patterns)]
extern crate itertools;
extern crate rand;
extern crate rustc_serialize;
extern crate schedule_recv;
extern crate unix_socket;

pub mod msg;
pub mod node;
pub mod port;

use std::env;
use std::thread;
use std::sync::mpsc;

use unix_socket::UnixStream;

use node::Node;
use port::Port;

fn main() {
    let mut args = env::args().skip(1);
    let my_id = match args.next() {
        Some(id) => id,
        None     => panic!("first command line argument must be unix socket name")
    };

    let (sender, receiver) = mpsc::channel();

    let right_sock = UnixStream::connect(&my_id).unwrap();
    let left_sock = right_sock.try_clone().unwrap();

    let port = Port::new(right_sock, sender);
    thread::spawn(move || port.relay());
    let mut node = Node::new(receiver, left_sock, my_id, args);
    node.main();
}
