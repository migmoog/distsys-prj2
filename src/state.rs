use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, io::prelude::*, net::TcpStream};

use crate::failures::Reasons;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Message {
    Token,
    Marker { from: usize, snapshot_id: u32 },
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
    has_token: bool,
    records: Vec<u32>,            // previous states recorded
    channel_values: Vec<Message>, // values recorded from an incoming channel (only tokens)
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
            has_token: false,
            records: Vec::new(),
            channel_values: Vec::new(),
        })
    }

    pub fn recv_message(&mut self, msg: Message) {
        match msg {
            Message::Token => {
                self.has_token = true;
                self.state += 1;
                println!("{{id: {}, state: {}}}", self.id, self.state)
            }
            Message::Marker {
                snapshot_id: _,
                from,
            } => {}
            Message::ResetSnapshot => {
                self.snapshot_reset();
                println!("id: {} is reset", self.id);
            }
        }
    }

    fn send_message(&mut self, sender: &mut impl Write, msg: Message) -> Result<(), Reasons> {
        let encoded_buffer = bincode::serialize(&msg).unwrap();
        match msg {
            Message::Token => {
                println!(
                    "{{id: {}, state: {}, sender: {}, receiver: {}, message: {:?}}}",
                    self.id, self.state, self.predecessor, self.successor, msg
                );
                self.has_token = false;
            }
            Message::Marker { snapshot_id, from } => {
                self.records.push(self.state);

                println!(
                    "{{id: {}, sender: {}, receiver: {}, msg: {:?}, state: {}, has_token:{}}}",
                    self.id,
                    self.predecessor,
                    self.successor,
                    Message::Marker { snapshot_id, from },
                    self.state,
                    self.has_token
                );
            }
            Message::ResetSnapshot => println!("Telling {} to reset", self.successor),
        }
        sender.write_all(&encoded_buffer[..]).map_err(Reasons::IO)?;
        Ok(())
    }

    fn send_to_channel(
        &mut self,
        outgoing_channels: &mut HashMap<usize, TcpStream>,
        channel_id: usize,
        msg: Message,
    ) -> Result<(), Reasons> {
        self.send_message(outgoing_channels.get_mut(&channel_id).unwrap(), msg)
    }

    pub fn pass_token(
        &mut self,
        outgoing_channels: &mut HashMap<usize, TcpStream>,
    ) -> Result<(), Reasons> {
        self.send_to_channel(outgoing_channels, self.successor, Message::Token)
    }

    // restore default state of snapshot data
    pub fn snapshot_reset(&mut self) {
        self.records.clear();
        self.channel_values.clear();
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
