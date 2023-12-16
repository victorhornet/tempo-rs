use clap::Parser;
use common::DEFAULT_PORT;

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(short, long, default_value = DEFAULT_PORT)]
    pub port: u16,
}
pub fn parse() -> Args {
    Args::parse()
}
