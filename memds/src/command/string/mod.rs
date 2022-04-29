use command_args::CommandArgs;
use command_args_derive::CommandArgsBlock;
use deseresp::types::OkResponse;

use crate::database::Database;

use super::{CommandHandler, Error};

#[derive(Debug, CommandArgsBlock)]
#[argtoken("GET")]
pub struct GetCommand<'a> {
    key: &'a str,
}

impl<'a> CommandHandler for GetCommand<'a> {
    type Output = Option<String>;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        db.get(self.key)
    }
}

#[derive(CommandArgsBlock, Debug)]
#[argtoken("SET")]
pub struct SetCommand<'a> {
  key: &'a str,
  value: &'a str,
  is_nx_xx: Option<NxOrXX>,
  is_get: Option<SetGet>,
}

#[derive(CommandArgsBlock, Debug)]
enum NxOrXX {
  #[argtoken("NX")]
  NX,
  // use enum name if not provided #[argtoken]
  XX
}

#[derive(CommandArgsBlock, Debug)]
#[argtoken("GET")]
struct SetGet;

impl<'a> CommandHandler for SetCommand<'a> {
    type Output = OkResponse;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        db.set(self.key, self.value)?;
        Ok(OkResponse)
    }
}

