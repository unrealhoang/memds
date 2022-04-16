use std::fmt::Display;

use command_args::CommandArgs;
use serde::Serialize;

use crate::server::Database;

mod connection;
mod string;

pub trait CommandHandler {
    type Output: Serialize;
    fn handle(self, db: &Database) -> Result<Self::Output, Error>;
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
    (($args:ident, $db:ident, $write_buf:ident) {$command_type:path}) => {
        if parse_handle::<$command_type>($args, $db, $write_buf)? {
            return Ok(());
        }
    };
    (($args:ident, $db:ident, $write_buf:ident) {$cmd_type1:path, $($cmd_type2:path),+}) => {
        try_commands!(($args, $db, $write_buf) {$cmd_type1});
        try_commands!(($args, $db, $write_buf) {$($cmd_type2),+})
    }
}
pub fn parse_and_handle(args: &[&str], db: &Database, write_buf: &mut Vec<u8>) -> Result<(), Error> {
    if args.is_empty() {
        return Err(Error::Parse(command_args::Error::Incompleted));
    }
    try_commands!((args, db, write_buf) {
        self::connection::HelloCommand,
        self::connection::CommandCommand,
        self::string::GetCommand,
        self::string::SetCommand
    });

    // not supported command
    handle_unsupported_command(args, write_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

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
