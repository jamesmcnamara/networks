use super::utils::Header;

#[derive(Debug)]
pub struct Response {
    pub status: usize,
    pub headers: Vec<Header >,
    pub body: String,

}

impl Response {
    pub fn new(status: usize, headers: Vec<Header>, body: String) -> Response {
        Response {
            status: status,
            headers: headers,
            body: body,
        }
    }
    
    pub fn parse_cookie(cookie: &str) -> String {
        match cookie.find(";") {
            Some(idx) => &cookie[..idx + 1],
            None      => cookie
        }.to_string()
    }
}
