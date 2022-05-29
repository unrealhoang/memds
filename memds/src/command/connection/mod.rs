use command_args_derive::CommandArgsBlock;
use deseresp::types::owned::SimpleString;
use serde::Serialize;

use crate::database::Database;

use super::{CommandHandler, Error};

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
pub struct CommandCommand;

#[derive(Debug, Serialize)]
pub struct ServerProperties {
    pub server: String,
    pub version: String,
    pub proto: usize,
}

#[derive(Debug, CommandArgsBlock)]
#[argtoken("PING")]
pub struct PingCommand;

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

impl CommandHandler for PingCommand {
    type Output = SimpleString;

    fn handle(self, _db: &Database) -> Result<Self::Output, Error> {
        Ok(SimpleString("PONG".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use command_args::CommandArgs;

    #[test]
    fn test_parse_hello_command() {
        let args = ["HELLO", "3", "AUTH", "user", "pass"];
        let command = HelloCommand::parse_maybe(&mut &args[..]).unwrap().unwrap();
        assert_eq!(command.protover, 3);
        assert_eq!(command.auth.as_ref().unwrap().username, "user");
        assert_eq!(command.auth.as_ref().unwrap().username, "user");
    }
}
