use std::borrow::Borrow;
use std::cell;
use std::collections::{HashMap, HashSet};
use std::convert::From;
use std::io::Write;
use std::mem;
use std::str::from_utf8;
use std::sync::mpsc;

use rand;
use rand::Rng;
use rustc_serialize::json::{encode, Json, ToJson};
use schedule_recv::oneshot_ms;
use unix_socket::UnixStream;

use super::msg::{BaseMsg, Entry, InternalMsg, Msg, MsgType};

enum MsgClass {
    Timeout,
    Client(Msg),
    Node(Msg),
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
        let mut rng = rand::thread_rng();
        let mut timer = self.reset_timer(&mut rng);
        loop {
            match {
                let chan = &self.base.reader;
                select! {
                    msg = chan.recv() => self.classify(msg.unwrap()),
                    _   = timer.recv() => MsgClass::Timeout
                }
            } {
                MsgClass::Timeout => {
                    if let NodeType::Leader{ .. } = self.node_type {
                        timer = self.reset_timer(&mut rng);
                        self.send_heartbeat();
                    } else {
                        self.into_candidate();
                        timer = self.reset_timer(&mut rng);
                    }
                },
                MsgClass::Node(msg) => {
                    self.handle_node(msg);
                    timer = self.reset_timer(&mut rng);
                },
                MsgClass::Client(msg) => {
                    self.handle_client(msg);
                },
            }
        }
    }


    fn reset_timer(&self, rng: &mut rand::ThreadRng) -> mpsc::Receiver<()> {
        oneshot_ms(if let NodeType::Leader{..} = self.node_type {
            100
        } else {
            150 + (rng.gen::<u32>() % 150u32)
        })
    }

    fn classify(&self, msg: Msg) -> MsgClass {
        match msg.msg {
            MsgType::Get(_) 
                | MsgType::Put(..)
                | MsgType::OK(_)
                | MsgType::Redirect
                | MsgType::Fail => MsgClass::Client(msg),
            _  => MsgClass::Node(msg),
        }
    }

    fn handle_node(&mut self, msg: Msg) {
        let mut outgoing = msg.clone();
        mem::swap(&mut outgoing.base.src, &mut outgoing.base.dst);
        outgoing.base.leader = self.base.leader;
        match msg.msg {
            MsgType::RequestVote { details, candidate_id } => {
                let ulysses_grant_vote = self.grant_vote(details, candidate_id);
                if ulysses_grant_vote {
                    //println!("{:?} is voting for {:?}", self.base.id, candidate_id);
                    self.base.voted_for = Some(candidate_id);
                }
                outgoing.msg = MsgType::RVResp(self.base.current_term, ulysses_grant_vote);
                self.send(&outgoing);
            },

            MsgType::RVResp(their_term, their_vote) => {
                let leader = if let NodeType::Candidate(ref mut votes) = self.node_type {
                    if their_vote {
                       votes.insert(msg.base.src);
                    }
                    votes.len() > (self.base.neighbors.len() / 2)
                } else {
                    false
                };

                self.maybe_update_term(their_term);

                if leader {
                    self.into_leader()
                }
            },

            MsgType::AppendEntries {details, leader_commit, entries} => {
                outgoing.msg = if details.term < self.base.current_term {
                    MsgType::AEResp {
                       term: self.base.current_term,
                       success: false,
                       conflicting_term: None,
                       first_entry_of_term: None,
                    }
                } else {
                    self.maybe_update_term(details.term);
                    self.node_type = NodeType::Follower;
                    self.base.voted_for = None;
                    self.base.leader = msg.base.leader;

                    if self.logs_match(details) {
                        MsgType::AEResp {
                           term: self.base.current_term,
                           success: true,
                           conflicting_term: None,
                           first_entry_of_term: None,
                        }
                    } else {
                        MsgType::AEResp {
                           term: self.base.current_term,
                           success: false,
                           conflicting_term: None,
                           first_entry_of_term: None,
                        }
                    }
                };

                self.send(&outgoing);
            },

            MsgType::AEResp { term, success, conflicting_term, first_entry_of_term } => {
                
            },

            _ => unreachable!("unrecognized node message: {}", msg.msg.name())
        }
    }

    fn handle_client(&mut self, mut msg: Msg) {
        let mut outgoing = msg.clone();
        mem::swap(&mut outgoing.base.src, &mut outgoing.base.dst);
        outgoing.base.leader = self.base.leader;
        //println!("{:?} got client message: {}", self.base.id, msg.to_json());
        match msg.msg {
            MsgType::Get(key) => {
                outgoing.msg = if let NodeType::Leader { .. } = self.node_type {
                    match self.base.state_machine.get(&key) {
                        Some(value) => MsgType::OK(value.clone()),
                        None        => MsgType::Fail
                    }
                } else {
                    MsgType::Redirect
                };
                //println!("{:?} response is {}", self.base.id, outgoing.to_json());
                self.send(&outgoing);
            },
            MsgType::Put(key, value) => {
                outgoing.msg = if let NodeType::Leader { .. } = self.node_type {
                    self.base.log.push(Entry { 
                        key: key.clone(), 
                        value: value.clone(), 
                        term: self.base.current_term
                    });
                    self.base.state_machine.insert(key, value.clone());
                    MsgType::OK(value)
                } else {
                    MsgType::Redirect
                };
                //println!("{:?} response is {}", self.base.id, outgoing.to_json());
                self.send(&outgoing);
            },
            MsgType::OK(_)
                | MsgType::Redirect
                | MsgType::Fail => (),
            _ => unreachable!("unrecognized client message: {}", msg.msg.name())
        }
    }

    fn maybe_update_term(&mut self, term: u64) {
        if term > self.base.current_term {
            self.base.current_term = term
        }
    }

    fn grant_vote(&self, details: InternalMsg, candidate_id: NodeId) -> bool {
        if details.term <= self.base.current_term {
            //println!("term outdated. my term is {} but theres is {}", self.base.current_term, details.term);
        }
        if details.last_entry < self.base.log.len() as u64 {
            //println!("log outdated. my log is {} but theres is {}", self.base.log.len(), details.last_entry);
        }
        if details.last_entry_term != self.base.log.last().map_or(0, |entry| entry.term) {
            //println!("log term outdated. my log term is {} but theres is {}", self.base.log.last().map_or(0, |entry| entry.term), details.last_entry_term);
        }
        if self.base.voted_for.map_or(false, |candidate| candidate != candidate_id) {
            //println!("im dumb!");
        }

        details.term > self.base.current_term
            && self.logs_match(details)
            && self.base.voted_for.map_or(true, |candidate| candidate == candidate_id)
    }

    fn logs_match(&self, details: InternalMsg) -> bool {
        details.last_entry >= self.base.log.len() as u64
            && details.last_entry_term == self.base.log.last().map_or(0, |entry| entry.term)

    }

    fn into_candidate(&mut self) {
        //println!("{:?} is becoming a candidate", self.base.id);
        let mut votes = HashSet::new();
        votes.insert(self.base.id);
        mem::replace(&mut self.node_type, NodeType::Candidate(votes));
        self.base.current_term += 1;
        self.base.voted_for = Some(self.base.id);
        for node in &self.base.neighbors {
            self.send_request_vote(*node);
        }
    }

    fn send_request_vote(&self, to: NodeId) {
        let details = self.make_details();

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


    fn into_leader(&mut self) {
        //println!("{:?} IS THE LEADER", self.base.id);
        self.base.leader = self.base.id;
        let mut match_index = vec![];
        let mut next_index = vec![];
        for _ in 0..self.base.neighbors.len() {
            match_index.push(self.base.log.len() + 1);
            next_index.push(0);
        }

        mem::replace(&mut self.node_type, NodeType::Leader {
            next_index: next_index, 
            match_index: match_index});

        self.send_heartbeat();
    }

    fn send_heartbeat(&self) {
        if let NodeType::Leader {..} = self.node_type {
            let details = self.make_details();
            for node in &self.base.neighbors {
                let base = BaseMsg::new(self.base.id,
                                        *node,
                                        NodeId::broadcast(),
                                        "append".to_owned());
                let heartbeat = Msg {
                    base: base,
                    msg: MsgType::AppendEntries {
                       details: details.clone(),
                       leader_commit: self.base.commit_idx,
                       entries: None,
                    },
                };
               
                self.send(&heartbeat);
            }
        }
    }

    fn make_details(&self) -> InternalMsg {
       let last_entry_term = self.base.log
            .last()
            .map_or(0, |entry| entry.term);

       InternalMsg::new(self.base.current_term,
                        self.base.log.len() as u64,
                        last_entry_term)


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
    leader: NodeId,
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
            leader: NodeId::broadcast(),
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
    Candidate(HashSet<NodeId>),
    Leader {
        next_index: Vec<usize>,
        match_index: Vec<usize>,
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
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

