use std::fmt::Display;

pub trait CommandArgs<'a>: Sized {
    fn parse(args: &mut &[&'a str]) -> Result<Self, Error>;
    fn parse_maybe(args: &mut &[&'a str]) -> Result<Option<Self>, Error> {
        if !args.is_empty() {
            Ok(Some(Self::parse(args)?))
        } else {
            Ok(None)
        }
    }
}

impl<'a> CommandArgs<'a> for &'a str {
    fn parse(args: &mut &[&'a str]) -> Result<Self, Error> {
        if let Some(s) = args.get(0) {
            *args = &args[1..];
            Ok(s)
        } else {
            Err(Error::Incompleted)
        }
    }

    fn parse_maybe(args: &mut &[&'a str]) -> Result<Option<Self>, Error> {
        if let Some(s) = args.get(0) {
            *args = &args[1..];
            Ok(Some(s))
        } else {
            Ok(None)
        }
    }
}

impl<'a, T: CommandArgs<'a>> CommandArgs<'a> for Vec<T> {
    fn parse(args: &mut &[&'a str]) -> Result<Self, Error> {
        if args.is_empty() {
            return Err(Error::Incompleted);
        }

        let mut result = Vec::new();
        while !args.is_empty() {
            let ele = T::parse(args)?;
            result.push(ele);
        }

        Ok(result)
    }

    fn parse_maybe(args: &mut &[&'a str]) -> Result<Option<Self>, Error> {
        if args.is_empty() {
            return Ok(None);
        }

        let mut result = Vec::new();
        while !args.is_empty() {
            let ele = T::parse(args)?;
            result.push(ele);
        }

        Ok(Some(result))
    }
}

impl<'a> CommandArgs<'a> for usize {
    fn parse(args: &mut &[&'a str]) -> Result<Self, Error> {
        if let Some(s) = args.get(0) {
            *args = &args[1..];
            Ok(s.parse().map_err(|_| Error::Parse)?)
        } else {
            Err(Error::Incompleted)
        }
    }

    fn parse_maybe(args: &mut &[&'a str]) -> Result<Option<Self>, Error> {
        if let Some(s) = args.get(0) {
            *args = &args[1..];
            Ok(Some(s.parse().map_err(|_| Error::Parse)?))
        } else {
            Ok(None)
        }
    }
}

pub trait CommandBuilder<'a> {
    const NAME: &'static str;
}

#[derive(Debug)]
pub enum Error {
    Incompleted,
    Parse,
    TokenNotFound(&'static str),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_usize() {
        let args = ["1"];
        let s = <usize as CommandArgs>::parse(&mut &args[..]).unwrap();
        assert_eq!(s, 1);
    }
}
