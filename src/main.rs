use bincode::deserialize;
use clap::Parser;
use socketry::{bind_listener, connect_channel, make_channels};
use std::{
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream},
    thread::sleep,
    time::Duration,
};

// CLI arguments
mod args;
// Error types
mod failures;
// data for PassToken
mod state;
// helpers for establishing connections
mod socketry;

use args::PassToken;
use failures::Reasons;
use state::{Data, Message};

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
    println!("{}", data);

    let mut listener = bind_listener(&hostname)?;
    let mut outgoing_channels = make_channels(&hostname, &peer_list)?;

    // means we're ready to go!
    // project said "no unneccesary prints"
    println!(
        "{} -> [{}] -> {}",
        peer_list[data.predecessor - 1],
        hostname,
        peer_list[data.successor - 1]
    );

    if args.token {
        // send the first token
        data.pass_token(&mut outgoing_channels)?;
    }

    let token_delay = Duration::from_secs_f64(args.token_delay.unwrap_or(0.0));
    let marker_delay = Duration::from_secs_f64(args.marker_delay.unwrap_or(0.0));

    loop {}
}
