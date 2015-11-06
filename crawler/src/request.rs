use std::collections::HashMap;

enum Header {
    Host(String),
    Connection(String),
    Cookie(String),
    AcceptEncoding(String)
}

enum Method {
    GET,
    POST(HashMap<String, String>)
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
        let method_text = match self.method {
            Method::GET => "GET",
            Method::POST(_) => "POST"
        };
        request.push_str(&format!("{} {} HTTP/1.0", method_text, self.url));
        for header in &self.headers {
            let header_text = match *header {
                Header::Host(ref host) =>
                    format_header_text("Host", host),
                Header::Connection(ref connection) =>
                    format_header_text("Connection", connection),
                Header::Cookie(ref cookie) =>
                    format_header_text("Cookie", cookie),
                Header::AcceptEncoding(ref accept_encoding) =>
                    format_header_text("Accept-Encoding", accept_encoding)
            };
            request.push_str(&header_text);
        }
        match self.method {
            Method::GET => (),
            Method::POST(ref inputs) => {
                request.push_str("\n");
                for (name, value) in inputs.iter() {
                    request.push_str(&format!("{}={}\n", name, value))
                }
            }
        }
    }
}

fn format_header_text(name: &str, value: &str) -> String {
    format!("{}: {}\n", name, value)
}
