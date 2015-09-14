extern crate argparse;

use argparse::{ArgumentParser, Store, StoreTrue};
use std::io::{Error, Read, Write};
use std::net::{TcpStream, IpAddr};
use std::str::FromStr;

struct UserPrefs {
    port: u16,
    secure: bool,
    nuid: String,
    hostname: String
}

impl UserPrefs {
    fn new(port: u16, secure: bool, nuid: String, hostname: String) -> UserPrefs {
        UserPrefs{ port: port, 
                   secure: secure,
                   nuid: nuid,
                   hostname: hostname }
    }
}

enum Response {
    Hello(String),
    Solution(i64),
    Bye(String),
}

impl Response {
    fn to_bytes(&self) -> Vec<u8> {
        let msg = match self {
            &Response::Hello(ref id)        => format!("HELLO {}", id),
            &Response::Solution(ref answer) => format!("{}", answer),
            &Response::Bye(ref secret)      => format!("BYE {}", secret),
        };
        format!("cs3700fall2015 {}\n", msg).as_bytes().to_owned()
    }
}

fn parse_args() -> UserPrefs {
    let mut port = 27993;
    let mut secure = false;
    let mut nuid = String::new();
    let mut hostname = String::new();
    {
        let mut ap = ArgumentParser::new();
        ap.set_description("Solves mad math problems");
        ap.refer(&mut hostname)
            .add_argument("hostname", Store, "Hostname to connect to").required();
        ap.refer(&mut nuid)
            .add_argument("nuid", Store, "NUID to register").required();
        ap.refer(&mut port)
            .add_option(&["-p", "--port"], Store, "Port to connect to");
        ap.refer(&mut secure)
            .add_option(&["-s", "--secure"], StoreTrue, "Use SSL");
        ap.parse_args_or_exit();
    }

    UserPrefs::new(port, secure, nuid, hostname)
}

fn solve_problem(equation: &[&str]) -> i64 {
    // Using the experimental language feature "slice_patterns", the code could
    // be as pretty as this
    // let (fst, opp, snd) = match equation {
    //     [fst, opp, snd] => (fst.parse::<i64>(), opp, snd.parse::<i64>()),
    //     _               => panic!("Poorly formatted message"),
    // };
    // instead:
    let (fst, opp, snd) = (equation[0].parse::<i64>(), 
                           equation[1], 
                           equation[2].parse::<i64>());
    match (fst, snd) {
        (Ok(fst), Ok(snd)) => match opp {
            "+" => fst + snd,
            "-" => fst - snd,
            "*" => fst * snd,
            "/" => fst / snd,
            _   => unreachable!("Operator not recognized: {}", opp) 
        },
        _                 => unreachable!("Parse int failure")
    }
}

fn parse_message(line: &str) -> Response {
    // Skip the course information
    let mut words = line.split_whitespace().skip(1);

    match words.next() {
        Some("STATUS") => 
            Response::Solution(solve_problem(&words.collect::<Vec<_>>())),
        Some("BYE")    => 
            Response::Bye(words.next()
                          .expect("BYE did not include secret flag")
                          .to_string()),
        Some(msg)   => panic!("{} is not implemented", msg),
        None      => panic!("Input EOF before message code"),
    }
}

fn read_line(socket: &mut TcpStream) -> Result<String, Error> {
    match socket.try_clone() {
        Ok(socket) => Ok(socket.bytes()
                         .map(|letter| 
                              letter.ok().expect("Error parsing letter") as char)
                         .take_while(|letter| *letter != '\n')
                         .collect()),
        Err(e) => Err(e),
    }
}

fn read_loop(mut socket: TcpStream) -> Result<(), Error> {
    loop {
        let message = try!(read_line(&mut socket)); 
        match parse_message(&message) {
            solution @ Response::Solution(_) => {
                try!(socket.write(&solution.to_bytes())); 
            },
            Response::Bye(secret) => { 
               println!("Success! The secret message was {}", secret);
               break;
            },
            Response::Hello(_)    => 
                unreachable!("Server sent back Hello. Possible echo server"),
        }
    }
    Ok(())
}

fn init(prefs: UserPrefs) -> Result<TcpStream, Error> {
    let mut socket = 
        try!(TcpStream::connect((&prefs.hostname[..], prefs.port)));
    try!(socket.write(&Response::Hello(prefs.nuid).to_bytes()));
    Ok(socket)
}

fn main() {
    match init(parse_args()).map(read_loop) {
        Ok(_)  => println!("The operation was a complete success!"),
        Err(e) => println!("The operation failed with code {}", e),
    }
}

#[test]
fn test_solve_problem() {
    assert_eq!(solve_problem(&["5", "*", "3"]), 15);
    assert_eq!(solve_problem(&["10", "/", "3"]), 3);
}
