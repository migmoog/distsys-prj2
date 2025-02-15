use bincode::deserialize;
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
    Marker {
        from: usize,
        snapshot_id: u32,
        initiator: usize,
    },
    AllChannelsClosed {
        from: usize,
    },
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
    seen_marker: bool, // CL has weird rules involving "seeing markers" (check example on canvas)
    closed_channels: HashSet<usize>, // IDs of closed channels (ex: C_{elem}_{self.id})
    records: Vec<u32>, // previous states recorded
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
            has_token: false,
            seen_marker: false,
            closed_channels: HashSet::new(),
            records: Vec::new(),
            channel_values: Vec::new(),
        })
    }

    pub fn recv_message(&mut self, msg: Message) {
        match msg {
            Message::Token => {
                if self.seen_marker && !self.closed_channels.contains(&self.predecessor) {
                    self.channel_values.push(Message::Token);
                }

                self.has_token = true;
                self.state += 1;
                println!("{{proc_id: {}, state: {}}}", self.id, self.state)
            }
            Message::Marker {
                snapshot_id, from, ..
            } => {
                if !self.seen_marker {
                    self.seen_marker = true;
                }

                if !self.closed_channels.contains(&from) {
                    println!("{{proc_id: {}, snapshot_id: {}, snapshot: \"channel closed\", channel:{}-{}, queue:{:?}}}",
                self.id, snapshot_id, from, self.id, self.channel_values);

                    self.closed_channels.insert(from);
                }
            }
            Message::ResetSnapshot => {
                self.reset_snapshot();
            }
            _ => {}
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
                    self.id, self.state, self.predecessor, self.successor, msg
                );
                self.has_token = false;
            }
            Message::Marker {
                snapshot_id,
                from,
                initiator,
            } => {
                if !self.seen_marker {
                    self.seen_marker = true;
                }

                if self.closed_channels.len() == 0 {
                    self.records.push(self.state);
                }

                println!(
                    "{{proc_id: {}, snapshot_id: {}, sender: {}, receiver: {}, msg: \"{:?}\", state: {}, has_token:{}}}",
                    self.id,
                    snapshot_id,
                    from,
                    channel_id,
                    Message::Marker { snapshot_id, from , initiator},
                    self.state,
                    self.has_token
                );
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

    // sends a marker to all outgoing channels
    // returns true if all the process' channels have been closed
    pub fn propagate_snapshot(
        &mut self,
        outgoing_channels: &mut Channels,
        snapshot_id: u32,
        initiator: usize,
    ) -> Result<bool, Reasons> {
        let channels_still_open = self.closed_channels.len() < outgoing_channels.len();
        if channels_still_open {
            for (&cid, channel) in outgoing_channels.iter_mut() {
                self.send_message(
                    channel,
                    Message::Marker {
                        from: self.id,
                        snapshot_id,
                        initiator,
                    },
                    cid,
                )?;
            }
        } else {
            println!(
                "proc_id: {}, all channels closed: {:?}",
                self.id, self.closed_channels
            );

            if initiator != self.id {
                self.send_to_channel(
                    outgoing_channels,
                    initiator,
                    Message::AllChannelsClosed { from: self.id },
                )?;
            }
        }

        Ok(!channels_still_open)
    }

    // restore default state of snapshot data
    pub fn reset_snapshot(&mut self) {
        self.records.clear();
        self.channel_values.clear();
        self.closed_channels.clear();
    }

    pub fn notify_reset(&mut self, outgoing_channels: &mut Channels) -> Result<(), Reasons> {
        for (&cid, channel) in outgoing_channels.iter_mut() {
            self.send_message(channel, Message::ResetSnapshot, cid)?;
        }
        Ok(())
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
