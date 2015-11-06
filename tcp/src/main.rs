#![feature(convert)]
#![feature(mpsc_select)]
#![feature(slice_patterns)]
extern crate itertools;
extern crate schedule_recv;
extern crate time;
extern crate rand;
extern crate rustc_serialize;

use std::io::{Read, BufReader, stdin, Write};
use std::net::UdpSocket;
use std::thread;
use std::env;

use rand::random;


/// Logs a timestamped format string and some optional arguments to stderr
macro_rules! log (
    ($fmt_string:expr, $($args:tt)*) => {{
        writeln!(&mut ::std::io::stderr(), 
                concat!("<{}> ", $fmt_string),
                ::time::now().strftime("%H:%M:%S.%f").unwrap(),
                $($args)*)
            .ok()
            .expect("logging failed");
    }
});

pub mod socket;
pub mod packet;


/// Attempts to bind to random UDP ports until it succeeds and returns a socket 
fn open_socket() -> UdpSocket {
    loop {
        let k = 1024 + random::<u16>();
        if let Ok(socket) = UdpSocket::bind(("127.0.0.1", k)) {
            log!("[bound] {}", k);
            return socket;
        }
    }
}

fn main() {
    let stdin_bytes = BufReader::new(stdin()).bytes();
    let local = open_socket();
    let recvr = if let Some(addr) = env::args().skip(1).next() {
        // Sender
        let (sender, recvr) = socket::open_sender(local, addr.as_str());
        thread::spawn(move || sender.send(stdin_bytes));
        recvr
    } else {
        // Receiver
        socket::open_recvr(local)
    };
    recvr.recv();
}
