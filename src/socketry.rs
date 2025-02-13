use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream},
    thread::sleep,
    time::Duration,
};

use crate::failures::Reasons;

const PORT: &'static str = "6969";
const MAX_ATTEMPTS: i32 = 10;
const ATTEMPT_WAIT: Duration = Duration::from_secs(5);

pub fn connect_channel(to_send: &str) -> Result<TcpStream, Reasons> {
    let mut sender_attempts = 0;
    let sendable_address = format!("{}:{}", to_send, PORT);

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
    Ok(sender)
}

pub fn bind_listener(hostname: &str) -> Result<TcpListener, Reasons> {
    let mut sender_attempts = 0;
    let sendable_address = format!("{}:{}", hostname, PORT);

    let sender = loop {
        match TcpListener::bind(&sendable_address) {
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
    Ok(sender)
}

// creates a vector of listening and sending sockets
// for each process. (connects like a spiderweb across the ring)
// NOTE: will skip over the hostname if it's found in the list
pub fn make_channels(
    hostname: &str,
    peer_list: &[String],
) -> Result<HashMap<usize, TcpStream>, Reasons> {
    let mut out = HashMap::new();
    for (idx, peer_name) in peer_list.into_iter().enumerate() {
        if hostname == peer_name {
            continue;
        }

        let channel_sockets = connect_channel(peer_name)?;
        out.insert(idx + 1, channel_sockets);
    }
    Ok(out)
}
