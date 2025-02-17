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
pub struct ChandyLamport {
    pub seen_marker: bool, // CL has weird rules involving "seeing markers" (check example on canvas)
    closed_channels: HashSet<usize>, // IDs of closed channels (ex: C_{elem}_{self.id})
    records: Vec<u32>,     // previous states recorded
    channel_values: Vec<Message>, // values recorded from an incoming channel (only tokens)
}
impl Default for ChandyLamport {
    fn default() -> Self {
        Self {
            seen_marker: false,
            closed_channels: HashSet::new(),
            records: Vec::new(),
            channel_values: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Data {
    // PassToken Data
    pub id: usize,
    pub state: u32,
    pub predecessor: usize,
    pub successor: usize,
    has_token: bool,

    pub snapshots: HashMap<u32, ChandyLamport>,
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
            has_token: false,
            successor,
            snapshots: HashMap::new(),
        })
    }

    pub fn initiate_snapshot(
        &mut self,
        outgoing_channels: &mut Channels,
        snapshot_id: u32,
    ) -> Result<(), Reasons> {
        assert!(self.snapshots.get(&snapshot_id).is_none());
        self.snapshots.insert(snapshot_id, ChandyLamport::default());

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
        eprintln!(
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
        let snapshot = self.snapshots.get_mut(&snapshot_id).unwrap();
        if snapshot.seen_marker {
            return Ok(());
        }
        snapshot.seen_marker = true;
        eprintln!(
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

    pub fn recv_message(&mut self, msg: Message, channel_count: usize) {
        match msg {
            Message::Token => {
                for (_snapshot_id, snapshot) in self.snapshots.iter_mut() {
                    if snapshot.seen_marker && !snapshot.closed_channels.contains(&self.predecessor)
                    {
                        snapshot.channel_values.push(Message::Token);
                    }
                }

                self.has_token = true;
                self.state += 1;
                eprintln!("{{proc_id: {}, state: {}}}", self.id, self.state)
            }

            Message::Marker { from, snapshot_id } => {
                let snapshot = self
                    .snapshots
                    .entry(snapshot_id)
                    .or_insert_with(ChandyLamport::default);
                snapshot.closed_channels.insert(from);
                eprintln!(
                    "{{proc_id: {}, snapshot_id: {}, snapshot: \"channel closed\", channel: {}-{}, queue: {:?}}}",
                    self.id, snapshot_id, from, self.id, snapshot.channel_values
                );

                if snapshot.closed_channels.len() == channel_count {
                    eprintln!(
                        "{{proc_id: {}, snapshot_id: {}, snapshot: \"complete\"}}",
                        self.id, snapshot_id
                    );
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
                eprintln!(
                    "{{proc_id: {}, state: {}, sender: {}, receiver: {}, message: \"{:?}\"}}",
                    self.id, self.state, self.predecessor, channel_id, msg
                );
                self.has_token = false;
            }
            Message::Marker { snapshot_id, from } => {
                eprintln!("{{proc_id: {}, snapshot_id: {}, sender: {}, receiver: {}, msg: {:?}, state: {}, has_token: {}}}",
                self.id,
                snapshot_id,
                from,
                channel_id,
                Message::Marker { from, snapshot_id },
                self.state,
                if self.has_token { "YES" } else { "NO" });
            }
        }
        let encoded_buffer = bincode::serialize(&msg).map_err(|_| Reasons::BadMessage)?;
        sender.write_all(&encoded_buffer).map_err(Reasons::IO)?;
        Ok(())
    }

    // Passes the token to the successive process
    pub fn pass_token(&mut self, outgoing_channels: &mut Channels) -> Result<(), Reasons> {
        self.send_message(
            outgoing_channels.get_mut(&self.successor).unwrap(),
            Message::Token,
            self.successor,
        )
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
