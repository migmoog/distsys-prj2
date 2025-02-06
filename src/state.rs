use serde::{Deserialize, Serialize};
use std::{fmt::Display, io::prelude::*};

use crate::failures::Reasons;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Message {
    Token,
    Marker { snapshot_id: u32 },
}

#[derive(Debug)]
pub struct Data {
    pub id: usize,
    pub state: u32,
    pub predecessor: usize,
    pub successor: usize,
    pub has_token: bool,
    pub has_marker: bool,
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
            has_marker: false,
        })
    }

    pub fn recv_message(&mut self, msg: Message) {
        match msg {
            Message::Token => {
                self.has_token = true;
                self.state += 1;
                println!(
                    "{{id: {}, state: {}, sender: {}, receiver: {}, message: {:?}}}",
                    self.id, self.state, self.predecessor, self.successor, msg
                );
            }
            Message::Marker { snapshot_id } => todo!(),
        }
    }

    pub fn send_message(&mut self, sender: &mut impl Write, msg: Message) -> Result<(), Reasons> {
        match msg {
            Message::Token => {
                self.has_token = false;
            }
            Message::Marker { snapshot_id } => todo!(),
        }
        let encoded_buffer = bincode::serialize(&msg).unwrap();
        sender.write_all(&encoded_buffer[..]).map_err(Reasons::IO)?;
        Ok(())
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
