use std::borrow::Borrow;
use std::cell;
use std::collections::HashMap;
use std::convert::From;
use std::io::{Read, Write};
use std::mem;
use std::str::from_utf8;

use itertools::Itertools;
use unix_socket::UnixStream;
use rustc_serialize::json::{encode, Json, ToJson};

use super::msg::{Msg, MsgType};

pub struct Node {
    base: BaseNode,
    node_type: NodeType,
}

impl Node {
    pub fn new<I: Iterator<Item=String>>(id: String, neighbors: I) -> Node {
        Node {
            base: BaseNode::new(id, neighbors),
            node_type: NodeType::Follower,
        }
    }

    pub fn handle_requests(&mut self) {
        let reader = mem::replace(&mut self.base.reader, None).unwrap();
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
        mem::swap(&mut msg.base.src, &mut msg.base.dst);
        mem::replace(&mut msg.msg, MsgType::Fail);
        self.send(msg);
    }

    fn send(&self, msg: Msg) {
        (*self.base.writer.borrow_mut())
            .write_all(encode(&msg.to_json()).unwrap().as_bytes())
            .unwrap()
    }
}

struct BaseNode {
    id: NodeId,
    current_term: usize,
    voted_for: Option<NodeId>,
    log: Vec<String>,
    commit_idx: usize,
    last_applied: usize,
    neighbors: Vec<NodeId>,
    reader: Option<UnixStream>,
    writer: cell::RefCell<UnixStream>,
    state_machine: HashMap<String, String>,
}

impl BaseNode {
    fn new<I: Iterator<Item=String>>(id: String, neighbors: I) -> BaseNode {
        let reader = UnixStream::connect(&id).unwrap();
        let writer = reader.try_clone().unwrap();
        BaseNode {
            id: NodeId::from(id.borrow()),
            current_term: 0,
            voted_for: None,
            log: vec![],
            commit_idx: 0,
            last_applied: 0,
            neighbors: neighbors.map(NodeId::from).collect(),
            reader: Some(reader),
            writer: cell::RefCell::new(writer),
            state_machine: HashMap::new(),
        }
    }
}

enum NodeType {
    Follower,
    Candidate,
    Leader {
        next_index: Vec<usize>,
        match_index: Vec<usize>,
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct NodeId(pub [u8; 4]);

impl NodeId {
    pub fn as_node_id(json: &Json) -> Option<NodeId> {
        NodeId::from_bytes(json
                           .as_string()
                           .expect("node id must be string")
                           .as_bytes()) 
    }

    fn from_bytes(bytes: &[u8]) -> Option<NodeId> {
        match bytes {
            [a, b, c, d] => Some(NodeId([a, b, c, d])),
            _            => None,
        }
    }
}

impl ToJson for NodeId {
    fn to_json(&self) -> Json {
        from_utf8(&self.0).unwrap().to_json()
    }
}

impl <'a>From<&'a str> for NodeId {
    fn from(s: &str) -> NodeId {
       NodeId::from_bytes(s.as_bytes()).expect(&format!("could not parse node id from str: {}", s))
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> NodeId {
       NodeId::from(&s[..])
    }
}

struct Split<I, F> {
    iter: I,
    f: F,
}

trait Splittable<R> : Iterator<Item=R> + Sized {

    fn split<F>(self, f: F) -> Split<Self, F> 
        where F: FnMut(&R) -> bool;
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

impl <R, I: Iterator<Item=R>>Splittable<R> for I {
    fn split<F>(self, f: F) -> Split<Self, F> 
        where F: FnMut(&R) -> bool {
            Split{ iter: self, f: f }
    }
}
