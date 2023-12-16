use color_eyre::eyre::{anyhow, Result};
use std::io::Cursor;

use crate::{get_line, get_u8, ClientID, FrameParseError};

pub const CREATE_BYTE: u8 = b'+';
pub const CREATE_COMMAND: &str = "CREATE";
pub const READ_BYTE: u8 = b'$';
pub const READ_COMMAND: &str = "READ";
pub const QUIT_BYTE: u8 = b'-';
pub const QUIT_COMMAND: &str = "QUIT";
pub const LIST_BYTE: u8 = b'%';
pub const LIST_COMMAND: &str = "LIST";
pub const DISCONNECT_BYTE: u8 = b'!';
pub const DISCONNECT_COMMAND: &str = "DISCONNECT";
pub const ID_BYTE: u8 = b'#';
pub const ID_COMMAND: &str = "ID";

#[derive(Debug)]
pub enum Command {
    Create(String),
    List(Vec<String>),
    Id(ClientID),
    Disconnect(ClientID),
    Read,
    Quit,
}

impl Command {
    pub fn byte(&self) -> u8 {
        match self {
            Command::Create(_) => CREATE_BYTE,
            Command::List(_) => LIST_BYTE,
            Command::Read => READ_BYTE,
            Command::Quit => QUIT_BYTE,
            Command::Disconnect(_) => DISCONNECT_BYTE,
            Command::Id(_) => ID_BYTE,
        }
    }
}

impl ToString for Command {
    fn to_string(&self) -> String {
        match self {
            Command::Create(_) => CREATE_COMMAND.to_string(),
            Command::List(_) => LIST_COMMAND.to_string(),
            Command::Read => READ_COMMAND.to_string(),
            Command::Quit => QUIT_COMMAND.to_string(),
            Command::Disconnect(_) => DISCONNECT_COMMAND.to_string(),
            Command::Id(_) => ID_COMMAND.to_string(),
        }
    }
}
impl From<Command> for u8 {
    fn from(command: Command) -> Self {
        command.byte()
    }
}
impl From<&Command> for u8 {
    fn from(command: &Command) -> Self {
        command.byte()
    }
}
impl From<Command> for Frame {
    fn from(val: Command) -> Self {
        Frame(val)
    }
}
impl From<Frame> for Command {
    fn from(frame: Frame) -> Self {
        frame.0
    }
}
impl From<u8> for Command {
    fn from(byte: u8) -> Self {
        match byte {
            CREATE_BYTE => Command::Create(String::new()),
            LIST_BYTE => Command::List(Vec::new()),
            READ_BYTE => Command::Read,
            QUIT_BYTE => Command::Quit,
            DISCONNECT_BYTE => Command::Disconnect(0),
            ID_BYTE => Command::Id(0),
            _ => panic!("invalid command"),
        }
    }
}

#[derive(Debug)]
pub struct Frame(pub Command);
impl Frame {
    pub fn check(src: &mut Cursor<&[u8]>) -> Result<(), FrameParseError> {
        match get_u8(src)? {
            CREATE_BYTE => {
                get_line(src)?;
                Ok(())
            }
            LIST_BYTE => {
                get_line(src)?;
                Ok(())
            }
            READ_BYTE => Ok(()),
            QUIT_BYTE => Ok(()),
            DISCONNECT_BYTE => {
                get_line(src)?;
                Ok(())
            }
            ID_BYTE => {
                get_line(src)?;
                Ok(())
            }
            other => Err(FrameParseError::Invalid(other)),
        }
    }
    pub fn parse(src: &mut Cursor<&[u8]>) -> Result<Frame> {
        match get_u8(src)? {
            CREATE_BYTE => {
                let line = get_line(src)?.to_vec();
                Ok(Command::Create(String::from_utf8(line)?).into())
            }
            LIST_BYTE => {
                let line = get_line(src)?.to_vec();
                let encoded_notes = String::from_utf8(line)?;
                let mut notes = Vec::new();
                let mut chars = encoded_notes.chars();
                let mut len = String::new();
                while let Some(ch) = chars.next() {
                    match ch {
                        '#' => {
                            let note_size = len.parse::<usize>()?;
                            let mut note = String::new();
                            for _ in 0..note_size {
                                let c = chars.next().ok_or(anyhow!("invalid frame"))?;
                                note.push(c);
                            }
                            notes.push(note);
                            len.clear();
                        }
                        c if c.is_ascii_digit() => len.push(c),
                        _ => return Err(anyhow!("invalid frame")),
                    }
                }
                Ok(Command::List(notes).into())
            }
            READ_BYTE => Ok(Command::Read.into()),
            QUIT_BYTE => Ok(Command::Quit.into()),
            DISCONNECT_BYTE => {
                let id = get_line(src)?;
                let id = String::from_utf8(id.to_vec())?;
                let id = id.parse::<u64>()?;
                Ok(Command::Disconnect(id).into())
            }
            ID_BYTE => {
                let id = get_line(src)?;
                let id = String::from_utf8(id.to_vec())?;
                let id = id.parse::<u64>()?;
                Ok(Command::Id(id).into())
            }
            other => Err(FrameParseError::Invalid(other).into()),
        }
    }
}
