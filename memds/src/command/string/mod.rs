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

#[derive(CommandArgsBlock, Debug, PartialEq)]
#[argtoken("SET")]
pub struct SetCommand<'a> {
    key: &'a str,
    value: &'a str,
    exists: Exists,
    get: Option<SetGet>,
    expire: Option<ExpireOption>,
}

#[derive(CommandArgsBlock, Debug, PartialEq)]
enum Exists {
    #[argtoken("NX")]
    NotExistedOnly,
    #[argtoken("XX")]
    ExistedOnly,
    #[argnotoken]
    Any,
}

#[derive(CommandArgsBlock, Debug, PartialEq)]
#[argtoken("GET")]
struct SetGet;

#[derive(CommandArgsBlock, Debug, PartialEq)]
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
    use command_args::CommandArgs;

    #[test]
    fn test_parse_set() {
        let cmd_str = vec!["SET", "a", "b", "NX", "GET", "EX", "20"];
        let s = SetCommand::parse_maybe(&mut &cmd_str[..]).unwrap().unwrap();

        assert_eq!(s.key, "a");
        assert_eq!(s.value, "b");
        assert_matches!(s.exists, Exists::NotExistedOnly);
        assert_matches!(s.get, Some(SetGet));
        assert_matches!(s.expire, Some(ExpireOption::ExpireAfterSecond(20)));

        let cmd_str = vec!["SET", "a", "b", "PXAT", "20"];
        let s = SetCommand::parse_maybe(&mut &cmd_str[..]).unwrap().unwrap();

        assert_eq!(s.key, "a");
        assert_eq!(s.value, "b");
        assert_matches!(s.exists, Exists::Any);
        assert_matches!(s.get, None);
        assert_matches!(s.expire, Some(ExpireOption::ExpireAtMs(20)));
    }

    #[test]
    fn test_encode_set() {
        let s = SetCommand {
            key: "abc",
            value: "def",
            exists: Exists::Any,
            get: None,
            expire: Some(ExpireOption::ExpireAfterSecond(20)),
        };
        let mut target: Vec<String> = Vec::new();
        s.encode(&mut target).unwrap();

        let expected = ["SET", "abc", "def", "EX", "20"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(expected, target);
        let target_ref = target.iter().map(String::as_str).collect::<Vec<_>>();

        let source = SetCommand::parse_maybe(&mut &target_ref[..]).unwrap().unwrap();
        assert_eq!(source, s);
    }
}
