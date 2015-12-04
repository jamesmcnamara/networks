#![feature(slice_patterns)]
extern crate itertools;
extern crate rustc_serialize;
extern crate unix_socket;

pub mod msg;
pub mod node;

use std::env;

use node::Node;

fn main() {
    println!("i'm alive!");
    let mut args = env::args().skip(1);
    let my_id = match args.next() {
        Some(id) => id,
        None     => panic!("first command line argument must be unix socket name")
    };
    let mut node = Node::new(my_id, args); 
    node.handle_requests();    
}
