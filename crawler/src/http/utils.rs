use std::fmt;

#[derive(Debug)]
pub enum Header {
    Host(String),
    Connection(String),
    SetCookie(String),
    Cookie(String),
    AcceptEncoding(String),
    ContentLength(usize),
    Location(String),
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

