use std::collections::HashMap;
use std::str::from_utf8;

use unix_socket::UnixStream;
use rustc_serialize::json::{Json, ToJson};

#[derive(PartialEq, Debug)]
pub struct NodeId(pub [u8; 4]);

impl NodeId {
    pub fn as_node_id(json: &Json) -> Option<NodeId> {
        match json.as_string().expect("node id must be string").as_bytes() {
            [a, b, c, d] => Some(NodeId([a as u8, b as u8, c as u8, d as u8])),
            _            => None,
        }
    }
}

impl ToJson for NodeId {
    fn to_json(&self) -> Json {
        from_utf8(&self.0).unwrap().to_json()
    }
}

struct BaseNode {
    current_term: usize,
    voted_for: Option<NodeId>,
    log: Vec<String>,
    commit_idx: usize,
    last_applied: usize,
    neighbors: Vec<NodeId>,
    port: UnixStream,
    state_machine: HashMap<String, String>,
}

enum NodeType {
    Follower,
    Candidate,
    Leader {
        next_index: Vec<usize>,
        match_index: Vec<usize>,
    }
}

pub struct Node {
    base: BaseNode,
    node_type: NodeType,
}
