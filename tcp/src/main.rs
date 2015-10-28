#![feature(convert)]
#![feature(iter_cmp)]
#![feature(mpsc_select)]
#![feature(slice_patterns)]
extern crate itertools;
extern crate schedule_recv;
extern crate time;
extern crate rustc_serialize;

use std::io::{Read, BufReader, stdin, Write};
use std::thread;
use std::env;

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

fn main() {
    let stdin_bytes = BufReader::new(stdin()).bytes();

    // Gnarly code to slice out the second and third arguments
    let (host, dest) = match &env::args().skip(1).collect::<Vec<_>>()[..] {
        [ref host, ref dest, ..] => (host.to_string(), dest.to_string()),
        _            => panic!("Must pass parameters of host and destination"),
    };
    let (mut sender, mut recvr) = socket::open_connection(&host, &dest); 
    thread::spawn(move || sender.send(stdin_bytes));
    recvr.recv();
}
