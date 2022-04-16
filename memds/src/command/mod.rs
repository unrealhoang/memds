use std::fmt::Display;

use command_args::CommandArgs;
use command_args_derive::CommandArgsBlock;
use serde::Serialize;

use crate::server::Database;

pub trait CommandHandler {
    type Output: Serialize;
    fn handle(self, db: &Database) -> Result<Self::Output, Error>;
}

#[derive(Debug, CommandArgsBlock)]
#[argtoken("AUTH")]
pub struct HelloAuthArg<'a> {
    pub username: &'a str,
    pub password: &'a str,
}

#[derive(Debug, CommandArgsBlock)]
#[argtoken("HELLO")]
pub struct HelloCommand<'a> {
    pub protover: usize,
    pub auth: Option<HelloAuthArg<'a>>,
}

#[derive(Debug, CommandArgsBlock)]
#[argtoken("COMMAND")]
pub struct CommandCommand {
}

#[derive(Debug, Serialize)]
pub struct ServerProperties {
    server: String,
    version: String,
    proto: usize,
}

impl<'a> CommandHandler for HelloCommand<'a> {
    type Output = ServerProperties;

    fn handle(self, _db: &Database) -> Result<Self::Output, Error> {
        Ok(ServerProperties {
            server: String::from("memds"),
            version: String::from("0.0.1"),
            proto: 3,
        })
    }
}

impl CommandHandler for CommandCommand {
    type Output = [usize; 0];

    fn handle(self, _db: &Database) -> Result<Self::Output, Error> {
        Ok([])
    }
}

#[derive(Debug, CommandArgsBlock)]
#[argtoken("GET")]
pub struct GetCommand<'a> {
    key: &'a str,
}

impl<'a> CommandHandler for GetCommand<'a> {
    type Output = Option<String>;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        Ok(db.get(self.key))
    }
}

#[derive(Debug, CommandArgsBlock)]
#[argtoken("SET")]
pub struct SetCommand<'a> {
    key: &'a str,
    value: &'a str,
}

#[derive(Debug)]
pub struct OkResponse;

impl Serialize for OkResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        serializer.serialize_newtype_struct("$SimpleString", "OK")
    }
}

impl<'a> CommandHandler for SetCommand<'a> {
    type Output = OkResponse;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        db.set(self.key, self.value);
        Ok(OkResponse)
    }
}

#[derive(Debug)]
pub enum Error {
    Parse(command_args::Error),
    Handle(String),
    Serialize(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error { }

fn parse_handle<'a, T>(args: &[&'a str], db: &Database, write_buf: &mut Vec<u8>) -> Result<bool, Error>
where
    T: CommandHandler,
    T: CommandArgs<'a>,
    T: std::fmt::Debug,
    <T as CommandHandler>::Output: std::fmt::Debug,
{
    let command = T::parse_maybe(&mut &args[..])
        .map_err(|e| Error::Parse(e))?;

    let mut serializer = deseresp::from_write(write_buf);
    if let Some(command) = command {
        tracing::info!("received {:?}", command);

        let result = command.handle(db)?;
        tracing::info!("Return {:?}", result);

        result.serialize(&mut serializer)
            .map_err(|e| Error::Serialize(e.to_string()))?;

        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_unsupported_command(args: &[&str], write_buf: &mut Vec<u8>) -> Result<(), Error> {
    let mut serializer = deseresp::from_write(write_buf);
    let response = deseresp::types::owned::SimpleError(format!("ERR command {} not supported", args[0]));

    response.serialize(&mut serializer)
        .map_err(|e| Error::Serialize(e.to_string()))?;

    Ok(())
}

macro_rules! try_commands {
    (($args:ident, $db:ident, $write_buf:ident) {$command_type:ident}) => {
        if parse_handle::<$command_type>($args, $db, $write_buf)? {
            return Ok(());
        }
    };
    (($args:ident, $db:ident, $write_buf:ident) {$cmd_type1:ident, $($cmd_type2:ident),+}) => {
        try_commands!(($args, $db, $write_buf) {$cmd_type1});
        try_commands!(($args, $db, $write_buf) {$($cmd_type2),+})
    }
}
pub fn parse_and_handle(args: &[&str], db: &Database, write_buf: &mut Vec<u8>) -> Result<(), Error> {
    if args.is_empty() {
        return Err(Error::Parse(command_args::Error::Incompleted));
    }
    try_commands!((args, db, write_buf) {
        HelloCommand, CommandCommand, GetCommand, SetCommand
    });

    // not supported command
    handle_unsupported_command(args, write_buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use command_args::CommandArgs;

    #[test]
    fn test_parse_hello_command() {
        let args = ["HELLO", "3", "AUTH", "user", "pass"];
        let command = HelloCommand::parse_maybe(&mut &args[..])
            .unwrap()
            .unwrap();
        assert_eq!(command.protover, 3);
        assert_eq!(command.auth.as_ref().unwrap().username, "user");
        assert_eq!(command.auth.as_ref().unwrap().username, "user");
    }

    #[test]
    fn test_handle_and_parse_hello_command() {
        let db = Database::new();
        let args = ["HELLO", "3", "AUTH", "user", "pass"];
        let mut write_buf = Vec::new();
        parse_and_handle(&args, &db, &mut write_buf)
            .unwrap();
        let result_s = std::str::from_utf8(&write_buf).unwrap();
        assert_eq!(result_s, "%3\r\n+server\r\n+memds\r\n+version\r\n+0.0.1\r\n+proto\r\n:3\r\n");
    }
}
