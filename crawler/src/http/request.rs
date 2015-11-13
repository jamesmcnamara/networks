use std::fmt;

use itertools::Itertools;

use super::utils::{Method, Header};
 
pub struct Request {
    pub headers: Vec<Header>,
    pub method: Method,
    pub route: String
}

impl Request {
    pub fn new(headers: Vec<Header>, method: Method, route: String) -> Request {
        Request {
            headers: headers,
            method: method,
            route: route
        }
    }

}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut request = format!("{} {} HTTP/1.1\n", self.method, self.route);

        for header in &self.headers {
            request.push_str(&header.to_string());
        }

        if let Method::POST(ref inputs) = self.method {
            let form = inputs
                .iter()
                .map(|&(ref key, ref value)| format!("{}={}", key, value))
                .join("&");
            request.push_str(&format!("{}\n{}\n\n",
                                      Header::ContentLength(form.len()),
                                      form));
        } else {
            request.push_str("\n");
        }
        write!(f, "{}", request)
    }
}
#[test]
fn test_to_string_get() {
    let route = "/home".to_string();
    let headers = vec![
        Header::Host("http://facebook.com".to_string()),
        Header::Connection("Keep-Alive".to_string()),
    ];
    let req = Request::new(headers, Method::GET, route);
    assert_eq!(req.to_string(),
        concat!("GET /home HTTP/1.0\nHost: http://facebook.com\n",
                "Connection: Keep-Alive\n\n")
        .to_string())
}

#[test]
fn test_to_string_post() {
    let route = "/search".to_string();
    let headers = vec![
        Header::Host("http://google.com".to_string()),
        Header::Connection("Keep-Alive".to_string()),
    ];

    let form = vec![("csrf-token", "alsddsfasd"), 
                    ("query", "extension+traits"),
                    ("another", "something")]
               .into_iter()
               .map(|(k, v)| (k.to_string(), v.to_string()))
               .collect();

    let req = Request::new(headers, Method::POST(form), route);
    let expected = concat!("POST /search HTTP/1.0\nHost: http://google.com\n",
                           "Connection: Keep-Alive\nContent-Length: 62\n\n",
                           "csrf-token=alsddsfasd&query=extension+traits&",
                           "another=something\n\n")
        .to_string();
    println!("{}", req.to_string());
    println!("{}", expected);
    assert_eq!(req.to_string(), expected);
}
