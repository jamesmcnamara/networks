use std::borrow::Borrow;
use std::collections::HashMap;
use std::convert::From;
use std::io::Read;
use std::str::from_utf8;

use unix_socket::UnixStream;
use rustc_serialize::json::{Json, ToJson};

use super::msg::Msg;

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

    pub fn handle_request(&mut self) {
        loop {
            let mut req = String::new();
            match self.base.port.read_to_string(&mut req) {
                Ok(n) => self.parse_message(req),
                _     => continue,
            }
        }
    }

    fn parse_message(&self, req: String) {
        let msg = Msg::from_str(&req);
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
    port: UnixStream,
    state_machine: HashMap<String, String>,
}

impl BaseNode {
    fn new<I: Iterator<Item=String>>(id: String, neighbors: I) -> BaseNode {
        BaseNode {
            id: NodeId::from(id.borrow()),
            current_term: 0,
            voted_for: None,
            log: vec![],
            commit_idx: 0,
            last_applied: 0,
            neighbors: neighbors.map(NodeId::from).collect(),
            port: UnixStream::connect(id).unwrap(),
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

