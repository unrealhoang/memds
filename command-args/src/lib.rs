use std::{fmt::{Write as _, Display}, io::{Write, self}};

pub trait CommandArgs<'a>: Sized {
    // fn encode<W: Write>(&self, target: &mut W) -> Result<(), Error>;
    fn parse_maybe(args: &mut &[&'a str]) -> Result<Option<Self>, Error>;
}

impl<'a> CommandArgs<'a> for &'a str {
    // fn encode<W: Write>(&self, target: &mut W) -> Result<(), Error> {
    //     target.write_all(self.as_bytes())
    //         .map_err(|e| Error::Write(e))
    // }

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
    // fn encode<W: Write>(&self, target: &mut W) -> Result<(), Error> {
    //     let mut first = true;
    //     for ele in self.iter() {
    //         ele.encode(target)?;
    //         if !first {
    //             target.write_all(b" ").map_err(|e| Error::Write(e))?;
    //         }
    //         first = true;
    //     }

    //     Ok(())
    // }

    fn parse_maybe(args: &mut &[&'a str]) -> Result<Option<Self>, Error> {
        if args.is_empty() {
            return Ok(None);
        }

        let mut result = Vec::new();
        while let Some(ele) = T::parse_maybe(args)? {
            result.push(ele);
        }

        Ok(Some(result))
    }
}

impl<'a> CommandArgs<'a> for usize {
    // fn encode<W: Write>(&self, target: &mut W) -> Result<(), Error> {
    //     write!(target, "{}", self)
    //         .map_err(|e| Error::Write(e))
    // }

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
    Write(io::Error),
    InvalidLength,
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
        let s = <usize as CommandArgs>::parse_maybe(&mut &args[..]).unwrap();
        assert_eq!(s, Some(1));
    }
}
