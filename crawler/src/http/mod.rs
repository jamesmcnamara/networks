pub mod request;
pub mod response;
pub mod utils;

trait Joinable {
    fn join(self, connect: &str) -> String;
}

impl <I: Iterator<Item=String>> Joinable for I { 
    fn join(self, connect: &str) -> String {
        let mut untrimmed_result = 
            self.fold(String::new(), 
                      |acc, line| acc + &line + connect);
        drop(untrimmed_result.pop());
        untrimmed_result
    }
}

