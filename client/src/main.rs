use color_eyre::eyre::{anyhow, Result};
use common::{
    protocol::{Command, Frame},
    Connection, WS_URL,
};
use std::{env, net::ToSocketAddrs};
use tokio::{
    net::TcpStream,
    time::{Duration, Instant},
};
mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = cli::parse();
    let ws_url = args
        .url
        .unwrap_or(env::var("TEMPO_SERVER_URL").unwrap_or(WS_URL.to_string()));

    let ws_url = ws_url.to_socket_addrs()?.collect::<Vec<_>>()[0];
    println!("Connecting to {}", ws_url);
    let mut client = connect(ws_url).await?;

    match args.command {
        cli::SubCommand::New { note } => {
            client.create_note(&note).await?;
        }
        cli::SubCommand::List => {
            let notes = client.read_notes().await?;
            println!("Notes:");
            for note in notes {
                println!("- {}", note);
            }
        }
    }
    client.disconnect().await?;
    Ok(())
}

async fn connect<T: tokio::net::ToSocketAddrs>(addr: T) -> Result<Client> {
    let socket = tokio::time::timeout(Duration::from_secs(30), TcpStream::connect(addr)).await??;
    let connection = Connection::new(socket);
    Client::new(connection).await
}

#[derive(Debug)]
pub struct Client {
    connection: Connection,
    id: u64,
}

impl Client {
    async fn new(mut connection: Connection) -> Result<Self> {
        let start = Instant::now();
        let timeout = start + Duration::from_secs(30);
        let command = tokio::time::timeout_at(timeout, async {
            println!("Waiting for id...");
            let Frame(command) = loop {
                if let Some(frame) = connection.read_frame().await.expect("connection closed") {
                    break frame;
                }
                let time_left = timeout - Instant::now();
                if time_left <= Duration::from_secs(5) {
                    println!("waiting for id ({time_left:.0?} left)...");
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            };
            command
        })
        .await
        .expect("connection timeout: no id received");
        match command {
            Command::Id(id) => {
                println!("Connected, id: {}", id);
                Ok(Self { connection, id })
            }
            c => Err(anyhow!(
                "unexpected command type: {} (expected {})",
                c.to_string(),
                Command::Id(0).to_string()
            )),
        }
    }
    async fn create_note(&mut self, body: &str) -> Result<()> {
        let body = body.trim().to_string() + "\r\n";
        self.connection
            .write_frame(&Command::Create(body).into())
            .await?;

        Ok(())
    }

    async fn read_notes(&mut self) -> Result<Vec<String>> {
        self.connection.write_frame(&Command::Read.into()).await?;
        let Frame(command) = self
            .connection
            .read_frame()
            .await?
            .expect("connection closed early");
        match command {
            Command::List(notes) => Ok(notes),
            c => Err(anyhow!("unexpected command type: {}", c.to_string())),
        }
    }

    async fn _quit(&mut self) -> Result<()> {
        self.connection.write_frame(&Command::Quit.into()).await?;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.connection
            .write_frame(&Command::Disconnect(self.id).into())
            .await?;
        Ok(())
    }
}
