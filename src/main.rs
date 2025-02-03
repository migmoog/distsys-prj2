use clap::Parser;
use std::{
    fs::File,
    io::{prelude::*, Read},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread::sleep,
    time::Duration,
};

mod failures;
mod state;

use failures::Reasons;
use state::Data;

const TOKEN_MSG: &'static str = "token";

#[derive(Parser, Debug)]
#[command(author = "Jeremy Gordon <jeremygordon.dev>")]
struct PassToken {
    #[arg(short = 'x')]
    token: bool,

    #[arg(short = 'h')]
    hostsfile: PathBuf,

    #[arg(short = 'm')]
    marker_delay: Option<f64>,

    #[arg(short = 's')]
    state: Option<u32>,

    #[arg(short = 't')]
    token_delay: Option<f64>,
}

const PORT: &'static str = "6969";
const MAX_ATTEMPTS: i32 = 10;
const ATTEMPT_WAIT: Duration = Duration::from_secs(5);

fn pass_print(id: usize, sender: usize, receiver: usize, msg: &str) {
    println!(
        "{{id: {}, sender: {}, receiver: {}, message: \"{}\"}}",
        id, sender, receiver, msg
    );
}

fn send_token(sender: &mut TcpStream, data: &Data) -> Result<(), Reasons> {
    let _ = sender
        .write_all(TOKEN_MSG.as_bytes())
        .map_err(Reasons::IO)?;
    pass_print(data.id, data.id, data.successor, TOKEN_MSG);
    Ok(())
}

fn main() -> Result<(), Reasons> {
    let hostname = hostname::get()
        .map_err(Reasons::IO)?
        .into_string()
        .expect("Host device's name as a string");
    let args = PassToken::parse();
    let peer_list: Vec<String> = match File::open(&args.hostsfile) {
        Ok(mut f) => {
            let mut out = String::new();
            let _ = f.read_to_string(&mut out).map_err(Reasons::IO)?;
            out.lines().map(str::to_string).collect()
        }
        Err(e) => return Err(Reasons::IO(e)),
    };

    let (mut data, before, after) = Data::from_list(&hostname, &peer_list, 0)?;
    println!("{}", data);

    let mut attempts = 0;
    let listener_address = format!("{}:{}", hostname, PORT);
    let listener = loop {
        match TcpListener::bind(&listener_address) {
            Ok(l) => break l,
            Err(e) => {
                if attempts == MAX_ATTEMPTS {
                    return Err(Reasons::IO(e));
                }
                attempts += 1;
                sleep(ATTEMPT_WAIT);
            }
        }
    };
    attempts = 0;

    let sendable_address = format!("{}:{}", peer_list[after], PORT);
    let mut sender = loop {
        match TcpStream::connect(&sendable_address) {
            Ok(s) => break s,
            Err(e) => {
                if attempts == MAX_ATTEMPTS {
                    return Err(Reasons::IO(e));
                }
                attempts += 1;
                sleep(ATTEMPT_WAIT);
            }
        }
    };

    // means we're ready to go!
    println!(
        "{} -> [{}]-> {}",
        peer_list[before], hostname, peer_list[after]
    );

    if args.token {
        send_token(&mut sender, &data)?;
    }

    let mut tok = listener.accept().map_err(Reasons::IO)?.0;
    loop {
        let mut buffer = [0u8; 1024];
        let _ = tok.read(&mut buffer[..]).map_err(Reasons::IO)?;
        let received = String::from_utf8_lossy(&buffer[..]);
        pass_print(data.id, data.predecessor, data.id, &received);
        data.update_token();
        sleep(Duration::from_secs_f64(args.token_delay.unwrap_or(1.0)));
        send_token(&mut sender, &data)?;
    }
}
