use command_args::CommandArgs;
use serde::Serialize;

use crate::{database::Database, Error};

mod connection;
mod string;
mod set;

pub trait CommandHandler {
    type Output: Serialize;
    fn handle(self, db: &Database) -> Result<Self::Output, Error>;
}

fn parse_handle<'a, T>(
    args: &[&'a str],
    db: &Database,
    write_buf: &mut Vec<u8>,
) -> Result<bool, Error>
where
    T: CommandHandler,
    T: CommandArgs<'a>,
    T: std::fmt::Debug,
{
    let command = T::parse_maybe(&mut &args[..]).map_err(Error::Parse)?;

    let mut serializer = deseresp::from_write(write_buf);
    if let Some(command) = command {
        let result = command.handle(db)?;

        result
            .serialize(&mut serializer)
            .map_err(|e| Error::Serialize(e.to_string()))?;

        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_unsupported_command(args: &[&str], write_buf: &mut Vec<u8>) -> Result<(), Error> {
    let mut serializer = deseresp::from_write(write_buf);
    let response =
        deseresp::types::owned::SimpleError(format!("ERR command {} not supported", args[0]));

    response
        .serialize(&mut serializer)
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

fn parse_and_handle_main(
    args: &[&str],
    db: &Database,
    write_buf: &mut Vec<u8>,
) -> Result<(), Error> {
    try_commands!((args, db, write_buf) {
        self::connection::HelloCommand,
        self::connection::CommandCommand,
        self::connection::PingCommand,
        self::string::GetCommand,
        self::string::SetCommand,
        self::set::SaddCommand,
        self::set::SmembersCommand
    });

    // not supported command
    handle_unsupported_command(args, write_buf)
}

// Entry point of command routing
// Try to parse command, handle and write response to write_buf
// On error, return true for connection event loop to Flush response
// Return Ok(false) if everything ok
pub fn parse_and_handle(
    args: &[&str],
    db: &Database,
    write_buf: &mut Vec<u8>,
) -> Result<bool, Error> {
    match parse_and_handle_main(args, db, write_buf) {
        Ok(_) => Ok(false),
        Err(Error::Parse(e)) => {
            tracing::error!("Failed to parse command: {:?}, e: {}", &args, e);
            let mut serializer = deseresp::from_write(write_buf);
            let response =
                deseresp::types::owned::SimpleError(format!("ERR failed to parse: {}", args[0]));

            response
                .serialize(&mut serializer)
                .map_err(|e| Error::Serialize(e.to_string()))?;

            Ok(true)
        }
        Err(Error::Handle(e)) => {
            tracing::error!("Failed to handle command: {:?}, e: {}", &args, e);
            let mut serializer = deseresp::from_write(write_buf);
            let response = deseresp::types::owned::SimpleError(e);

            response
                .serialize(&mut serializer)
                .map_err(|e| Error::Serialize(e.to_string()))?;

            Ok(true)
        }
        Err(e @ Error::Serialize(_)) => {
            // failed to serialize response to user,
            // nothing to do except log and disconnect
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_and_parse_hello_command() {
        let db = Database::new();
        let args = ["HELLO", "3", "AUTH", "user", "pass"];
        let mut write_buf = Vec::new();
        parse_and_handle(&args, &db, &mut write_buf).unwrap();
        let result_s = std::str::from_utf8(&write_buf).unwrap();
        assert_eq!(
            result_s,
            "%3\r\n+server\r\n+memds\r\n+version\r\n+0.0.1\r\n+proto\r\n:3\r\n"
        );
    }
}
