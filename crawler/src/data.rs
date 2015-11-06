use std::collections::{HashSet, VecDeque};
use std::sync::mpsc;

pub enum UrlMsg {
    Add(Vec<String>),
    SetCookie(String),
    Request(usize),
    Flag(String),
}

pub type UrlResponse = Option<(String, Option<String>)>;

struct UrlStore {
    flags: usize,
    cookie: Option<String>,
    visited: HashSet<String>,
    frontier: VecDeque<String>,
    req_chan: mpsc::Receiver<UrlMsg>,
    resp_chans: Vec<mpsc::Sender<UrlResponse>>
}

impl UrlStore {
    fn new(first_url: String, 
          rx: mpsc::Receiver<UrlMsg>, 
          resp_chans: Vec<mpsc::Sender<UrlResponse>>)  -> UrlStore {
        let mut frontier = VecDeque::new();
        frontier.push_back(first_url);
        UrlStore {
           flags: 0,
           cookie: None,
           visited: HashSet::new(),
           frontier: frontier,
           req_chan: rx,
           resp_chans: resp_chans,
        }
    }

    fn listen(&mut self) {
        while self.flags < 5 {
            match self.req_chan.recv() {
                Ok(UrlMsg::Add(urls))         =>  self.add(urls),
                Ok(UrlMsg::SetCookie(cookie)) => {self.cookie = Some(cookie)},
                Ok(UrlMsg::Flag(flag))        => {println!("{}", flag); self.flags += 1},
                Ok(UrlMsg::Request(id))       =>  self.send_url_to(id),
                _                             =>  continue,
            }
        }
    }

    fn add(&mut self, urls: Vec<String>) {
        for url in urls {
            self.frontier.push_back(url);
        }   
    }

    fn send_to(&self, id: usize, msg: UrlResponse) {
        self.resp_chans[id].send(msg)
            .ok()
            .expect(&format!("Crawler {} hung up", id))
    }

    fn send_url_to(&mut self, id: usize) {
        if let Some(url) = self.frontier.pop_front() {
            if self.visited.contains(&url) {
                self.send_url_to(id)
            } else {
                self.send_to(id, Some((url, self.cookie.clone())))
            }
        } else {
            self.send_to(id, None)
        }
    }
}
