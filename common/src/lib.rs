use bytes::{Buf, BytesMut};
use color_eyre::eyre::{anyhow, Result};
use protocol::*;
use std::io::Cursor;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{Duration, Instant},
};
pub mod protocol;

pub type NoteID = u64;
pub type ClientID = u64;
#[derive(Debug, Clone)]
pub struct Note {
    id: NoteID,
    body: String,
    pub created_at: Instant,
}
impl Note {
    pub fn new(id: NoteID, body: String) -> Self {
        Self {
            id,
            body,
            created_at: Instant::now(),
        }
    }
    pub fn id(&self) -> NoteID {
        self.id
    }
    pub fn elapsed(&self) -> Duration {
        self.created_at.elapsed()
    }
    pub fn body(&self) -> &str {
        &self.body
    }
}

pub const NOTE_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_PORT: &str = "7536";
pub const DEFAULT_ADDRESS: &str = "127.0.0.1";
pub const WS_URL: &str = "127.0.0.1:7536";

#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(1024),
        }
    }

    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame));
            }
            let bytes_read = self.stream.read_buf(&mut self.buffer).await?;
            if 0 == bytes_read {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    return Err(anyhow!("connection reset by peer"));
                };
            }
        }
    }

    pub async fn write_frame(&mut self, frame: &Frame) -> Result<()> {
        match frame.0 {
            Command::Create(ref body) => {
                let command = &[CREATE_BYTE];
                let body = body.as_bytes();
                self.stream.write_all(&[command, body].concat()).await?
            }
            Command::List(ref notes) => {
                let msg = notes.iter().fold(String::new(), |f, note| {
                    f + note.len().to_string().as_str() + "#" + note
                });
                let frame_arg = format!("{msg}\r\n");
                let body = frame_arg.as_bytes();
                let command = &[LIST_BYTE];
                self.stream.write_all(&[command, body].concat()).await?
            }
            Command::Read => self.stream.write_all(&[READ_BYTE]).await?,
            Command::Quit => self.stream.write_all(&[QUIT_BYTE]).await?,
            Command::Disconnect(id) => {
                let command = &[DISCONNECT_BYTE];
                let body = id.to_string();
                let body = body.as_bytes();
                let sep = b"\r\n";
                self.stream
                    .write_all(&[command, body, sep].concat())
                    .await?
            }
            Command::Id(id) => {
                let command = &[ID_BYTE];
                let body = id.to_string();
                let body = body.as_bytes();
                let sep = b"\r\n";
                self.stream
                    .write_all(&[command, body, sep].concat())
                    .await?
            }
        }
        Ok(())
    }

    pub fn parse_frame(&mut self) -> Result<Option<Frame>> {
        let mut buf = Cursor::new(&self.buffer[..]);

        match Frame::check(&mut buf) {
            Ok(_) => {
                let len = buf.position() as usize;
                buf.set_position(0);
                let frame = Frame::parse(&mut buf)?;
                self.buffer.advance(len);
                Ok(Some(frame))
            }
            Err(FrameParseError::Incomplete) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// Find a line
fn get_line<'a>(src: &mut Cursor<&'a [u8]>) -> Result<&'a [u8], FrameParseError> {
    // Scan the bytes directly
    let start = src.position() as usize;
    // Scan to the second to last byte
    let end = src.get_ref().len() - 1;

    for i in start..end {
        if src.get_ref()[i] == b'\r' && src.get_ref()[i + 1] == b'\n' {
            // We found a line, update the position to be *after* the \n
            src.set_position((i + 2) as u64);

            // Return the line
            return Ok(&src.get_ref()[start..i]);
        }
    }

    Err(FrameParseError::Incomplete)
}

fn get_u8(src: &mut Cursor<&[u8]>) -> Result<u8, FrameParseError> {
    if !src.has_remaining() {
        return Err(FrameParseError::Incomplete);
    }

    Ok(src.get_u8())
}

#[derive(Error, Debug)]
pub enum FrameParseError {
    #[error("incomplete frame")]
    Incomplete,
    #[error("invalid frame start byte: {0:?}")]
    Invalid(u8),
}
