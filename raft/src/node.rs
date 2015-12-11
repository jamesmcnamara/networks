use std::borrow::Borrow;
use std::cell;
use std::cmp;
use std::collections::{HashMap, HashSet};
use std::convert::From;
use std::fmt;
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
            150 
        } else {
            300 + (rng.gen::<u32>() % 150u32)
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
                    ////println!("{} is voting for {}", self.base.id, candidate_id);
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
                       match_index: self.base.last_index()
                    }
                } else {
                    self.maybe_update_term(details.term);
                    self.node_type = NodeType::Follower;
                    self.base.voted_for = None;
                    self.base.leader = msg.base.leader;

                    if let Some(idx) = self.base.index_of(details.last_entry, details.last_entry_term) {
                        //println!("{} received a valid append entryi, len: {}", self.base.id, self.base.log.len());
                        self.base.log.truncate(idx);
                        if let Some(entries) = entries {
                            self.base.log.extend(entries);
                        }

                        self.maybe_commit_logs(leader_commit);

                        MsgType::AEResp {
                           term: self.base.current_term,
                           success: true,
                           match_index: self.base.last_index()
                        }
                    } else {
                        //println!("{} received a bad append entry: don't have {} with {}, log len: {}, term is", self.base.id, details.last_entry, details.last_entry_term, self.base.log.len());
                        MsgType::AEResp {
                           term: self.base.current_term,
                           success: false,
                           match_index: self.base.last_index()
                        }
                    }
                };

                self.send(&outgoing);
            },

            MsgType::AEResp { term, success, match_index } => {
                let retry_id = if let NodeType::Leader {ref mut next_indicies, ref mut match_indicies, ..} = self.node_type {
                    if success {
                        match_indicies.insert(msg.base.src, match_index);
                        next_indicies.insert(msg.base.src, match_index + 1);

                        None
                    } else {
                        if let Some(idx) = match_indicies.get(&msg.base.src) {
                            let new_idx =  cmp::max(*idx, 1) - 1;
                            next_indicies.insert(msg.base.src, new_idx);

                            Some((msg.base.src, new_idx))
                        } else {
                            None 
                        }
                    }
                } else {
                    None   
                };

                if let Some((id, new_idx)) = retry_id {
                    self.send_retry_append(id, new_idx);
                } else {
                    let commit_idx = self.base.commit_idx;
                    self.maybe_commit_logs(commit_idx);
                }
            },

            _ => unreachable!("unrecognized node message: {}", msg.msg.name())
        }
    }

    fn handle_client(&mut self, msg: Msg) {
        let mut outgoing = msg.clone();
        mem::swap(&mut outgoing.base.src, &mut outgoing.base.dst);
        outgoing.base.leader = self.base.leader;
        ////println!("{} got client message: {}", self.base.id, msg.to_json());
        match msg.msg {
            MsgType::Get(key) => {
                outgoing.msg = if let NodeType::Leader { .. } = self.node_type {
                    match self.base.state_machine.get(&key) {
                        Some(value) => MsgType::OK(value.clone()),
                        None        => {
                            ////println!("leader {} can't find key {}", self.base.id, key);
                            MsgType::Fail
                        }
                    }
                } else {
                    MsgType::Redirect
                };
                ////println!("{} response is {}", self.base.id, outgoing.to_json());
                self.send(&outgoing);
            },
            MsgType::Put(key, value) => {
                let append = if let NodeType::Leader {ref mut outstanding, ..} = self.node_type {
                    let entry = Entry { 
                        key: key.clone(), 
                        value: value.clone(), 
                        term: self.base.current_term
                    };
                    outstanding.insert(entry.clone(), outgoing);
                    
                    Some(entry)
                } else {
                    outgoing.msg = MsgType::Redirect;
                    self.send(&outgoing);
                    ////println!("{} response is {}", self.base.id, outgoing.to_json());

                    None
                };

                if let Some(entry) = append {
                    //println!("leader len is {}", self.base.log.len());
                    self.send_append_entries(entry.clone());
                    self.base.log.push(entry);
                }
            },
            MsgType::OK(_)
                | MsgType::Redirect
                | MsgType::Fail => panic!("got an external message"),
            _ => unreachable!("unrecognized client message: {}", msg.msg.name())
        }
    }

    fn maybe_update_term(&mut self, term: u64) {
        if term > self.base.current_term {
            //println!("new term: {}", term);
            self.base.current_term = term
        }
    }

    fn maybe_commit_logs(&mut self, leader_commit: u64) {
        let mut msgs = vec![];

        if let NodeType::Leader {ref mut match_indicies, ref mut outstanding, .. } = self.node_type {
            let mut matches = match_indicies.values().collect_vec();
            matches.sort();
            let majority_idx = matches.len() / 2;
            ////println!("about to look at index: {}", majority_idx);
            let committable = *(matches[majority_idx]);
            ////println!("look in index: {} and found {}", majority_idx, committable);

            if committable > leader_commit {
                //println!("about to loop and log len is {}, matches is {:?}, commitable: {}", self.base.log.len(), matches, committable);
                for entry in &self.base.log[leader_commit as usize .. committable as usize + 1] {
                    ////println!("inserting {:?}", entry);
                    self.base.state_machine.insert(entry.key.clone(), entry.value.clone());
                    if let Some(mut msg) = outstanding.remove(entry) {
                        msg.msg = MsgType::OK(entry.value.clone());
                        msgs.push(msg);
                    }
                }
                ////println!("success");
               self.base.commit_idx = majority_idx as u64;

               ////println!("outstanding is: {:?}", outstanding.values().collect_vec());
            }
        } else {
            if leader_commit > self.base.commit_idx {
                ////println!("about to loop");
                let range = self.base.commit_idx as usize 
                    .. cmp::min(self.base.last_index() as usize, leader_commit as usize);
                for entry in &self.base.log[range] {
                    ////println!("inserting {:?}", entry);
                    self.base.state_machine.insert(entry.key.clone(), entry.value.clone());
                }
                self.base.commit_idx = leader_commit;
            }
        }

        for msg in msgs {
            ////println!("{} response is {}", self.base.id, msg.to_json());
            self.send(&msg);
        }
    }

    fn grant_vote(&self, details: InternalMsg, candidate_id: NodeId) -> bool {
        details.term > self.base.current_term
            && details.last_entry >= self.base.last_index()
            && details.last_entry_term == self.base.log.last().map_or(0, |entry| entry.term)
            && self.base.voted_for.map_or(true, |candidate| candidate == candidate_id)
    }

    fn into_candidate(&mut self) {
        ////println!("{} is becoming a candidate", self.base.id);
        let mut votes = HashSet::new();
        votes.insert(self.base.id);
        mem::replace(&mut self.node_type, NodeType::Candidate(votes));
        self.base.current_term += 1;
        self.base.voted_for = Some(self.base.id);

        self.send_request_vote()
    }


    fn into_leader(&mut self) {
        ////println!("{} IS THE LEADER", self.base.id);
        self.base.leader = self.base.id;
        let mut match_indicies = HashMap::new();
        let mut next_indicies = HashMap::new();
        for node in &self.base.neighbors {
            match_indicies.insert(*node, 0);
            next_indicies.insert(*node, self.base.log.len() as u64);
        }

        self.node_type = NodeType::Leader {
            next_indicies: next_indicies, 
            match_indicies: match_indicies,
            outstanding: HashMap::new()
        };

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
        //println!("sending hearbeat");
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

    fn send_retry_append(&self, dst: NodeId, idx: u64) {
        let base = BaseMsg::new(self.base.id,
                                dst,
                                self.base.leader, 
                                "retry".to_owned());
        ////println!("looking up!");
        let details = InternalMsg::new(self.base.current_term, 
                                       idx, 
                                       self.base.get_term(idx));
        let retry = Msg {
            base: base,
            msg: MsgType::AppendEntries {
                details: details,
                leader_commit: self.base.commit_idx,
                entries: Some(self.base.log[idx as usize..self.get_chunk_index(idx as usize)].to_vec())
            }
        };
        ////println!("found");
        self.send(&retry);
    }
    
    fn get_chunk_index(&self, idx: usize) -> usize {
        cmp::min(idx + 35, cmp::max(self.base.log.len(), 1) - 1)
    }

    fn make_details(&self) -> InternalMsg {
       let last_entry_term = self.base.log
            .last()
            .map_or(0, |entry| entry.term);

       InternalMsg::new(self.base.current_term,
                        self.base.last_index(),
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
            neighbors: neighbors.map(NodeId::from).collect(),
            reader: reader,
            writer: cell::RefCell::new(writer),
            state_machine: HashMap::new(),
        }
    }

    fn get_term(&self, idx: u64) -> u64 {
        self.log.get(idx as usize).map_or(0, |entry| entry.term)
    }

    fn last_index(&self) -> u64 {
        cmp::max(self.log.len(), 1) as u64 - 1 
    }


    fn index_of(&self, index: u64, term: u64) -> Option<usize> {
        if index == 0 {
            Some(0)
        } else {
            self.log.get(index as usize)
                .and_then(|entry| if entry.term == term { 
                    Some(index as usize) 
                } else { 
                    None 
                })
        }
    }
}

enum NodeType {
    Follower,
    Candidate(HashSet<NodeId>),
    Leader {
        next_indicies: HashMap<NodeId, u64>,
        match_indicies: HashMap<NodeId, u64>,
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


impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&format!("[{}{}{}{}]", self.0[0] as char, 
                                  self.0[1] as char, 
                                  self.0[2] as char, 
                                  self.0[3] as char), 
                          f)
    }
}
