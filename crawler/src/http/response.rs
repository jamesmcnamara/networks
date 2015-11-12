use itertools::Itertools;

use super::utils::Header;

#[derive(Debug)]
pub struct Response {
    pub status: usize,
    pub headers: Vec<Header >,
    pub body: String,

}

impl Response {
    pub fn new(resp: String) -> Response {
        let mut lines = resp.lines();
        let status = Response::parse_status(lines.next());
        let not_empty = |line: &&str| !line.is_empty();
        let headers: Vec<_> = lines
            .take_while(&not_empty)
            .map(Response::split_header)
            .filter_map(Response::parse_header)
            .collect();

        let body = resp
            .lines()
            .skip_while(&not_empty)
            .join("\n");

        Response {
            status: status,
            headers: headers,
            body: body,
        }
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

    fn split_header(header: &str) -> Option<(&str, &str)> {
        match header.find(":") {
            Some(index) => Some((&header[..index], &header[index + 2..])),
            _           => None,
        }
    }
    
    fn parse_header(header: Option<(&str, &str)>) -> Option<Header> {
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
            Some(("Location", loc))       => Some(Header::Location(loc.trim().to_string())),
            Some(("Connection", status))  => Some(Header::Connection(status.trim().to_string())),
            _                             => None
        }
    }

    fn parse_cookie(cookie: &str) -> String {
        match cookie.find(";") {
            Some(idx) => &cookie[..idx + 1],
            None      => cookie
        }.to_string()
    }
}
