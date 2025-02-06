use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author = "Jeremy Gordon <jeremygordon.dev>")]
pub struct PassToken {
    #[arg(short = 'x')]
    pub token: bool,

    #[arg(short = 'h')]
    pub hostsfile: PathBuf,

    #[arg(short = 'm')]
    pub marker_delay: Option<f64>,

    #[arg(short = 's')]
    pub snapshot_delay: Option<u32>,

    #[arg(short = 'p', requires = "snapshot_delay")]
    pub snapshot_id: Option<u32>,

    #[arg(short = 't')]
    pub token_delay: Option<f64>,
}
