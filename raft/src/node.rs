use std::borrow::Borrow;
use std::cell;
use std::collections::{HashMap, HashSet};
use std::convert::From;
use std::io::Write;
use std::mem;
use std::str::from_utf8;
use std::sync::mpsc;

use itertools::Itertools;
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
                    msg = chan.recv() => match msg {
                        Ok(msg) => self.classify(msg),
                        Err(_)  => return,
                    },
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
                       entry: self.base.log.len() as u64,
                    }
                } else {
                    self.maybe_update_term(details.term);
                    self.node_type = NodeType::Follower;
                    self.base.voted_for = None;
                    self.base.leader = msg.base.leader;

                    if self.logs_match(details) {
                        if let Some(entries) = entries {
                            self.base.log.extend(entries);
                        }

                        self.maybe_commit_logs(leader_commit);

                        MsgType::AEResp {
                           term: self.base.current_term,
                           success: true,
                           conflicting_term: None,
                           entry: self.base.log.len() as u64,
                        }
                    } else {
                        MsgType::AEResp {
                           term: self.base.current_term,
                           success: false,
                           conflicting_term: None,
                           entry: self.first_entry_of_term(),
                        }
                    }
                };

                self.send(&outgoing);
            },

            MsgType::AEResp { term, success, conflicting_term, entry } => {
                if let NodeType::Leader {ref mut next_index, ref mut match_index, ..} = self.node_type {
                    if success {
                        match_index.insert(msg.base.src, entry);
                        next_index.insert(msg.base.src, entry + 1);
                        self.maybe_commit_logs(self.base.commit_idx);
                    } else {
                        next_index.insert(msg.base.src, entry - 1);
                    }

                }

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
                if let NodeType::Leader {ref mut outstanding, ..} = self.node_type {
                    let entry = Entry { 
                        key: key.clone(), 
                        value: value.clone(), 
                        term: self.base.current_term
                    };
                    self.base.log.push(entry.clone());
                    outstanding.insert(entry.clone(), outgoing);
                    self.send_append_entries(entry);
                } else {
                    outgoing.msg = MsgType::Redirect;
                    self.send(&outgoing);
                }
                //println!("{:?} response is {}", self.base.id, outgoing.to_json());
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

    fn maybe_commit_logs(&mut self, leader_commit: u64) {
        if let NodeType::Leader {ref mut match_index, ref mut outstanding, .. } = self.node_type {
            let matches = match_index.values().collect_vec();
            matches.sort();
            let majority_idx = matches.len() / 2 - 1;
            let committable = *(matches[majority_idx]);

            if committable > leader_commit {
                for entry in &self.base.log[leader_commit as usize .. committable as usize + 1] {
                    self.commit(entry.clone());
                    outstanding.remove(entry).map(|mut msg| {
                        msg.msg = MsgType::OK(entry.value.clone());
                        self.send(&msg);
                    });
                }
               self.base.commit_idx = majority_idx as u64;
            }
        } else {
            if leader_commit > self.base.commit_idx {
                for entry in &self.base.log[self.base.commit_idx as usize .. leader_commit as usize + 1] {
                    self.commit(entry.clone());
                }
                self.base.commit_idx = leader_commit;
            }
        }
    }

    fn commit(&mut self, entry: Entry) {
        self.base.state_machine.insert(entry.key, entry.value);
    }

    fn grant_vote(&self, details: InternalMsg, candidate_id: NodeId) -> bool {
        // if details.term <= self.base.current_term {
        //     //println!("term outdated. my term is {} but theres is {}", self.base.current_term, details.term);
        // }
        // if details.last_entry < self.base.log.len() as u64 {
        //     //println!("log outdated. my log is {} but theres is {}", self.base.log.len(), details.last_entry);
        // }
        // if details.last_entry_term != self.base.log.last().map_or(0, |entry| entry.term) {
        //     //println!("log term outdated. my log term is {} but theres is {}", self.base.log.last().map_or(0, |entry| entry.term), details.last_entry_term);
        // }
        // if self.base.voted_for.map_or(false, |candidate| candidate != candidate_id) {
        //     //println!("im dumb!");
        // }

        details.term > self.base.current_term
            && self.logs_match(details)
            && self.base.voted_for.map_or(true, |candidate| candidate == candidate_id)
    }

    fn logs_match(&self, details: InternalMsg) -> bool {
        details.last_entry >= self.base.log.len() as u64
            && details.last_entry_term == self.base.log.last().map_or(0, |entry| entry.term)

    }

    fn first_entry_of_term(&self) -> u64 {
        0
    }

    fn into_candidate(&mut self) {
        //println!("{:?} is becoming a candidate", self.base.id);
        let mut votes = HashSet::new();
        votes.insert(self.base.id);
        mem::replace(&mut self.node_type, NodeType::Candidate(votes));
        self.base.current_term += 1;
        self.base.voted_for = Some(self.base.id);

        self.send_request_vote()
    }


    fn into_leader(&mut self) {
        //println!("{:?} IS THE LEADER", self.base.id);
        self.base.leader = self.base.id;
        let mut match_index = HashMap::new();
        let mut next_index = HashMap::new();
        for node in &self.base.neighbors {
            match_index.insert(*node, self.base.log.len() as u64 + 1);
            next_index.insert(*node, 0);
        }

        mem::replace(&mut self.node_type, NodeType::Leader {
            next_index: next_index, 
            match_index: match_index,
            outstanding: HashMap::new()
        });

        self.send_heartbeat();
    }

    fn send_request_vote(&self) {
        let details = self.make_details();
        for to in &self.base.neighbors {
            let base = BaseMsg::new(self.base.id,
                                    *to,
                                    self.base.leader, 
                                    "rv".to_owned());
            let rv = Msg {
                base: base,
                msg: MsgType::RequestVote {
                   details: details.clone(),
                   candidate_id: self.base.id,
                },
            };

            self.send(&rv)
        }
    }


    fn send_heartbeat(&self) {
        if let NodeType::Leader {..} = self.node_type {
            let details = self.make_details();
            for node in &self.base.neighbors {
                let base = BaseMsg::new(self.base.id,
                                        *node,
                                        self.base.leader, 
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

    fn send_append_entries(&self, entry: Entry) {
        if let NodeType::Leader {..} = self.node_type {
            let details = self.make_details();
            for node in &self.base.neighbors {
                let base = BaseMsg::new(self.base.id,
                                        *node,
                                        self.base.leader, 
                                        "append".to_owned());
                let append = Msg {
                    base: base,
                    msg: MsgType::AppendEntries {
                       details: details.clone(),
                       leader_commit: self.base.commit_idx,
                       entries: Some(vec![entry.clone()]),
                    },
                };
               
                self.send(&append);
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
        drop((*self.base.writer.borrow_mut())
            .write_all((encode(&msg.to_json()).unwrap() + "\n").as_bytes()))
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
        next_index: HashMap<NodeId, u64>,
        match_index: HashMap<NodeId, u64>,
        outstanding: HashMap<Entry, Msg>,
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
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

