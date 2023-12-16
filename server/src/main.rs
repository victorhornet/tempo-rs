use color_eyre::eyre::Result;
use server::NotesServer;
use tokio::net::TcpListener;
mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = cli::parse();
    let mut notes_server = NotesServer::default();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    println!("Listening at {}", listener.local_addr()?);
    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Accepted client: {}", addr);
        match notes_server.handle_connection(socket).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error: {}", e);
                continue;
            }
        }
    }
}
