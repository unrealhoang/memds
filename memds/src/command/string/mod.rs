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
  exists: Option<Exists>,
  get: Option<SetGet>,
  expire: Option<ExpireOption>
}

#[derive(CommandArgsBlock, Debug)]
enum Exists {
  #[argtoken("NX")]
  NX,
  // use enum name if not provided #[argtoken]
  XX
}

#[derive(CommandArgsBlock, Debug)]
#[argtoken("GET")]
struct SetGet;

#[derive(CommandArgsBlock, Debug)]
enum ExpireOption {
  #[argtoken("EX")]
  ExpireAfterSecond(usize),
  #[argtoken("PX")]
  ExpireAfterMs(usize),
  #[argtoken("EXAT")]
  ExpireAtSecond(usize),
  #[argtoken("PXAT")]
  ExpireAtMs(usize),
  KeepTTL,
}

impl<'a> CommandHandler for SetCommand<'a> {
    type Output = OkResponse;

    fn handle(self, db: &Database) -> Result<Self::Output, Error> {
        db.set(self.key, self.value)?;
        Ok(OkResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    #[test]
    fn test_parse_set() {
        let cmd_str = vec!["SET", "a", "b", "NX", "GET", "EX", "20"];
        let s = SetCommand::parse_maybe(&mut &cmd_str[..]).unwrap().unwrap();

        assert_eq!(s.key, "a");
        assert_eq!(s.value, "b");
        assert_matches!(s.exists, Some(Exists::NX));
        assert_matches!(s.get, Some(SetGet));
        assert_matches!(s.expire, Some(ExpireOption::ExpireAfterSecond(20)));
    }
}
