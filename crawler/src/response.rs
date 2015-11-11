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
            200 => None,
            301 => None,
            403 => None,
            500 => Some(vec![self.url.clone()]),
            code => unreachable!("Received code {}, could not parse", code)
        };
        if let Some(urls) = new_urls { self.sender.send(UrlMsg::Add(urls)); }
    }
}
