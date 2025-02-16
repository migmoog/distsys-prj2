use bincode::deserialize;
use clap::Parser;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use socketry::{bind_listener, make_channels};
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
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
    // extracting peer names from hostfile
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

    // initializing process data and connecting to peers
    let mut data = Data::from_list(&hostname, &peer_list, 0)?;

    let listener = bind_listener(&hostname)?;
    let mut outgoing_channels = make_channels(&hostname, &peer_list)?;
    let mut incoming_channels = HashMap::new();
    let expected_connections = peer_list.len() - 1;
    while incoming_channels.len() < expected_connections {
        if let Ok((sock, _)) = listener.accept() {
            // need this so nix::poll can do its work
            sock.set_nonblocking(true).map_err(Reasons::IO)?;
            incoming_channels.insert(sock.as_raw_fd(), sock);
        }
    }
    // creating an array for poll() calls
    let mut poll_fds: Vec<PollFd> = incoming_channels
        .iter()
        .map(|(_, s)| PollFd::new(s.as_fd(), PollFlags::POLLIN))
        .collect();

    eprintln!("{}", data);
    // send the first token if we have the flag "-x"
    if args.token {
        data.pass_token(&mut outgoing_channels)?;
    }

    let token_delay = Duration::from_secs_f64(args.token_delay.unwrap_or(0.0));
    let marker_delay = Duration::from_secs_f64(args.marker_delay.unwrap_or(0.0));

    loop {
        // check if we're ready to begin the snapshot
        if let (Some(snapshot_id), Some(activate_state)) = (args.snapshot_id, args.snapshot_delay) {
            if !data.seen_marker
                && data.desired_snapshot == snapshot_id
                && data.state == activate_state
            {
                data.initiate_snapshot(&mut outgoing_channels, snapshot_id)?;
            }
        }

        // poll() connections for any events like markers or tokens
        let events = poll(&mut poll_fds, PollTimeout::NONE).map_err(|e| Reasons::IO(e.into()))?;
        if events == 0 {
            continue;
        }
        let mut message_queue = Vec::new();
        for pfd in poll_fds.iter().filter(|pfd| {
            pfd.revents()
                .unwrap_or(PollFlags::empty())
                .contains(PollFlags::POLLIN)
        }) {
            // Read Channel from TCP socket
            let mut buffer = [0u8; 1024];
            let b = incoming_channels
                .get(&pfd.as_fd().as_raw_fd())
                .unwrap()
                .read(&mut buffer)
                .map_err(Reasons::IO)?;
            let msg: Message = deserialize(&buffer[..b]).map_err(|_| Reasons::BadMessage)?;
            message_queue.push(msg);
        }

        // sort so that Markers are FIFO
        message_queue.sort_by_key(|msg| match msg {
            Message::Marker { .. } => 0,
            Message::Token => 1,
        });

        for msg in message_queue {
            data.recv_message(msg, incoming_channels.len());
            match msg {
                // pass the token if we encounter one
                Message::Token => {
                    sleep(token_delay);
                    data.pass_token(&mut outgoing_channels)?;
                }
                // propagate the snapshot if we must
                Message::Marker { snapshot_id, .. } => {
                    sleep(marker_delay);
                    data.propagate_snapshot(&mut outgoing_channels, snapshot_id)?;
                }
            }
        }
    }
}
