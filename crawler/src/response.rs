use std::sync::mpsc;

use data::UrlMsg;

struct Response {
    status: usize,
    headers: Vec<String>,
    sender: mpsc::Sender<UrlMsg>,
    url: String
}

impl Response {
    fn new(status: usize,
           headers: Vec<String>,
           sender: mpsc::Sender<UrlMsg>,
           url: String) -> Response {
        Response {
            status: status,
            headers: headers,
            sender: sender,
            url: url
        }
    }

    fn respond(&self) {
        let mut new_urls = Vec::new();
        match self.status {
            200 => new_urls.push_all(&parse_ok(&self.headers)),
            301 => new_urls.push(parse_moved(&self.headers)),
            403 => (),
            500 => new_urls.push(self.url.clone()),
            _ => unreachable!()
        }
        if new_urls.is_empty() {
            self.sender.send(UrlMsg::Add(new_urls));
        }
    }
}

fn parse_ok(headers: &[String]) -> Vec<String> {
    Vec::new()
}

fn parse_moved(headers: &[String]) -> String {
    "hello".to_string()
}
