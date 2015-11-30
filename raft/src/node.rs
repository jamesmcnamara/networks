use std::str::from_utf8;

use unix_socket::UnixStream;
use rustc_serialize::json::{Json, ToJson};

pub struct NodeId(pub [u8; 4]);

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
