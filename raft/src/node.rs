use std::borrow::Borrow;
use std::cell;
use std::collections::HashMap;
use std::convert::From;
use std::io::{Read, Write};
use std::mem;
use std::str::from_utf8;
use std::sync::mpsc;

use itertools::Itertools;
use rand::{Rng, thread_rng};
use rustc_serialize::json::{encode, Json, ToJson};
use schedule_recv::oneshot_ms;
use unix_socket::UnixStream;

use super::msg::{Msg, MsgType, Entry};

pub struct Node {
    base: BaseNode,
    node_type: NodeType,
}

impl Node {
    pub fn new<I: Iterator<Item=String>>(reader: mpsc::Receiver<Msg>, socket: UnixStream, id: String, neighbors: I) -> Node {
        Node {
            base: BaseNode::new(reader, socket, id, neighbors),
            node_type: NodeType::Follower,
        }
    }

    pub fn main(mut self) {
        loop {
            let mut rng = thread_rng();
            {
                let chan = &self.base.reader;
                let timer = oneshot_ms(150 + (rng.gen::<u32>() % 150u32));
                select! {
                    msg = chan.recv() => {
                        self.handle_message(msg.unwrap());
                        continue;
                    },
                    _   = timer.recv() => ()
                }
            }
            self.into_candidate() 
        }

    }

    fn handle_message(&self, mut msg: Msg) {
        mem::swap(&mut msg.base.src, &mut msg.base.dst);
        mem::replace(&mut msg.msg, MsgType::Fail);
        self.send(msg);
    }

    fn into_candidate(&mut self) {

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
    log: Vec<Entry>,
    commit_idx: usize,
    last_applied: usize,
    neighbors: Vec<NodeId>,
    reader: mpsc::Receiver<Msg>,
    writer: cell::RefCell<UnixStream>,
    state_machine: HashMap<String, String>,
}

impl BaseNode {
    fn new<I: Iterator<Item=String>>(reader: mpsc::Receiver<Msg>, writer: UnixStream, id: String, neighbors: I) -> BaseNode {
        BaseNode {
            id: NodeId::from(id.borrow()),
            current_term: 0,
            voted_for: None,
            log: vec![],
            commit_idx: 0,
            last_applied: 0,
            neighbors: neighbors.map(NodeId::from).collect(),
            reader: reader,
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

