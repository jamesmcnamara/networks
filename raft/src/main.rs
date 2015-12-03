#![feature(slice_patterns)]
extern crate itertools;
extern crate rustc_serialize;
extern crate unix_socket;

pub mod msg;
pub mod node;

use std::env;

use node::Node;

fn main() {
    let mut args = env::args().skip(1);
    let my_id = match args.next() {
        Some(id) => id,
        None     => panic!("first command line argument must be unix socket name")
    };
    let node = Node::new(my_id, args); 
    
}
