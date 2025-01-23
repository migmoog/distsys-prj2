#[derive(Debug)]
pub enum Reasons {
    TCP,
    IO(std::io::Error),
    HostNotInHostsfile,
}
