use command_args_derive::CommandArgsBlock;
use serde::Serialize;

use crate::server::Database;

use super::{CommandHandler, Error};

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
