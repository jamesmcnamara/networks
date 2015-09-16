#![warn(dead_code)]
extern crate argparse;

use argparse::{ArgumentParser, Store, StoreTrue};
use std::io::{Error, Read, Write};
use std::net::TcpStream;

/// Structure to hold results of parsing command line arguments
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


/// Response is a type that represents a message to be sent to the server,
/// and its contents. The enumeration misleadingly contains Bye, as it it used
/// to signal the end of the read loop and return the secret flag to the user 
enum Response {
    Hello(String),
    Solution(i64),
    Bye(String),
}

impl Response {

    /// Generates the properly formatted message for each response type,
    /// and returns it in its byte encoding
    fn to_bytes(&self) -> Vec<u8> {
        let msg = match *self {
            Response::Hello(ref id)        => format!("HELLO {}", id),
            Response::Solution(ref answer) => format!("{}", answer),
            Response::Bye(ref secret)      => format!("BYE {}", secret),
        };
        format!("cs3700fall2015 {}\n", msg).as_bytes().to_owned()
    }
}


/// Consumes an arry of strings describing an arithmetic problem, and 
/// returns the answer
/// # Examples
/// ```
/// let x = ["5", "+", "3"];
/// assert_eq!(&x, 8);
/// ```
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


/// Parses the given message and determines whether it's a problem or a 
/// response, and returns the appropriate response
/// # Examples
/// ```
/// let x = "cs3700fall2015 STATUS 5 + 3\n";
/// assert_eq!(parse_message(x), Response::Solution(8))
/// ```
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
        Some(msg)      => panic!("{} is not implemented", msg),
        None           => panic!("Input EOF before message code"),
    }
}


/// Attempts to read a single line from the given TCP Socket
/// Returns an error if cloning the socket fails or parsing the 
/// bits as ASCII fails
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

/// Main loop of execution of the program. Continuously tries to read a message
/// from the socket, and take the appropriate action until the BYE message is 
/// received rom the server. 
/// Panics on non STATUS/BYE status codes
/// Returns an error on parsing issues
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


/// Parses command line arguments and returns a UserPrefs struct containing
/// the appropriate settings
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


/// Given a set of preferences, connects to the TCP socket, writes the 
/// HELLO message, and returns the socket
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
