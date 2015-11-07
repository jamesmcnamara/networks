use std::collections::HashMap;

enum Header {
    Host(String),
    Connection(String),
    Cookie(String),
    AcceptEncoding(String)
}

impl Header {
    fn to_string(&self) -> String {
        match *self {
            Header::Host(ref host) =>
                format_header_text("Host", host),
            Header::Connection(ref connection) =>
                format_header_text("Connection", connection),
            Header::Cookie(ref cookie) =>
                format_header_text("Cookie", cookie),
            Header::AcceptEncoding(ref accept_encoding) =>
                format_header_text("Accept-Encoding", accept_encoding)
        }
    }
}

fn format_header_text(name: &str, value: &str) -> String {
    format!("{}: {}\n", name, value)
}

enum Method {
    GET,
    POST(HashMap<String, String>)
}

impl Method {
    fn to_string(&self) -> &'static str {
        match *self {
            Method::GET => "GET",
            Method::POST(_) => "POST"
        }
    }
}

struct Request {
    headers: Vec<Header>,
    method: Method,
    url: String
}

impl Request {
    fn new(headers: Vec<Header>, method: Method, url: String) -> Request {
        Request {
            headers: headers,
            method: method,
            url: url
        }
    }

    fn send(&self) {
        let mut request = String::new();
        request.push_str(&format!("{} {} HTTP/1.0", self.method.to_string(), self.url));
        for header in &self.headers {
            request.push_str(&header.to_string());
        }
        if let Method::POST(ref inputs) = self.method {
            request.push_str("\n");
            for (name, value) in inputs.iter() {
                request.push_str(&format!("{}={}\n", name, value))
            }
        }
    }
}
