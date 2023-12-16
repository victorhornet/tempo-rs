use color_eyre::eyre::{anyhow, Result};
use common::{
    protocol::{Command, Frame},
    ClientID, Connection, Note, NoteID, NOTE_TIMEOUT,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::mpsc::{self, Receiver, Sender},
    sync::Arc,
};
use tokio::{net::TcpStream, sync::Mutex as AsyncMutex, task::JoinHandle};

pub struct NotesServer {
    notes: Arc<AsyncMutex<BTreeMap<NoteID, Note>>>,
    cleanup_sender: Sender<NoteID>,
    cleanup_handler: JoinHandle<()>,
    disconnect_sender: Sender<ClientID>,
    disconnect_handler: JoinHandle<()>,
    client_handlers: Arc<AsyncMutex<HashMap<ClientID, JoinHandle<Result<()>>>>>,
}

impl Default for NotesServer {
    fn default() -> Self {
        let notes = Arc::new(AsyncMutex::new(BTreeMap::new()));
        let (cleanup_sender, cleanup_receiver) = mpsc::channel::<NoteID>();
        let cleanup_handler = tokio::spawn({
            let notes = notes.clone();
            Self::cleanup(cleanup_receiver, notes)
        });
        let client_handlers = Arc::new(AsyncMutex::new(HashMap::new()));
        let (disconnect_sender, disconnect_receiver) = mpsc::channel::<ClientID>();
        let disconnect_handler = tokio::spawn({
            let client_handlers = client_handlers.clone();
            Self::handle_disconnects(disconnect_receiver, client_handlers)
        });
        Self {
            notes,
            cleanup_sender,
            cleanup_handler,
            disconnect_sender,
            disconnect_handler,
            client_handlers,
        }
    }
}

impl NotesServer {
    pub fn new() -> Self {
        Self::default()
    }

    async fn cleanup(recv: Receiver<NoteID>, notes: Arc<AsyncMutex<BTreeMap<NoteID, Note>>>) {
        while let Ok(id) = recv.recv() {
            let note = {
                notes
                    .lock()
                    .await
                    .get(&id)
                    .expect("note must exist")
                    .clone()
            };
            println!("[Cleanup] Received note: {:?}", note);
            while note.elapsed() < NOTE_TIMEOUT {
                let timeout = NOTE_TIMEOUT - note.elapsed();
                println!("Sleeping for {:?}", timeout);
                tokio::time::sleep(timeout).await;
            }
            {
                let mut notes = notes.lock().await;
                notes.remove(&id);
            }
        }
        println!("Cleanup thread finished");
    }

    async fn handle_disconnects(
        recv: Receiver<ClientID>,
        client_handlers: Arc<AsyncMutex<HashMap<ClientID, JoinHandle<Result<()>>>>>,
    ) {
        while let Ok(id) = recv.recv() {
            {
                let mut client_handlers = client_handlers.lock().await;
                client_handlers.remove(&id);
            }
        }
        println!("Cleanup thread finished");
    }

    pub async fn close(self) -> Result<()> {
        drop(self.cleanup_sender);
        let client_handlers = self.client_handlers.lock().await;
        for (_, handle) in client_handlers.iter() {
            handle.abort();
            //todo tell client to disconnect
        }
        self.cleanup_handler
            .await
            .map_err(|_| anyhow!("failed to join cleanup thread"))?;
        self.disconnect_handler
            .await
            .map_err(|_| anyhow!("failed to join disconnect thread"))?;
        Ok(())
    }

    pub async fn handle_connection(&mut self, socket: TcpStream) -> Result<()> {
        let notes_handler = self.create_handler();
        let connection = Connection::new(socket);
        {
            let mut client_handlers = self.client_handlers.lock().await;
            let id = client_handlers.len() as ClientID;
            let handle = tokio::spawn(notes_handler.run(connection, id as u64));
            client_handlers.insert(id, handle);
        }
        Ok(())
    }

    pub fn create_handler(&mut self) -> NotesHandler {
        NotesHandler::new(
            self.notes.clone(),
            self.cleanup_sender.clone(),
            self.disconnect_sender.clone(),
        )
    }
}

#[derive(Debug)]
pub struct NotesHandler {
    pub notes: Arc<AsyncMutex<BTreeMap<NoteID, Note>>>,
    cleanup_sender: Sender<NoteID>,
    disconnect_sender: Sender<ClientID>,
}

impl NotesHandler {
    pub fn new(
        notes: Arc<AsyncMutex<BTreeMap<NoteID, Note>>>,
        cleanup_sender: Sender<NoteID>,
        disconnect_sender: Sender<ClientID>,
    ) -> Self {
        Self {
            notes,
            cleanup_sender,
            disconnect_sender,
        }
    }
    pub async fn create_note(&mut self, body: &str) -> Result<NoteID> {
        let mut notes = self.notes.lock().await;
        let id = notes.keys().last().map_or(0, |k| k + 1);
        let note = Note::new(id, body.to_owned());
        notes.insert(id, note);
        self.cleanup_sender
            .send(id)
            .map_err(|_| anyhow!("Failed to send id {id} through channel."))?;
        Ok(id)
    }
    pub async fn get(&self, id: u64) -> Option<Note> {
        let notes = self.notes.lock().await;
        let note = notes.get(&id)?.to_owned();
        Some(note)
    }
    pub async fn get_all(&self) -> Vec<Note> {
        let notes = self.notes.lock().await;
        notes.values().cloned().collect()
    }

    pub async fn remove(&mut self, id: u64) -> Option<Note> {
        self.notes.lock().await.remove(&id)
    }

    async fn run(mut self, mut connection: Connection, id: u64) -> Result<()> {
        println!("Running handler for {id}");
        connection
            .write_frame(&Command::Id(id).into())
            .await
            .map_err(|_| anyhow!("failed to write id"))?;
        println!("Sent id: {}, awaiting commands", id);
        loop {
            if let Some(Frame(command)) = connection.read_frame().await? {
                println!("[Handler {id}] Received command: {:?}", command);
                match command {
                    Command::Create(body) => {
                        let body = body.as_str();
                        self.create_note(body).await?;
                    }
                    Command::Read => {
                        let notes = self.get_all().await;
                        let notes = notes.iter().map(|note| note.body().to_owned()).collect();
                        let frame = Command::List(notes).into();
                        connection.write_frame(&frame).await?;
                    }
                    Command::Disconnect(id) => {
                        self.disconnect_sender
                            .send(id)
                            .map_err(|_| anyhow!("Failed to send id {id} through channel."))?;
                        return Ok(());
                    }
                    Command::Quit => {
                        println!("Closing connection");
                        todo!();
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn close(self) -> Result<()> {
        drop(self.cleanup_sender);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_100_notes() -> Result<()> {
        let mut notes_server = NotesServer::new();
        let mut notes_handler = notes_server.create_handler();
        for _ in 0..100 {
            notes_handler.create_note("test note").await?;
        }
        notes_handler.close()?;
        notes_server.close().await?;
        Ok(())
    }
}
