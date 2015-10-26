#![feature(convert)]
#![feature(mpsc_select)]
#![feature(slice_patterns)]
extern crate itertools;
extern crate schedule_recv;
extern crate rustc_serialize;

use std::io::{Read, BufReader, stdin};
use std::thread;
use std::env;

pub mod socket;
pub mod packet;

fn main() {
    let stdin_bytes = BufReader::new(stdin()).bytes();
    let (host, dest) = match &env::args().skip(1).collect::<Vec<_>>()[..] {
        [ref host, ref dest, ..] => (host.to_string(), dest.to_string()),
        _            => panic!("Must pass parameters of host and destination"),
    };
    let (mut sender, mut recvr) = socket::open_connection(&host, &dest); 
    thread::spawn(move || sender.send(stdin_bytes));
    recvr.recv();
}
