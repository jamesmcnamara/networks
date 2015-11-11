use select::dom::Dom;
use select::node::Node;
use select::predicate::*;

pub fn parse_html(html: &str) -> (Vec<String>, Option<String>) {
    let document = Dom::from_str(html);
    let secret = document
        .find(Class("secret_flag"))
        .first()
        .map(|secret| secret.text());
    let links = document
        .find(Name("a"))
        .iter()
        .map(|a| a.text())
        .collect();
    (links, secret)
}
