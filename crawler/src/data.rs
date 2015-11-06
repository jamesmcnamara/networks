use std::collections::HashSet;
use std::sync::mpsc;

pub enum UrlMsg {
    Add(Vec<String>),
    Request(usize),
}

struct UrlStore {
   cookie: Option<String>,
   visited: HashSet<String>,
   frontier: Vec<String>,
   req_chan: mpsc::Receiver<UrlMsg>,
   resp_chans: Vec<mpsc::Sender<(String, Option<String>)>>
}
