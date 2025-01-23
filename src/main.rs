use clap::Parser;
use std::{
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream},
    path::PathBuf,
};

mod failures;
mod state;

use failures::Reasons;
use state::Data;

#[derive(Parser, Debug)]
#[command(author = "Jeremy Gordon <jeremygordon.dev>")]
struct Args {
    #[arg(short = 'x')]
    token: Option<String>,

    #[arg(short = 'h')]
    hostsfile: PathBuf,

    #[arg(short = 'm')]
    marker_delay: f64,

    #[arg(short = 's')]
    state: u32,

    #[arg(short = 't')]
    token_delay: f64,
}

fn main() -> Result<(), Reasons> {
    println!("Hello World!");
    let hostname = match hostname::get() {
        Ok(s) => s.into_string().expect("Host name of container"),
        Err(e) => return Err(Reasons::IO(e)),
    };

    let args = Args::parse();
    let peer_list: Vec<String> = match File::open(&args.hostsfile) {
        Ok(mut f) => {
            let mut out = String::new();
            if let Err(e) = f.read_to_string(&mut out) {
                return Err(Reasons::IO(e));
            }
            out.lines().map(|s| s.to_string()).collect()
        }
        Err(e) => return Err(Reasons::IO(e)),
    };

    let data = Data::from_list(&hostname, &peer_list, 0)?;
    println!("{data}");
    Ok(())
}
