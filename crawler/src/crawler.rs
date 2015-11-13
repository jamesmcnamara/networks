use std::net;
use std::io::{Read, Write};
use std::ops::Deref;
use std::str::Lines;
use std::sync::mpsc;

use itertools::Itertools;

use data::{UrlReq, UrlResp};
use http::utils::{Header, Method};
use http::request::Request;
use http::response::Response;
use parse::parse_html;

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


    pub fn get_next_page(mut self) {
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
        //println!("req is {}", req);
        drop(self.pipe.write(req.to_string().as_bytes()));
        self.handle_response(req.route.clone());
    }

    fn add_chunks_continue(&self, lines: Vec<String>, body: &mut String) -> bool {
        let mut lines = lines.into_iter();
        while let Some(line) = lines.next() {
            let mut len = Crawler::parse_len(&line);
            if len == 0 {
                return false;
            }
            while len > 0 {
                match lines.next() {
                    Some(line) => {
                        body.push_str(&line);
                        len -= line.len();
                    },
                    None => {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn some_thing(&mut self, lines: Vec<String>, mut body: String) -> String {
        if self.add_chunks_continue(lines, &mut body) {
            let mut resp = String::new();
            drop(self.pipe.read_to_string(&mut resp));
            self.some_thing(resp.lines().map(str::to_string).collect(), body)
        } else {
            body
        }
    }

    fn read_body(&mut self, lines: Vec<String>, headers: &[Header]) -> String {
        if Header::chunked_encoding(&headers) {
            //println!("chunked is true, data is {:?}", lines);
            self.some_thing(lines, String::new())
        } else {
            lines.into_iter().join("\n")
        }
    }

    fn handle_response(&mut self, route: String) {
        let mut resp = String::new();
        drop(self.pipe.read_to_string(&mut resp));
        //println!("raw response is {}", resp);
        let lines = resp.lines().map(str::to_string).collect_vec();
        let mut resp_lines = resp.lines();
        let mut body_lines = resp
            .lines()
            .map(str::trim)
            .skip_while(|line| !line.is_empty())
            .skip(1);
        let status = Crawler::parse_status(resp_lines.next());
        let headers = Header::from_lines(resp_lines);
        //println!("headers are {:?}", headers);
        self.process_headers(&headers);
        let body = self.read_body(body_lines.map(str::to_owned).collect(), &headers);
        //println!("raw response was {}", resp);
        let resp = Response::new(status, headers, body); 
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
        //println!("response is {:?}", resp);
    }

    fn parse_status(status_line: Option<&str>) -> usize {
        let status = status_line
            .expect("http response had no content")
            .split_whitespace()
            .nth(1)
            .map(str::parse);
        match status {
            Some(Ok(n)) => n,
            _           => panic!("Parsing of status code failed: {:?}", status),
        }
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
        let (urls, sf) = parse_html(body);
        if let Some(sf) = sf {
            self.req_chan.send(UrlReq::Flag(sf));
        }
        urls
    }

    fn reset_connection(&mut self) {
        self.pipe = net::TcpStream::connect(self.host.deref()).unwrap();
    }


    fn parse_len(line: &str) -> usize {
        let hex_len = match line.split(";").next() {
            Some(slice) => slice.trim(),
            None        => line.trim(),
        };
        usize::from_str_radix(hex_len, 16)
            .ok()
            .expect(&format!("invalid hex for content len: {}", line))
    }

    fn get_headers(&self) -> Vec<Header> {
        let mut headers = vec![Header::Connection("close".to_string()),
                               Header::Host(self.host.clone())];
        drop(self.req_chan.send(UrlReq::GetCookie(self.id)));
        if let Ok(UrlResp::Cookie(Some(cookie))) = self.resp_recv.recv() {
            headers.push(Header::Cookie(cookie)); 
        }
        headers
    }
    
    pub fn clone_chan(&self) -> mpsc::Sender<UrlReq> {
        self.req_chan.clone()
    }


}
