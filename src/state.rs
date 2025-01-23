use std::fmt::Display;

use crate::failures::Reasons;

pub struct Data {
    id: u32,
    state: u32,
    predecessor: u32,
    successor: u32,
}

impl Data {
    pub fn from_list(hostname: &str, peer_list: &Vec<String>, state: u32) -> Result<Self, Reasons> {
        // Pattern Matching + Iter chain hell
        // I love rust :-)
        let Some((id, _)) = peer_list
            .iter()
            .enumerate()
            .find(|(_, pname)| **pname == hostname)
        else {
            return Err(Reasons::HostNotInHostsfile);
        };

        let predecessor = if id == 0 { peer_list.len() - 1 } else { id - 1 };
        let successor = if id == peer_list.len() - 1 { 0 } else { id + 1 };

        // 1 based counting 🤷‍♂️
        let (predecessor, id, successor) =
            (predecessor as u32 + 1, id as u32 + 1, successor as u32 + 1);

        Ok(Data {
            id,
            state,
            predecessor,
            successor,
        })
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
