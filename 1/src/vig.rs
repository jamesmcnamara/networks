#![feature(slice_patterns)]

use std::net::TcpStream;

fn main() {
    println!("{:?}", parse_response("coursename STATUS 5 - 8\n".to_string()));
}

fn solve(first: String, op: String, second: String) -> i32 {
    if let (Ok(first), Ok(second)) = (first.parse::<i32>(), second.parse::<i32>()) {
        match op {
            "+" => first + second,
            "-" => first - second,
            "/" => first / second,
            "*" => first * second,
            _   => unreachable!("BIIIIIITCH")
        }
    }
    else {
        panic!("meow")
    }
}

#[derive(Debug)]
enum Response {
    Status(i32),
    Bye(String)
}

fn parse_response(message: String) -> Response {
    let response = message
        .chars()
        .takewhile(|c| c != '\n')
        .split(' ')
        .collect::<Vec<_>>();
    match response {
        [_, "STATUS", first, op, second] => Response::Status(solve(first, op, second)),
        [_, "BYE", secret] => Response::Bye(secret),
        _ => unreachable!("UGH")
    }
}
