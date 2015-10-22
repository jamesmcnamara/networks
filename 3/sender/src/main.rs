extern crate itertools;
extern crate packet;

use itertools::Itertools;

use packet::Packet;

use std::net::UdpSocket;
use std::io::{Read, BufRead, BufReader, stdin};

fn main() {
    let mut stdin_bytes = BufReader::new(stdin()).bytes();
    let sock = match UdpSocket::bind("localhost:12345") {
        Ok(sock) => sock,
        Err(e)   => panic!("Error occured opening socket: {}", e),
    };
    let recvr = ("localhost", 23456);
    for byte_block in &stdin_bytes.into_iter().chunks_lazy(1500) {
        let packet: Vec<_> = byte_block
            .map(|v| match v {
                Ok(b)  => b,
                Err(_) => panic!("Failed reading from stdin!"),
            })
            .collect();
        match sock.send_to(&packet, recvr) {
            Ok(n)  => {println!("sent {} bytes", n);},
            Err(e) => panic!(e),
        }
    }

}
