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

    // means we're ready to go!
    // project said "no unneccesary prints"
    /*println!(
        "{} -> [{}] -> {}",
        peer_list[data.predecessor - 1],
        hostname,
        peer_list[data.successor - 1]
    );*/

    if args.token {
        // send the first token
        data.pass_token(&mut outgoing_channels)?;
    }

    let token_delay = Duration::from_secs_f64(args.token_delay.unwrap_or(0.0));
    let marker_delay = Duration::from_secs_f64(args.marker_delay.unwrap_or(0.0));

    let mut last_snapshot_id = 0;
    let mut concluded_processes = HashSet::new();
    let mut i_am_initiator = false;
    loop {
        // check if we can initiate a snapshot
        if let (Some(snapshot_id), Some(activate_state)) = (args.snapshot_id, args.snapshot_delay) {
            if last_snapshot_id == snapshot_id - 1 && data.state == activate_state {
                println!(
                    "{{proc_id: {}, snapshot_id: {}, snapshot: \"started\"}}",
                    data.id, snapshot_id
                );
                last_snapshot_id = snapshot_id;
                i_am_initiator = true;
                let all_closed = data.propagate_snapshot(&mut outgoing_channels, snapshot_id)?;
                assert!(!all_closed);
                continue;
            }
        }

        if concluded_processes.len() == poll_fds.len() {
            println!("{{proc_id: {}, snapshot:\"complete\"}}", data.id);
            concluded_processes.clear();
            i_am_initiator = false;
            data.notify_reset(&mut outgoing_channels)?;
        }

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
                Message::Marker {
                    from: _,
                    snapshot_id,
                } => {
                    last_snapshot_id = last_snapshot_id.max(snapshot_id);
                    sleep(marker_delay);
                    let all_closed =
                        data.propagate_snapshot(&mut outgoing_channels, snapshot_id)?;
                    if all_closed {
                        concluded_processes.insert(data.id);
                    }
                }
                Message::AllChannelsClosed { from } => {
                    if i_am_initiator {
                        concluded_processes.insert(from);
                    }
                }
                Message::ResetSnapshot => {
                    concluded_processes.clear();
                }
            }
        }
    }
}
