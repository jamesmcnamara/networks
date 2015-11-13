use std::collections::{HashSet, VecDeque};
use std::sync::mpsc;

// pub struct Cookie(HashMap<String, String>);
// 
// impl Cookie {
//     fn new() -> Cookie {
//         Cookie(HashMap::new())
//     }
// 
//     fn add(&mut self, cookie: String) {
//        let (key, value) = match cookie.find("=") {
//            Some(idx) => (cookie[..idx], cookie[idx..]),
//            None      => panic!("malformed cookie: {}", cookie),
//        };
//        self.0.insert(key.to_string(), value.to_string());
//     }
// 
//     fn get(&self, cookie: String) -> Option<String> {
//         self.0.get(cookie)
//     }
// }


pub enum UrlReq {
    Add(Vec<String>),
    SetCookie(String),
    GetCookie(usize),
    Request(usize),
    Flag(String),
}

pub enum UrlResp {
    Cookie(Option<String>),
    Url(String),
    EmptyQueue
}

pub type RespChans = Vec<mpsc::Sender<UrlResp>>;

pub struct UrlStore {
    flags: usize,
    cookie: Option<String>,
    visited: HashSet<String>,
    frontier: VecDeque<String>,
    req_chan: mpsc::Receiver<UrlReq>,
    resp_chans: RespChans,
}

impl UrlStore {
    pub fn new(rx: mpsc::Receiver<UrlReq>, resp_chans: RespChans) -> UrlStore {
        UrlStore {
           flags: 0,
           cookie: None,
           visited: HashSet::new(),
           frontier: VecDeque::new(),
           req_chan: rx,
           resp_chans: resp_chans,
        }
    }

    pub fn listen(mut self) {
        while self.flags < 5 {
            match self.req_chan.recv() {
                Ok(UrlReq::Add(urls))         =>  self.add(urls),
                Ok(UrlReq::SetCookie(cookie)) =>  self.add_cookie(cookie),
                Ok(UrlReq::GetCookie(id))     =>  self.send_cookie(id),
                Ok(UrlReq::Flag(flag))        =>  self.handle_flag(flag),
                Ok(UrlReq::Request(id))       =>  self.send_url_to(id),
                _                             =>  continue,
            }
        }
    }

    pub fn add_crawler(&mut self, resp_chan: mpsc::Sender<UrlResp>) {
        self.resp_chans.push(resp_chan);
    }

    fn add(&mut self, urls: Vec<String>) {
        for url in urls {
            self.frontier.push_back(url);
        }
    }
    
    pub fn add_crawler(&mut self, resp: mpsc::Sender<UrlResp>) {
        self.resp_chans.push(resp);
    }

    fn add_cookie(&mut self, cookie: String) {
        if let Some(ref mut old_cookie) = self.cookie {
            old_cookie.push_str(&cookie);
        } else {
            self.cookie = Some(cookie);
        }
    }
    
    fn send_cookie(&self, id: usize) {
        self.send_to(id, UrlResp::Cookie(self.cookie.clone()));
    }

    fn handle_flag(&mut self, flag: String) {
        println!("{}", flag);
        self.flags += 1;
    }

    fn send_to(&self, id: usize, msg: UrlResp) {
        self.resp_chans[id].send(msg)
            .ok()
            .expect(&format!("Crawler {} hung up", id))
    }

    fn get_fresh_url(&mut self) -> Option<String> {
        if let Some(url) = self.frontier.pop_front() {
            if self.visited.contains(&url) {
                self.get_fresh_url()
            } else {
                self.visited.insert(url.clone());
                Some(url)
            }
        } else {
            None
        }
    }

    fn send_url_to(&mut self, id: usize) {
        match self.get_fresh_url() {
            Some(url) => self.send_to(id, UrlResp::Url(url)),
            _         => self.send_to(id, UrlResp::EmptyQueue),
        }
    }
}
