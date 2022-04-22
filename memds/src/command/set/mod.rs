use command_args_derive::CommandArgsBlock;

use crate::database::Database;

use super::{CommandHandler, Error};

#[derive(Debug, CommandArgsBlock)]
#[argtoken("SADD")]
pub struct SaddCommand<'a> {
    key: &'a str,
    elements: Vec<&'a str>,
}

impl<'a> CommandHandler for SaddCommand<'a> {
    type Output = usize;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        db.sadd(self.key, &self.elements)
    }
}

#[derive(Debug, CommandArgsBlock)]
#[argtoken("SMEMBERS")]
pub struct SmembersCommand<'a> {
    key: &'a str,
}

impl<'a> CommandHandler for SmembersCommand<'a> {
    type Output = Option<Vec<String>>;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        db.smembers(self.key)
    }
}
