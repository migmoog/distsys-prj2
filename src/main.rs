use clap::Parser;
use std::{
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread::sleep,
    time::Duration,
};

mod failures;
mod state;

use failures::Reasons;
use state::Data;

#[derive(Parser, Debug)]
#[command(author = "Jeremy Gordon <jeremygordon.dev>")]
struct Args {
    #[arg(short = 'x')]
    token: bool,

    #[arg(short = 'h')]
    hostsfile: PathBuf,

    #[arg(short = 'm')]
    marker_delay: Option<f64>,

    #[arg(short = 's')]
    state: Option<u32>,

    #[arg(short = 't')]
    token_delay: Option<f64>,
}

const PORT: &'static str = "6969";
const MAX_ATTEMPTS: i32 = 10;
const ATTEMPT_WAIT: Duration = Duration::from_secs(5);

fn main() -> Result<(), Reasons> {
    let hostname = hostname::get()
        .map_err(Reasons::IO)?
        .into_string()
        .expect("Host device's name as a string");
    let args = Args::parse();
    let peer_list: Vec<String> = match File::open(&args.hostsfile) {
        Ok(mut f) => {
            let mut out = String::new();
            let _ = f.read_to_string(&mut out).map_err(Reasons::IO)?;
            out.lines().map(str::to_string).collect()
        }
        Err(e) => return Err(Reasons::IO(e)),
    };

    let (data, predecessor, successor) = Data::from_list(&hostname, &peer_list, 0)?;
    println!("{}", data);

    // set up a listener
    let mut attempts = 0;
    let listener_address = format!("{}:{}", hostname, PORT);
    let listener = loop {
        match TcpListener::bind(&listener_address) {
            Ok(l) => break l,
            Err(e) => {
                if attempts == MAX_ATTEMPTS {
                    return Err(Reasons::IO(e));
                }
                attempts += 1;
                sleep(ATTEMPT_WAIT);
            }
        }
    };
    //println!("Listening on {}", listener_address);
    attempts = 0;

    let sendable_address = format!("{}:{}", peer_list[successor], PORT);
    let sender = loop {
        match TcpStream::connect(&sendable_address) {
            Ok(s) => break s,
            Err(e) => {
                if attempts == MAX_ATTEMPTS {
                    return Err(Reasons::IO(e));
                }
                attempts += 1;
                sleep(ATTEMPT_WAIT);
            }
        }
    };
    //println!("{} connected to {}", hostname, sendable_address);

    println!(
        "{} -> [{}]-> {}",
        peer_list[predecessor], hostname, peer_list[successor]
    );
    loop {}

    Ok(())
}
