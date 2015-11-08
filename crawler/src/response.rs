use std::sync::mpsc;

use data::UrlMsg;

struct Response {
    status: usize,
    headers: Vec<String>,
    sender: mpsc::Sender<UrlMsg>,
    url: String
}

impl Response {
    fn new(status: usize, headers: Vec<String>, sender: mpsc::Sender<UrlMsg>,
           url: String) -> Response {
        Response {
            status: status,
            headers: headers,
            sender: sender,
            url: url
        }
    }

    fn respond(&self) {
        let new_urls = match self.status {
            200 => Some(parse_ok(&self.headers)),
            301 => Some(vec![parse_moved(&self.headers)]),
            403 => None,
            500 => Some(vec![self.url.clone()]),
            code => unreachable!("Received code {}, could not parse", code)
        };
        if let Some(urls) = new_urls {
            self.sender.send(UrlMsg::Add(urls));
        }
    }
}

fn parse_ok(headers: &[String]) -> Vec<String> {
    Vec::new()
}

fn parse_moved(headers: &[String]) -> String {
    "hello".to_string()
}
