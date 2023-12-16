use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long)]
    pub url: Option<String>,
    #[arg(short, long, default_value = "false")]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: SubCommand,
}

#[derive(Subcommand, Debug)]
pub enum SubCommand {
    New { note: String },
    List,
}

pub fn parse() -> Args {
    Args::parse()
}
