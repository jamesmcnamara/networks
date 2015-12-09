use std::io::{Read, Write};
use std::mem;
use std::sync::mpsc;

use unix_socket::UnixStream;

use super::msg::{Msg, MsgType, Entry};

pub struct Port {
    socket: Option<UnixStream>,
    sender: mpsc::Sender<Msg>,
}

impl Port {
    pub fn new(socket: UnixStream, sender: mpsc::Sender<Msg>) -> Port {
        Port {
            socket: Some(socket),
            sender: sender,
        }
    }

    pub fn relay(mut self) {
        let reader = mem::replace(&mut self.socket, None).unwrap();
        let msgs = reader.bytes() 
            .map(Result::unwrap)
            .split(|byte| '\n' != *byte as char);
        for byte_block in msgs {
            match String::from_utf8(byte_block) {
                Ok(msg) => self.handle_message(msg),
                Err(e)  => panic!("error! {}", e),
            }
        }
    }

    fn handle_message(&self, req: String) {
        println!("message is {}", req);
        let mut msg = Msg::from_str(&req);
        self.sender.send(msg);
    }
}

struct Split<I, F> {
    iter: I,
    f: F,
}

impl <I: Iterator, F>Iterator for Split<I, F> where F: FnMut(&I::Item) -> bool {
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut out = vec![];
        for item in self.iter.by_ref() {
            if (self.f)(&item) {
                out.push(item);
            } else {
                return Some(out);
            }
        }
        None
    }
}

trait Splittable<R> : Iterator<Item=R> + Sized {

    fn split<F>(self, f: F) -> Split<Self, F> 
        where F: FnMut(&R) -> bool;
}

impl <R, I: Iterator<Item=R>>Splittable<R> for I {
    fn split<F>(self, f: F) -> Split<Self, F> 
        where F: FnMut(&R) -> bool {
            Split{ iter: self, f: f }
    }
}
