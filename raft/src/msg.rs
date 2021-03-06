use std::collections::BTreeMap;
use std::convert::From;

use rustc_serialize::json::{Json, Object, ToJson};

use super::node::NodeId;

macro_rules! get {
    ($obj:ident -> $key:expr; $parser:path) => {{
        let err = format!("Json parse failed for {}", $key);
        $parser($obj.as_object()
                .expect("get not provided an object")
                .get($key)
                .expect(&err))
            .expect(&err)
    }}
}

#[derive(Clone, PartialEq, Debug)]
pub struct Msg {
    pub base: BaseMsg,
    pub msg: MsgType,
}

impl Msg {
    pub fn new(base: BaseMsg, typ: MsgType) -> Msg {
        Msg {
            base: base,
            msg: typ,
        }
    }

    pub fn from_str(s: &str) -> Msg {
        let raw = Json::from_str(s)
            .ok()
            .expect("parsing json from str failed");
        let base = BaseMsg::from(&raw);
        let msg = MsgType::from(&raw);

        Msg { base: base, msg: msg }
    }
}

impl ToJson for Msg {
    fn to_json(&self) -> Json {
        let mut d = BTreeMap::new();
        self.base.fill(&mut d);
        self.msg.fill(&mut d);

        Json::Object(d)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct BaseMsg {
    pub src: NodeId,
    pub dst: NodeId,
    pub leader: NodeId,
    pub mid: String,
}

impl BaseMsg {
    pub fn new(src: NodeId, dst: NodeId, leader: NodeId, mid: String) -> BaseMsg {
        BaseMsg {
            src: src,
            dst: dst,
            leader: leader,
            mid: mid
        }
    }

    fn fill(&self, d: &mut Object) {
        d.add_json("src", self.src);
        d.add_json("dst", self.dst);
        d.add_json("leader", self.leader);
        d.add_json("MID", self.mid.to_owned());
    }
}

impl <'a>From<&'a Json> for BaseMsg {
    fn from(obj: &'a Json) -> BaseMsg {
        BaseMsg {
            leader: get!(obj -> "leader"; NodeId::as_node_id),
            src: get!(obj -> "src"; NodeId::as_node_id),
            dst: get!(obj -> "dst"; NodeId::as_node_id),
            mid: get!(obj -> "MID"; Json::as_string).to_owned()
        }
    }
}


#[derive(Clone, PartialEq, Debug)]
pub enum MsgType {
    Fail,
    Redirect,
    OK(String),
    Get(String),
    Put(String, String),
    AppendEntries {
        details: InternalMsg,
        leader_commit: u64,
        entries: Option<Vec<Entry>> },
    AEResp {
        term: u64,
        success: bool,
        match_index: u64,
        commit_idx: u64,
    },
    RequestVote {
        details: InternalMsg,
        candidate_id: NodeId,
    },
    RVResp(u64, bool),
}

impl MsgType {
    fn fill(&self, d: &mut Object) {
        d.add_json("type", self.name().to_owned());
        match *self {
            MsgType::Fail | MsgType::Redirect => return,
            MsgType::OK(ref value) => d.add_json("value", value.to_owned()),
            MsgType::Get(ref key) => d.add_json("key", key.to_owned()),
            MsgType::Put(ref key, ref val) => {
                d.add_json("key", key.to_owned());
                d.add_json("value", val.to_owned());
            },
            MsgType::AppendEntries {ref details, leader_commit, ref entries} => {
                details.fill(d);
                d.add_json("leader_commit", leader_commit);
                d.add_json("entries", entries.clone());
            },
            MsgType::AEResp {term, success, match_index, commit_idx} => {
                d.add_json("term", term);
                d.add_json("success", success);
                d.add_json("match_index", match_index);
                d.add_json("commit_idx", commit_idx);
            },
            MsgType::RequestVote {ref details, ref candidate_id} => {
                details.fill(d);
                d.add_json("candidate_id", candidate_id.clone());
            },
            MsgType::RVResp(term, vote) => {
                d.add_json("term", term);
                d.add_json("vote", vote);
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match *self {
            MsgType::Fail => "fail",
            MsgType::Redirect => "redirect",
            MsgType::OK(_) => "ok",
            MsgType::Get(_) => "get",
            MsgType::Put(..) => "put",
            MsgType::AppendEntries{ .. } => "append_entries",
            MsgType::AEResp { .. } => "ae_resp",
            MsgType::RequestVote{ .. } => "request_vote",
            MsgType::RVResp(..) => "rv_resp",
        }
    }

    fn parse_append_entries(json: &Json) -> MsgType {
        let int_msg = InternalMsg::from(json);
        let obj = json.as_object().expect("parse_append_entries expects a JSON object");
        let entries = obj
            .get("entries")
            .map_or(None, |entries| {
                if entries.is_array() {
                    Some(entries.as_array()
                        .expect("entries must be an array")
                        .iter()
                        .map(Entry::from)
                        .collect())
                } else {
                    None
                }
            });
        MsgType::AppendEntries {
            details: int_msg,
            leader_commit: get!(json -> "leader_commit"; Json::as_u64),
            entries: entries
        }
    }

    fn parse_ae_resp(json: &Json) -> MsgType {
        MsgType::AEResp {
            term: get!(json -> "term"; Json::as_u64),
            success: get!(json -> "success"; Json::as_boolean),
            match_index: get!(json -> "match_index"; Json::as_u64),
            commit_idx: get!(json -> "commit_idx"; Json::as_u64),
        }
    }

    fn parse_request_vote(obj: &Json) -> MsgType {
        let int_msg = InternalMsg::from(obj);
        MsgType::RequestVote{
            details: int_msg,
            candidate_id: get!(obj -> "candidate_id"; NodeId::as_node_id)
        }
    }
}

impl <'a>From<&'a Json> for MsgType {
    fn from(obj: &'a Json) -> MsgType {
        match get!(obj -> "type"; Json::as_string) {
            "fail" => MsgType::Fail,
            "redirect" => MsgType::Redirect,
            "ok" => MsgType::OK(get!(obj -> "value"; Json::as_string).to_owned()),
            "get" => MsgType::Get(get!(obj -> "key"; Json::as_string).to_owned()),
            "put" => MsgType::Put(get!(obj -> "key"; Json::as_string).to_owned(),
                                  get!(obj -> "value";Json::as_string).to_owned()),
            "append_entries" => MsgType::parse_append_entries(obj),
            "ae_resp" => MsgType::parse_ae_resp(obj),
            "request_vote" => MsgType::parse_request_vote(obj),
            "rv_resp" => MsgType::RVResp(get!(obj -> "term"; Json::as_u64),
                                         get!(obj -> "vote"; Json::as_boolean)),
            _              => unreachable!("unknown message type"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct InternalMsg {
    pub term: u64,
    pub last_entry: u64,
    pub last_entry_term: u64,
}

impl InternalMsg {
    pub fn new(term: u64, last_entry: u64, last_entry_term: u64) -> InternalMsg {
        InternalMsg {
            term: term,
            last_entry: last_entry,
            last_entry_term: last_entry_term,
        }
    }

    fn fill(&self, d: &mut Object) {
        d.add_json("term", self.term);
        d.add_json("last_entry", self.last_entry);
        d.add_json("last_entry_term", self.last_entry_term);
    }
}

impl <'a>From<&'a Json> for InternalMsg {
    fn from(obj: &'a Json) -> InternalMsg {
        InternalMsg {
            term: get!(obj -> "term"; Json::as_u64),
            last_entry: get!(obj -> "last_entry"; Json::as_u64),
            last_entry_term: get!(obj -> "last_entry_term"; Json::as_u64),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub struct Entry {
    pub key: String,
    pub value: String,
    pub term: u64,
}

impl Entry {
    pub fn new(key: &str, val: &str, term: u64) -> Entry {
        Entry {
            key: key.to_owned(),
            value: val.to_owned(),
            term: term,
        }
    }
}

impl <'a>From<&'a Json> for Entry {
    fn from(entry: &'a Json) -> Entry {
        Entry {
            key: get!(entry -> "key"; Json::as_string).to_owned(),
            value: get!(entry -> "value"; Json::as_string).to_owned(),
            term: get!(entry -> "term"; Json::as_u64),
        }
    }
}

impl ToJson for Entry {
    fn to_json(&self) -> Json {
        let mut d = BTreeMap::new();
        d.add_json("key", self.key.to_owned());
        d.add_json("value", self.value.to_owned());
        d.add_json("term", self.term);
        Json::Object(d)
    }
}

pub trait AddJson {
    fn add_json<T: ToJson>(&mut self, key: &'static str, val: T);
}

impl AddJson for Object {
    fn add_json<T: ToJson>(&mut self, key: &'static str, val: T) {
        self.insert(key.to_owned(), val.to_json());
    }
}

#[allow(dead_code)]
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
    assert_eq!(msg.to_json().to_string(), s("{\"MID\":\"BABADOOK\",\"dst\":\"001E\",\"key\":\"hello\",\"leader\":\"AA43\",\"src\":\"13AE\",\"type\":\"get\"}"));
}

#[test]
fn test_internal_msg_serialize() {
    let details = InternalMsg{ term: 4, last_entry_term: 3, last_entry: 213 };
    let append = MsgType::AppendEntries {
        details: details,
        leader_commit: 5,
        entries: Some(vec![Entry::new("x", "13", 1), Entry::new("y","27", 1)]),
    };

    let base = BaseMsg {
        src: NodeId(['1' as u8, '3' as u8, 'A' as u8, 'E' as u8]),
        dst: NodeId(['0' as u8, '0' as u8, '1' as u8, 'E' as u8]),
        leader: NodeId(['A' as u8, 'A' as u8, '4' as u8, '3' as u8]),
        mid: s("BABADOOK")
    };
    let msg = Msg { base: base, msg: append};
    let d = msg.to_json().to_string();
    println!("");
    println!("{}", d);
    let e = s("{\"MID\":\"BABADOOK\",\"dst\":\"001E\",\"entries\":[{\"key\":\"x\",\"term\":1,\"value\":\"13\"},{\"key\":\"y\",\"term\":1\"value\":\"27\"}],\"last_entry\":213,\"last_entry_term\":3,\"leader\":\"AA43\",\"leader_commit\":5,\"src\":\"13AE\",\"term\":4,\"type\":\"append_entries\"}");
    println!("{}", e);
    assert_eq!(msg.to_json().to_string(), s("{\"MID\":\"BABADOOK\",\"dst\":\"001E\",\"entries\":[{\"key\":\"x\",\"term\":1,\"value\":\"13\"},{\"key\":\"y\",\"term\":1,\"value\":\"27\"}],\"last_entry\":213,\"last_entry_term\":3,\"leader\":\"AA43\",\"leader_commit\":5,\"src\":\"13AE\",\"term\":4,\"type\":\"append_entries\"}"));
}

#[test]
fn test_msg_deserialize() {
    let msg = "{\"dst\":\"001E\",\"leader\":\"AA43\",\"MID\":\"BABADOOK\",\"src\":\"13AE\",\"type\":\"ok\",\"value\":\"blah\"}";
    let base = BaseMsg {
        src: NodeId(['1' as u8, '3' as u8, 'A' as u8, 'E' as u8]),
        dst: NodeId(['0' as u8, '0' as u8, '1' as u8, 'E' as u8]),
        leader: NodeId(['A' as u8, 'A' as u8, '4' as u8, '3' as u8]),
        mid: s("BABADOOK")
    };
    let msg_type = MsgType::OK(s("blah"));
    assert_eq!(Msg {base: base, msg: msg_type}, Msg::from_str(msg));
}

#[test]
fn test_int_msg_deserialize() {
    let details = InternalMsg{ term: 4, last_entry_term: 3, last_entry: 213 };
    let append = MsgType::AppendEntries {
        leader_commit: 5,
        details: details,
        entries: Some(vec![Entry::new("x", "13", 1), Entry::new("y","27", 1)]),
    };

    let base = BaseMsg {
        src: NodeId(['1' as u8, '3' as u8, 'A' as u8, 'E' as u8]),
        dst: NodeId(['0' as u8, '0' as u8, '1' as u8, 'E' as u8]),
        leader: NodeId(['A' as u8, 'A' as u8, '4' as u8, '3' as u8]),
        mid: s("BABADOOK")
    };
    let msg = Msg { base: base, msg: append};
    assert_eq!(msg, Msg::from_str("{\"MID\":\"BABADOOK\",\"dst\":\"001E\",\"entries\":[{\"key\":\"x\",\"term\":1,\"value\":\"13\"},{\"key\":\"y\",\"term\":1,\"value\":\"27\"}],\"last_entry\":213,\"last_entry_term\":3,\"leader\":\"AA43\",\"leader_commit\":5,\"src\":\"13AE\",\"term\":4,\"type\":\"append_entries\"}"));
}
