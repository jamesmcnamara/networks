use std::fs::File;
use std::io::Read;

use select::dom::Dom;
use select::node::Node;
use select::predicate::*;

pub fn parse_html(html: &str) -> (Vec<String>, Option<String>) {
    let document = Dom::from_str(html);
    let secret = document
        .find(Class("secret_flag"))
        .first()
        .map(|secret| secret
             .text()
             .split(": ")
             .nth(1)
             .expect(&format!("Secret flag was malformed, {}", secret.text()))
             .to_string());

    let links = document
        .find(Name("a"))
        .iter()
        .filter_map(|a| a.attr("href").map(str::to_string))
        .filter(|ln| {
                println!("link is: {}", ln);
                ln.starts_with("/") 
                || ln.starts_with("http://fring.ccs.neu.edu")})
        .collect();
    (links, secret)
}

#[test]
fn test_parse() {
    let mut html_file = File::open("/Users/jamesmcnamara/workspace/networks/crawler/test/data.html").unwrap();
    let mut html = String::new();
    drop(html_file.read_to_string(&mut html));
    let (links, secret_flag) = parse_html(&html);
    let should_be_links = 
        vec!["/fakebook/", "/fakebook/871550943/", "/fakebook/871595319/", 
             "/fakebook/871603334/", "/fakebook/873257087/", 
             "/fakebook/873467579/", "/fakebook/873637318/", 
             "/fakebook/873659837/", "/fakebook/873725940/", 
             "/fakebook/873878302/", "/fakebook/874157510/"];
    let should_be_secret_flag = 
        "1234567890abcdefghijklmnopqrstuvwxyz1234567890abcdefghijklmnopqr";

    assert_eq!(links, should_be_links);
    assert_eq!(secret_flag, Some(should_be_secret_flag.to_string()));
}
