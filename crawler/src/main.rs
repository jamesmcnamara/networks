extern crate itertools;
extern crate select;

pub mod data;
pub mod http;
pub mod crawler;
pub mod parse;

use std::env;
use std::thread;
use std::sync::mpsc;

use itertools::Itertools;

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

fn fork_crawler(crawler: &Crawler, us: &mut UrlStore, host: &str, id: usize) -> Crawler {
    let req_chan = crawler.clone_chan();
    let (resp_sx, resp_rx) = mpsc::channel();
    us.add_crawler(resp_sx);
    Crawler::new(req_chan, resp_rx, id, host)
}

fn main() {
    let (user, pass) = user_and_pass();
    let url = "/accounts/login/?next=/fakebook/";
    let host = "fring.ccs.neu.edu:80";
    let (req_sx, req_rx) = mpsc::channel();
    let (resp_sx, resp_rx) = mpsc::channel();
    let mut url_store = UrlStore::new(req_rx, vec![resp_sx]);
    let mut crawler = Crawler::new(req_sx, resp_rx, 0, host);
    let crawlers = (1..10)
        .map(|i| fork_crawler(&crawler, &mut url_store, host, i))
        .collect_vec();
    thread::spawn(move || url_store.listen());
    crawler.get_page(url.to_string());
    crawler.login(url.to_string(), user, pass);
    for crawler in crawlers {
        thread::spawn(move || crawler.get_next_page());
    }
    crawler.get_next_page();
}
