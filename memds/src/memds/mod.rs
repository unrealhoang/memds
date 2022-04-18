use crate::Error;

pub enum MemDS {
    String(StringDS),
}

pub struct StringDS {
    s: String,
}

impl MemDS {
    pub fn string(&self, key: &str) -> Result<&StringDS, Error> {
        match self {
            MemDS::String(s) => Ok(s),
            _ => Err(Error::Handle(format!("ERR key {} is not string", key))),
        }
    }
}

impl StringDS {
    pub fn from<S: ToString>(s: S) -> Self {
        Self { s: s.to_string() }
    }

    pub fn fetch(&self) -> String {
        self.s.to_owned()
    }
}
