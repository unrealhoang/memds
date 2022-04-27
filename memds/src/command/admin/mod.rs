use command_args_derive::CommandArgsBlock;
use deseresp::types::OkResponse;

use crate::database::Database;

use super::{CommandHandler, Error};

#[derive(Debug, CommandArgsBlock)]
#[argtoken("SAVE")]
pub struct SaveCommand;

impl CommandHandler for SaveCommand {
    type Output = OkResponse;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        db.save()?;

        Ok(OkResponse)
    }
}
