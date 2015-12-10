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

use super::msg::{BaseMsg, Entry, InternalMsg, Msg, MsgType};

enum SelResult {
    Timeout,
    ClientMsg,
    NodeMsg,
}

pub struct Node {
    base: BaseNode,
    node_type: NodeType,
}

impl Node {
    pub fn new<I>(reader: mpsc::Receiver<Msg>, socket: UnixStream, id: String, neighbors: I) -> Node
        where I: Iterator<Item=String>
    {
        Node {
            base: BaseNode::new(reader, socket, id, neighbors),
            node_type: NodeType::Follower,
        }
    }

    pub fn main(mut self) {
        let mut rng = thread_rng();
        let mut timer = oneshot_ms(150 + (rng.gen::<u32>() % 150u32));
        loop {
            match {
                let chan = &self.base.reader;
                select! {
                    msg = chan.recv() => self.handle_message(msg.unwrap()),
                    _   = timer.recv() => SelResult::Timeout
                }
            } {
                SelResult::Timeout => {
                    self.into_candidate();
                    timer = oneshot_ms(150 + (rng.gen::<u32>() % 150u32));
                },
                SelResult::NodeMsg => {
                    timer = oneshot_ms(150 + (rng.gen::<u32>() % 150u32));
                },
                SelResult::ClientMsg => (),
            }
        }

    }

    fn handle_message(&self, mut msg: Msg) -> SelResult {
        let res = match msg.msg {
            MsgType::Get(_)
                | MsgType::Put(..)
                | MsgType::OK(_)
                | MsgType::Redirect
                | MsgType::Fail => SelResult::ClientMsg,
            _  => SelResult::NodeMsg,
        };
        mem::swap(&mut msg.base.src, &mut msg.base.dst);
        mem::replace(&mut msg.msg, MsgType::Fail);
        self.send(&msg);

        res
    }

    fn into_candidate(&mut self) {
        println!("{:?} is becoming a candidate", self.base.id);
        mem::replace(&mut self.node_type, NodeType::Candidate);
        self.base.current_term += 1;
        self.base.voted_for = Some(self.base.id);
        for node in &self.base.neighbors {
            self.send_request_vote(*node);
        }
    }


    fn send_request_vote(&self, to: NodeId) {
        let last_entry_term = self
            .base
            .log
            .last()
            .map_or(self.base.current_term, |entry| entry.term);

        let details = InternalMsg::new(self.base.current_term,
                                       self.base.log.len() as u64,
                                       last_entry_term);

        let base = BaseMsg::new(self.base.id,
                                to,
                                NodeId::broadcast(),
                                "rv".to_owned());
        let rv = Msg {
            base: base,
            msg: MsgType::RequestVote {
               details: details,
               candidate_id: self.base.id,
            },
        };

        self.send(&rv)
    }


    fn send(&self, msg: &Msg) {
        (*self.base.writer.borrow_mut())
            .write_all((encode(&msg.to_json()).unwrap() + "\n").as_bytes())
            .unwrap()
    }
}

struct BaseNode {
    id: NodeId,
    current_term: u64,
    voted_for: Option<NodeId>,
    log: Vec<Entry>,
    commit_idx: u64,
    last_applied: u64,
    neighbors: Vec<NodeId>,
    reader: mpsc::Receiver<Msg>,
    writer: cell::RefCell<UnixStream>,
    state_machine: HashMap<String, String>,
}

impl BaseNode {
    fn new<I>(reader: mpsc::Receiver<Msg>, writer: UnixStream, id: String, neighbors: I) -> BaseNode
        where I: Iterator<Item=String>
    {
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

    fn broadcast() -> NodeId {
        NodeId(['F' as u8, 'F' as u8, 'F' as u8, 'F' as u8])
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

