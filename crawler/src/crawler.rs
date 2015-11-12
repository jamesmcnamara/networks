use std::net;
use std::io::{Read, Write};
use std::ops::Deref;
use std::sync::mpsc;

use itertools::Itertools;

use data::{UrlReq, UrlResp};
use http::utils::{Header, Method};
use http::request::Request;
use http::response::Response;

pub struct Crawler {
   req_chan: mpsc::Sender<UrlReq>,
   resp_recv: mpsc::Receiver<UrlResp>,
   id: usize,
   host: String,
   pipe: net::TcpStream,
}

impl Crawler {
    pub fn new(req_chan: mpsc::Sender<UrlReq>, 
           resp_recv: mpsc::Receiver<UrlResp>,
           id: usize,
           host: &str) -> Crawler {
        Crawler {
            req_chan: req_chan,
            resp_recv: resp_recv,
            id: id,
            host: host.to_string(),
            pipe: net::TcpStream::connect(host).unwrap(),
        }
    }
   
    pub fn login(&mut self, page: String, user: String, pass: String) {
        let headers = self.get_headers();
        let csrf_token = headers
            .iter()
            .filter_map(Crawler::get_csrf_token)
            .next()
            .expect("no csrf token in cookies")
            .to_string();
        let form = vec![("username".to_string(), user), 
                        ("password".to_string(), pass),
                        ("csrfmiddlewaretoken".to_string(), csrf_token)];
        let req = Request::new(headers, Method::POST(form), page);
        self.send_request(req);
    }

    fn get_csrf_token(header: &Header) -> Option<&str> {
        fn split_pairs(pair: &str) -> Option<(&str, &str)> {
            pair.find("=").map(|idx| (&pair[..idx], &pair[idx + 1..]))
        }

        if let Header::Cookie(ref cookie) = *header {
            cookie.split(";")
                .filter_map(split_pairs)
                .filter_map(|(key, val)| if key == "csrftoken" {Some(val)} else {None})
                .next()
        } else {
            None
        }
    }

    pub fn get_page(&mut self, page: String) {
        let req = Request::new(self.get_headers(), Method::GET, page);
        self.send_request(req);
    }


    pub fn get_next_page(&mut self) {
        loop {
            if let Err(_) = self.req_chan.send(UrlReq::Request(self.id)) {
                return;
            }
            if let Ok(UrlResp::Url(url)) = self.resp_recv.recv() {
                let req = Request::new(self.get_headers(), Method::GET, url);
                self.send_request(req);
            }
        }
    }
    
    pub fn send_request(&mut self, req: Request) {
        println!("req is {}", req);
        drop(self.pipe.write(req.to_string().as_bytes()));
        self.handle_response(req.route.clone());
    }

    fn handle_response(&mut self, route: String) {
        let mut resp = String::new();
        drop(self.pipe.read_to_string(&mut resp));
        println!("raw response was {}", resp);
        let resp = Response::new(resp); 
        self.process_headers(&resp.headers);
        let new_urls = match resp.status {
            200...299 => Some(self.parse_body(&resp.body)),
            300...399 => Some(vec![self.get_redirect(&resp.headers)]),
            400...499 => None,
            500...599 => Some(vec![route]),
            code => unreachable!("Received code {}, could not parse", code)
        };
         if let Some(urls) = new_urls {
             drop(self.req_chan.send(UrlReq::Add(urls)));
        }
        println!("response is {:?}", resp);
    }

    fn get_redirect(&self, headers: &[Header]) -> String {
        for header in headers {
            if let Header::Location(ref loc) = *header {
                return loc.clone();
            }
        }
        panic!("request with redirect did not have location field");
    }

    fn process_headers(&mut self, headers: &[Header]) {
        for header in headers {
            match *header {
                Header::SetCookie(ref cookie) => {
                    drop(self.req_chan.send(UrlReq::SetCookie(cookie.clone())))
                },
                Header::Connection(ref status) if status == "close" => {
                    self.reset_connection();
                },
                _ => continue,
            }
        }
    }
    
    pub fn parse_body(&self, body: &str) -> Vec<String> {
        vec![]
    }

    fn reset_connection(&mut self) {
        self.pipe = net::TcpStream::connect(self.host.deref()).unwrap();
    }

    fn get_headers(&self) -> Vec<Header> {
        let mut headers = vec![//Header::Connection("Keep-Alive".to_string()),
                               Header::Host(self.host.clone())];
        drop(self.req_chan.send(UrlReq::GetCookie(self.id)));
        if let Ok(UrlResp::Cookie(Some(cookie))) = self.resp_recv.recv() {
            headers.push(Header::Cookie(cookie)); 
        }
        headers
    }
}
