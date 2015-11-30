use std::collections::BTreeMap;

use rustc_serialize::json::{Json, ToJson};

use super::node::NodeId;

pub trait AddJson {
    fn add_json<T: ToJson>(&mut self, key: &'static str, val: T);
}

impl AddJson for BTreeMap<String, Json> {
    fn add_json<T: ToJson>(&mut self, key: &'static str, val: T) {
        self.insert(key.to_owned(), val.to_json());
    }
}

struct BaseMsg {
    src: NodeId,
    dst: NodeId,
    leader: NodeId,
    mid: String,
}

impl BaseMsg {
    fn fill(&self, d: &mut BTreeMap<String, Json>) {
        d.insert(s("src"), self.src.to_json());
        d.insert(s("dst"), self.dst.to_json());
        d.insert(s("leader"), self.leader.to_json());
        d.insert(s("mid"), self.mid.to_json());
    }
}

struct InternalMsg {
    term: usize, 
    last_entry: usize, 
    last_term: usize, 
}

impl InternalMsg {
    fn fill(&self, d: &mut BTreeMap<String, Json>) {
        d.add_json("term", self.term);
        d.add_json("last_entry", self.last_entry);
        d.add_json("last_term", self.last_term);
    }
}

enum MsgType {
    OK,
    Fail,
    Redirect,
    Get(String),
    Put(String, String),
    AppendEntries { 
        details: InternalMsg,
        entries: Option<Vec<String>> },
    RequestVote {
        details: InternalMsg,
        candidate_id: usize,
    },
}

impl MsgType {
    fn fill(&self, d: &mut BTreeMap<String, Json>) {
        d.add_json("type", self.name().to_owned());
        match *self {
            MsgType::OK | MsgType::Fail | MsgType::Redirect => return,
            MsgType::Get(ref key) => d.add_json("key", key.to_owned()),
            MsgType::Put(ref key, ref val) => {
                d.add_json("key", key.to_owned());
                d.add_json("value", val.to_owned());
            },
            MsgType::AppendEntries {ref details, ref entries} => {
                details.fill(d);
                d.add_json("entries", entries.clone());
            }
            MsgType::RequestVote {ref details, ref candidate_id} => {
                details.fill(d);
                d.add_json("candidate_id", candidate_id.clone());
            }
        }
    }

    fn name(&self) -> &'static str {
        match *self {
            MsgType::OK => "ok",
            MsgType::Fail => "fail",
            MsgType::Redirect => "redirect",
            MsgType::Get(_) => "get",
            MsgType::Put(..) => "put",
            MsgType::AppendEntries{ .. } => "append_entries",
            MsgType::RequestVote{ .. } => "request_vote",
        }
    }
}

pub struct Msg {
    base: BaseMsg,
    msg: MsgType,
}

impl ToJson for Msg {
    fn to_json(&self) -> Json {
        let mut d = BTreeMap::new();
        self.base.fill(&mut d);
        self.msg.fill(&mut d);
        
        Json::Object(d)
    }
}

fn s(string: &str) -> String {
    string.to_owned()
}

#[test]
fn test_msg_serialize() {
    let get = MsgType::Get(s("hello"));
    let base = BaseMsg {
        src: NodeId(['1' as u8, '3' as u8, 'A' as u8, 'E' as u8]), 
        dst: NodeId(['0' as u8, '0' as u8, '1' as u8, 'E' as u8]),
        leader: NodeId(['A' as u8, 'A' as u8, '4' as u8, '3' as u8]),
        mid: s("BABADOOK")
    };
    let msg = Msg { base: base, msg: get };
    assert_eq!(msg.to_json().to_string(), s("{\"dst\":\"001E\",\"key\":\"hello\",\"leader\":\"AA43\",\"mid\":\"BABADOOK\",\"src\":\"13AE\",\"type\":\"get\"}"));
}

#[test]
fn test_internal_msg_serialize() {
    let details = InternalMsg{ term: 4, last_term: 3, last_entry: 213 };
    let append = MsgType::AppendEntries {
        details: details, 
        entries: Some(vec![s("x: 13"), s("y: 27")]),
    };

    let base = BaseMsg {
        src: NodeId(['1' as u8, '3' as u8, 'A' as u8, 'E' as u8]), 
        dst: NodeId(['0' as u8, '0' as u8, '1' as u8, 'E' as u8]),
        leader: NodeId(['A' as u8, 'A' as u8, '4' as u8, '3' as u8]),
        mid: s("BABADOOK")
    };
    let msg = Msg { base: base, msg: append};
    assert_eq!(msg.to_json().to_string(), s("{\"dst\":\"001E\",\"entries\":[\"x: 13\",\"y: 27\"],\"last_entry\":213,\"last_term\":3,\"leader\":\"AA43\",\"mid\":\"BABADOOK\",\"src\":\"13AE\",\"term\":4,\"type\":\"append_entries\"}"));
}
