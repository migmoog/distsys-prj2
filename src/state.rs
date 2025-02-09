use serde::{Deserialize, Serialize};
use std::{fmt::Display, io::prelude::*};

use crate::failures::Reasons;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Message {
    Token,
    Marker { snapshot_id: u32 },
    ResetSnapshot, // done when a snapshot is considered complete
}

#[derive(Debug)]
pub struct Data {
    // PassToken Data
    pub id: usize,
    pub state: u32,
    pub predecessor: usize,
    pub successor: usize,

    // Chandy Lamport Data
    seen_marker: bool,
    has_token: bool,
    records: Vec<u32>,            // previous states recorded
    channel_values: Vec<Message>, // values recorded from an incoming channel (only tokens)
    recording_incoming: bool,
}

impl Data {
    pub fn from_list(hostname: &str, peer_list: &Vec<String>, state: u32) -> Result<Self, Reasons> {
        // Pattern Matching + Iter chain hell
        // I love rust :-)
        let Some(id) = peer_list.iter().position(|pname| *pname == hostname) else {
            return Err(Reasons::HostNotInHostsfile);
        };

        let l = peer_list.len() - 1;
        let predecessor = if id == 0 { l } else { id - 1 };
        let successor = if id == l { 0 } else { id + 1 };

        // 1 based counting 🤷‍♂️
        let (predecessor, id, successor) = (predecessor + 1, id + 1, successor + 1);

        Ok(Data {
            id,
            state,
            predecessor,
            successor,
            seen_marker: false,
            has_token: false,
            records: Vec::new(),
            channel_values: Vec::new(),
            recording_incoming: false,
        })
    }

    pub fn recv_message(&mut self, msg: Message) {
        match msg {
            Message::Token => {
                if self.recording_incoming {
                    self.channel_values.push(Message::Token);
                }
                self.has_token = true;
                self.state += 1;
                println!("{{id: {}, state: {}}}", self.id, self.state)
            }
            Message::Marker { snapshot_id: _ } => {
                if !self.seen_marker {
                    self.recording_incoming = false;
                    self.seen_marker = true;

                    // incoming channel is now considered closed
                    println!(
                        "{{id: {}, snapshot:\"channel closed\", channel:{}-{}, queue:{:?}}}",
                        self.id, self.predecessor, self.id, self.channel_values
                    );
                }
            }
            Message::ResetSnapshot => {
                self.snapshot_reset();
                println!("id: {} is reset", self.id);
            }
        }
    }

    pub fn send_message(&mut self, sender: &mut impl Write, msg: Message) -> Result<(), Reasons> {
        let encoded_buffer = bincode::serialize(&msg).unwrap();
        match msg {
            Message::Token => {
                println!(
                    "{{id: {}, state: {}, sender: {}, receiver: {}, message: {:?}}}",
                    self.id, self.state, self.predecessor, self.successor, msg
                );
                self.has_token = false;
            }
            Message::Marker { snapshot_id } => {
                self.records.push(self.state);
                self.recording_incoming = true;

                println!(
                    "{{id: {}, sender: {}, receiver: {}, msg: {:?}, state: {}, has_token:{}}}",
                    self.id,
                    self.predecessor,
                    self.successor,
                    Message::Marker { snapshot_id },
                    self.state,
                    self.has_token
                );
            }
            Message::ResetSnapshot => println!("Telling {} to reset", self.successor),
        }
        sender.write_all(&encoded_buffer[..]).map_err(Reasons::IO)?;
        Ok(())
    }

    // restore default state of snapshot data
    pub fn snapshot_reset(&mut self) {
        self.records.clear();
        self.channel_values.clear();
        self.seen_marker = false;
    }
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{id:{}, state:{}, predecessor:{}, successor:{}}}",
            self.id, self.state, self.predecessor, self.successor
        )
    }
}
