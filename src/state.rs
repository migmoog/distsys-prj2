use std::fmt::Display;

use crate::failures::Reasons;

#[derive(Debug)]
pub struct Data {
    id: usize,
    state: u32,
    predecessor: usize,
    pub successor: usize,
}

impl Data {
    pub fn from_list(
        hostname: &str,
        peer_list: &Vec<String>,
        state: u32,
    ) -> Result<(Self, usize, usize), Reasons> {
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

        Ok((
            Data {
                id,
                state,
                predecessor,
                successor,
            },
            predecessor - 1,
            successor - 1,
        ))
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
