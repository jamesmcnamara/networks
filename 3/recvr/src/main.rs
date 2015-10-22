use std::net::UdpSocket;
use std::io::{Read, BufRead, BufReader, stdin};

fn main() {
    let sock = match UdpSocket::bind("localhost:23456") {
        Ok(sock) => sock,
        Err(e)   => panic!("Error occured opening socket: {}", e),
    };
    let sender = ("localhost", 12345);
    let mut payload: Vec<u8> = vec![];
    loop {
        match sock.recv_from(&mut payload) {
            Ok((0, _))    => {continue;},
            Ok((n, addr)) => {println!("received {} bytes from {}", n, addr);},
            Err(_)        => {continue;},
        }
        println!("{:?}", payload);
    }
}
