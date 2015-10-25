#![feature(slice_patterns)]
#![feature(mpsc_select)]
extern crate itertools;
extern crate packet;
extern crate schedule_recv;

use itertools::Itertools;

use packet::packet::Packet;

use std::net::UdpSocket;
use std::io::{Read, BufRead, BufReader, stdin};
use std::thread;
use std::env;

pub mod socket;

fn main() {
    let mut stdin_bytes = BufReader::new(stdin()).bytes();
    let (host, dest) = match &env::args().skip(1).collect::<Vec<_>>()[..] {
        [ref host, ref dest, ..] => (host.to_string(), dest.to_string()),
        _            => panic!("Must pass parameters of host and destination"),
    };
    let (mut sender, mut recvr) = socket::open_connection(&host, &dest); 
    let _ = thread::spawn(move|| { sender.send(stdin_bytes) });
    recvr.recv();
}
