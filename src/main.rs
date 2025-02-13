use bincode::deserialize;
use clap::Parser;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use socketry::{bind_listener, connect_channel, make_channels};
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream},
    os::fd::{AsFd, AsRawFd},
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
    let mut incoming_channels = HashMap::new();
    let expected_connections = peer_list.len() - 1;
    while incoming_channels.len() < expected_connections {
        if let Ok((mut sock, _)) = listener.accept() {
            // need this so nix::poll can do its work
            sock.set_nonblocking(true).map_err(Reasons::IO)?;
            incoming_channels.insert(sock.as_raw_fd(), sock);
        }
    }

    let mut poll_fds: Vec<PollFd> = incoming_channels
        .iter()
        .map(|(_, s)| PollFd::new(s.as_fd(), PollFlags::POLLIN))
        .collect();

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

    loop {
        let events = poll(&mut poll_fds, PollTimeout::NONE).map_err(|e| Reasons::IO(e.into()))?;

        if events == 0 {
            continue;
        }

        for pfd in poll_fds.iter().filter(|pfd| {
            pfd.revents()
                .unwrap_or(PollFlags::empty())
                .contains(PollFlags::POLLIN)
        }) {
            let mut buffer = [0u8; 1024];
            let b = incoming_channels
                .get(&pfd.as_fd().as_raw_fd())
                .unwrap()
                .read(&mut buffer)
                .map_err(Reasons::IO)?;
            let msg: Message = deserialize(&buffer[..b]).map_err(|_| Reasons::BadMessage)?;

            data.recv_message(msg);
            match msg {
                Message::Token => {
                    sleep(token_delay);
                    data.pass_token(&mut outgoing_channels)?;
                }
                _ => {}
            }
        }
    }
}
