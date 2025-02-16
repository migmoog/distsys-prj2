use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    io::prelude::*,
    net::TcpStream,
};

use crate::failures::Reasons;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Message {
    Token,
    Marker { from: usize, snapshot_id: u32 },
}

#[derive(Debug)]
pub struct Data {
    // PassToken Data
    pub id: usize,
    pub state: u32,
    pub predecessor: usize,
    pub successor: usize,

    // Chandy Lamport Data
    pub desired_snapshot: u32, // the id of ongoing snapshot
    has_token: bool,
    pub seen_marker: bool, // CL has weird rules involving "seeing markers" (check example on canvas)
    closed_channels: HashSet<usize>, // IDs of closed channels (ex: C_{elem}_{self.id})
    records: Vec<u32>,     // previous states recorded
    channel_values: Vec<Message>, // values recorded from an incoming channel (only tokens)
}

type Channels = HashMap<usize, TcpStream>;

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

            desired_snapshot: 1,
            has_token: false,
            seen_marker: false,
            closed_channels: HashSet::new(),
            records: Vec::new(),
            channel_values: Vec::new(),
        })
    }

    pub fn initiate_snapshot(
        &mut self,
        outgoing_channels: &mut Channels,
        snapshot_id: u32,
    ) -> Result<(), Reasons> {
        assert!(!self.seen_marker);
        self.seen_marker = true;
        self.records.push(self.state);

        for (&channel_id, channel) in outgoing_channels.iter_mut() {
            self.send_message(
                channel,
                Message::Marker {
                    from: self.id,
                    snapshot_id,
                },
                channel_id,
            )?;
        }
        println!(
            "{{proc_id: {}, snapshot_id: {}, snapshot: \"started\"}}",
            self.id, snapshot_id
        );
        Ok(())
    }

    pub fn propagate_snapshot(
        &mut self,
        outgoing_channels: &mut Channels,
        snapshot_id: u32,
    ) -> Result<(), Reasons> {
        if self.seen_marker || snapshot_id != self.desired_snapshot {
            return Ok(());
        }

        self.seen_marker = true;
        println!(
            "{{proc_id: {}, snapshot_id: {}, snapshot: \"started\"}}",
            self.id, snapshot_id
        );
        for (&channel_id, channel) in outgoing_channels.iter_mut() {
            self.send_message(
                channel,
                Message::Marker {
                    from: self.id,
                    snapshot_id,
                },
                channel_id,
            )?;
        }
        Ok(())
    }

    fn reset_snapshot(&mut self) {
        self.seen_marker = false;
        self.channel_values.clear();
        self.closed_channels.clear();
        self.records.clear();
        self.desired_snapshot += 1;
    }

    pub fn recv_message(&mut self, msg: Message, channel_count: usize) {
        match msg {
            Message::Token => {
                if self.seen_marker && !self.closed_channels.contains(&self.predecessor) {
                    self.channel_values.push(Message::Token);
                }

                self.has_token = true;
                self.state += 1;
                println!("{{proc_id: {}, state: {}}}", self.id, self.state)
            }

            Message::Marker { from, snapshot_id } => {
                self.closed_channels.insert(from);
                println!(
                    "{{proc_id: {}, snapshot_id: {}, snapshot: \"channel closed\", channel: {}-{}, queue: {:?}}}",
                    self.id, snapshot_id, from, self.id, self.channel_values
                );

                if self.closed_channels.len() == channel_count {
                    println!(
                        "{{proc_id: {}, snapshot_id: {}, snapshot: \"complete\"}}",
                        self.id, snapshot_id
                    );

                    self.reset_snapshot();
                }
            }
        }
    }

    fn send_message(
        &mut self,
        sender: &mut impl Write,
        msg: Message,
        channel_id: usize,
    ) -> Result<(), Reasons> {
        match msg {
            Message::Token => {
                println!(
                    "{{proc_id: {}, state: {}, sender: {}, receiver: {}, message: \"{:?}\"}}",
                    self.id, self.state, self.predecessor, channel_id, msg
                );
                self.has_token = false;
            }
            _ => {}
        }
        let encoded_buffer = bincode::serialize(&msg).map_err(|_| Reasons::BadMessage)?;
        sender.write_all(&encoded_buffer).map_err(Reasons::IO)?;
        Ok(())
    }

    fn send_to_channel(
        &mut self,
        outgoing_channels: &mut Channels,
        channel_id: usize,
        msg: Message,
    ) -> Result<(), Reasons> {
        self.send_message(
            outgoing_channels.get_mut(&channel_id).unwrap(),
            msg,
            channel_id,
        )
    }

    // Passes the token to the successive process
    pub fn pass_token(&mut self, outgoing_channels: &mut Channels) -> Result<(), Reasons> {
        self.send_to_channel(outgoing_channels, self.successor, Message::Token)
    }
}

impl Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{proc_id: {}, state:{}, predecessor:{}, successor:{}}}",
            self.id, self.state, self.predecessor, self.successor
        )
    }
}
