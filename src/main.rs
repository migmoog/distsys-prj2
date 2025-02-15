use bincode::deserialize;
use clap::Parser;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use socketry::{bind_listener, make_channels};
use std::{
    collections::{HashMap, HashSet},
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
    let hostname = hostname::get()
        .map_err(Reasons::IO)?
        .into_string()
        .expect("Host device's name as a string");
    let mut args = PassToken::parse();
    let peer_list: Vec<String> = match File::open(&args.hostsfile) {
        Ok(mut f) => {
            let mut out = String::new();
            let _ = f.read_to_string(&mut out).map_err(Reasons::IO)?;
            out.lines().map(str::to_string).collect()
        }
        Err(e) => return Err(Reasons::IO(e)),
    };

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
    let mut poll_fds: Vec<PollFd> = incoming_channels
        .iter()
        .map(|(_, s)| PollFd::new(s.as_fd(), PollFlags::POLLIN))
        .collect();

    println!("{}", data);
    if args.token {
        // send the first token
        data.pass_token(&mut outgoing_channels)?;
    }

    let token_delay = Duration::from_secs_f64(args.token_delay.unwrap_or(0.0));
    let marker_delay = Duration::from_secs_f64(args.marker_delay.unwrap_or(0.0));

    let mut i_am_initiator = false;
    let mut snapshot_complete = false;
    let mut finished_processes = HashSet::new();
    loop {
        if let (Some(snapshot_id), Some(activate_state)) = (args.snapshot_id, args.snapshot_delay) {
            if !i_am_initiator && snapshot_id == 1 && data.state == activate_state {
                println!(
                    "{{proc_id: {}, snapshot_id: {}, snapshot: \"started\"}}",
                    data.id, snapshot_id
                );
                data.propagate_snapshot(&mut outgoing_channels, snapshot_id, data.id)?;
                i_am_initiator = true;
            }
        }

        if finished_processes.len() == peer_list.len() {
            println!(
                "{{proc_id: {}, snapshot_id: {}, snapshot: \"completed\"}}",
                data.id, 1
            );
            i_am_initiator = false;
            snapshot_complete = true;
        }

        let _events = poll(&mut poll_fds, PollTimeout::NONE).map_err(|e| Reasons::IO(e.into()))?;
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

        for msg in message_queue {
            data.recv_message(msg);
            match msg {
                Message::Token => {
                    sleep(token_delay);
                    data.pass_token(&mut outgoing_channels)?;
                }
                Message::Marker {
                    snapshot_id,
                    initiator,
                    ..
                } => {
                    if snapshot_complete || finished_processes.contains(&data.id) {
                        continue;
                    }
                    sleep(marker_delay);
                    let all_closed =
                        data.propagate_snapshot(&mut outgoing_channels, snapshot_id, initiator)?;
                    if all_closed && i_am_initiator {
                        finished_processes.insert(data.id);
                    }
                }
                Message::AllChannelsClosed { from } => {
                    // CHANDY LAMPORT RULE
                    assert!(i_am_initiator);
                    finished_processes.insert(from);
                }
                Message::ResetSnapshot => {}
            }
        }
    }
}
