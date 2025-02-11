use clap::Parser;
use socketry::establish_connection;
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
    let (before, after) = (data.predecessor - 1, data.successor - 1);
    println!("{}", data);

    let (mut listener, mut sender) = establish_connection(&hostname, &peer_list[after])?;
    // TODO: Cristina said chandy lamport has connections to EVERY socket (ex: in a 3 process ring,
    // p1 connects to p2-p4 and viceversa)

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

    let mut started_snapshots = 0;
    loop {
        // send marker if ready
        if let (Some(activate_state), Some(snapshot_id)) = (args.snapshot_delay, args.snapshot_id) {
            if started_snapshots + 1 == snapshot_id && data.state == activate_state {
                // snapshot has initiated
                println!("{{id: {}, snapshot:\"started\"}}", data.id);
                data.send_message(&mut sender, Message::Marker { snapshot_id })?;
                started_snapshots += 1;
                continue;
            }
        }

        let mut buffer = [0; 1024];
        let bytes_read = tok.read(&mut buffer[..]).map_err(Reasons::IO)?;
        let received =
            bincode::deserialize(&buffer[..bytes_read]).map_err(|_| Reasons::BadMessage)?;

        data.recv_message(received);
        match received {
            Message::Token => sleep(token_delay),
            Message::Marker { snapshot_id } => {
                if let Some(own_id) = args.snapshot_id {
                    if own_id == snapshot_id {
                        println!("{{id: {}, snapshot:\"completed\"}}", data.id);
                        data.send_message(&mut sender, Message::ResetSnapshot)?;
                        continue;
                    }
                }

                sleep(marker_delay);
            }
            Message::ResetSnapshot => {}
        }
        data.send_message(&mut sender, received)?;
    }
}
