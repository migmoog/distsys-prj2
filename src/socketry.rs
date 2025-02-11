use std::{
    net::{TcpListener, TcpStream},
    thread::sleep,
    time::Duration,
};

use crate::failures::Reasons;

const PORT: &'static str = "6969";
const MAX_ATTEMPTS: i32 = 10;
const ATTEMPT_WAIT: Duration = Duration::from_secs(5);

pub fn establish_connection(
    to_listen: &str,
    to_send: &str,
) -> Result<(TcpListener, TcpStream), Reasons> {
    let (mut listen_attempts, mut sender_attempts) = (0, 0);
    let (listener_address, sendable_address) = (
        format!("{}:{}", to_listen, PORT),
        format!("{}:{}", to_send, PORT),
    );

    let listener = loop {
        match TcpListener::bind(&listener_address) {
            Ok(l) => break l,
            Err(e) => {
                if listen_attempts == MAX_ATTEMPTS {
                    return Err(Reasons::IO(e));
                }
                listen_attempts += 1;
                sleep(ATTEMPT_WAIT);
            }
        }
    };
    let sender = loop {
        match TcpStream::connect(&sendable_address) {
            Ok(s) => break s,
            Err(e) => {
                if sender_attempts == MAX_ATTEMPTS {
                    return Err(Reasons::IO(e));
                }
                sender_attempts += 1;
                sleep(ATTEMPT_WAIT);
            }
        }
    };
    Ok((listener, sender))
}

// creates a vector of listening and sending sockets
// for each process. (connects like a spiderweb across the ring)
// NOTE: will skip over the hostname if it's found in the list
fn make_channels(
    hostname: &str,
    peer_list: &[String],
) -> Result<Vec<(TcpListener, TcpStream)>, Reasons> {
    let mut out = Vec::new();
    for peer_name in peer_list {
        if hostname == peer_name {
            continue;
        }

        let channel_sockets = establish_connection(hostname, peer_name)?;
        out.push(channel_sockets);
    }
    Ok(out)
}
