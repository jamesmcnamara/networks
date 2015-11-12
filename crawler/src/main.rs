extern crate itertools;

pub mod data;
pub mod http;
pub mod crawler;

use std::env;
use std::thread;
use std::sync::mpsc;

use self::crawler::Crawler;
use self::data::UrlStore;

fn user_and_pass() -> (String, String) {
   let mut args = env::args().skip(1).take(2);
    let username = args
        .next()
        .expect("the first command line argument must be a username");

    let password = args
        .next()
        .expect("the second command line argument must be a password");

    (username, password)
}

fn main() {
    let (user, pass) = user_and_pass();
    let url = "/accounts/login/?next=/fakebook/";
    let host = "fring.ccs.neu.edu:80";
    let (req_sx, req_rx) = mpsc::channel();
    let (resp_sx, resp_rx) = mpsc::channel();
    let url_store = UrlStore::new(req_rx, vec![resp_sx]);
    let mut crawler = Crawler::new(req_sx, resp_rx, 0, host);
    thread::spawn(move || url_store.listen());
    crawler.get_page(url.to_string());
    crawler.login(url.to_string(), user, pass);
    crawler.get_next_page();
}
