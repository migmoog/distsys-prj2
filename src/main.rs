use clap::Parser;
use std::{
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream},
    thread::sleep,
    time::Duration,
};

mod args;
mod failures;
mod state;

use args::PassToken;
use failures::Reasons;
use state::{Data, Message};

const PORT: &'static str = "6969";
const MAX_ATTEMPTS: i32 = 10;
const ATTEMPT_WAIT: Duration = Duration::from_secs(5);

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

    let mut data = Data::from_list(&hostname, &peer_list, 0)?;
    let (before, after) = (data.predecessor - 1, data.successor - 1);
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
        "{} -> [{}] -> {}",
        peer_list[before], hostname, peer_list[after]
    );

    if args.token {
        data.send_message(&mut sender, Message::Token)?;
    }

    let mut tok = listener.accept().map_err(Reasons::IO)?.0;
    let token_delay = Duration::from_secs_f64(args.token_delay.unwrap_or(0.0));
    let marker_delay = Duration::from_secs_f64(args.marker_delay.unwrap_or(0.0));

    let mut channel_values: Vec<Message> = Vec::new();
    loop {
        let mut buffer = [0; 1024];
        let bytes_read = tok.read(&mut buffer[..]).map_err(Reasons::IO)?;
        let received =
            bincode::deserialize(&buffer[..bytes_read]).map_err(|_| Reasons::BadMessage)?;
        channel_values.push(received);

        data.recv_message(received);
        match received {
            Message::Token => {
                sleep(token_delay);
            }
            Message::Marker { snapshot_id } => {
                sleep(marker_delay);
            }
        }
        data.send_message(&mut sender, received)?;
    }
}
