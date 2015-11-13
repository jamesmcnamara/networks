use std::fmt;
use std::str::Lines;

use super::response::Response;

#[derive(Debug)]
pub enum Header {
    Host(String),
    Connection(String),
    SetCookie(String),
    Cookie(String),
    AcceptEncoding(String),
    ContentLength(usize),
    Location(String),
    TransferEncoding(String),
}

impl Header {
    pub fn from_lines(resp: Lines) -> Vec<Header> {
        resp.take_while(|line| !line.is_empty())
            .map(Header::split_header)
            .filter_map(Header::parse_header)
            .collect()
    }


    fn split_header(header: &str) -> Option<(&str, &str)> {
        match header.find(":") {
            Some(index) => Some((&header[..index], &header[index + 2..])),
            _           => None,
        }
    }
    
    fn parse_header(header: Option<(&str, &str)>) -> Option<Header> {
        fn trim(s: &str) -> String {
            s.trim().to_string()
        }
        match header {
            Some(("Content-Length", len)) => {
                match len.trim().parse() {
                    Ok(len) => Some(Header::ContentLength(len)),
                    Err(_)  => None,
                }
            },
            Some(("Set-Cookie", cookie))   =>  {
                let cookie = Response::parse_cookie(cookie);
                Some(Header::SetCookie(cookie))
            },
            Some(("Location", loc))                => Some(Header::Location(trim(loc))),
            Some(("Connection", status))           => Some(Header::Connection(trim(status))),
            Some(("Transfer-Encoding", strategy))  => Some(Header::TransferEncoding(trim(strategy))),
            _                                      => None
        }
    }

    pub fn chunked_encoding(headers: &[Header]) -> bool {
        for header in headers {
            if let Header::TransferEncoding(ref strategy) = *header {
                return strategy == "chunked";
            }
        }
        false
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Header::Host(ref host) => 
                write!(f, "Host: {}\n", host), 
            Header::Connection(ref connection) => 
                write!(f, "Connection: {}\n", connection),
            Header::SetCookie(ref cookie) => 
                write!(f, "SetCookie: {}\n", cookie),
            Header::Cookie(ref cookie) => 
                write!(f, "Cookie: {}\n", cookie),
            Header::AcceptEncoding(ref accept_encoding) => 
                write!(f, "Accept-Encoding: {}\n", accept_encoding),
            Header::Location(ref loc) => 
                write!(f, "Location: {}\n", loc),
            Header::ContentLength(ref len) => 
                write!(f, "Content-Length: {}\n", len),
            Header::TransferEncoding(ref strategy) => 
                write!(f, "Transfer-Encoding: {}\n", strategy),
        }
    }
}

pub enum Method {
    GET,
    POST(Vec<(String, String)>)
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Method::GET     => write!(f, "GET"),
            Method::POST(_) => write!(f, "POST"),
        }
    }
}

