use std::collections::HashSet;

use crate::Error;

pub enum MemDS {
    String(StringDS),
    Set(SetDS),
}

pub struct StringDS {
    s: String,
}

pub struct SetDS {
    s: HashSet<String>,
}

impl MemDS {
    pub fn string(&self, key: &str) -> Result<&StringDS, Error> {
        match self {
            MemDS::String(s) => Ok(s),
            _ => Err(Error::Handle(format!("ERR key {} is not string", key))),
        }
    }

    pub fn set(&self, key: &str) -> Result<&SetDS, Error> {
        match self {
            MemDS::Set(s) => Ok(s),
            _ => Err(Error::Handle(format!("ERR key {} is not set", key))),
        }
    }

    pub fn set_mut(&mut self, key: &str) -> Result<&mut SetDS, Error> {
        match self {
            MemDS::Set(s) => Ok(s),
            _ => Err(Error::Handle(format!("ERR key {} is not set", key))),
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

impl SetDS {
    pub fn from<S: ToString>(s: S) -> Self {
        let mut set = HashSet::new();
        set.insert(s.to_string());
        Self {
            s: set
        }
    }

    pub fn add<E, S>(&mut self, elements: E) -> usize
    where
        E: Iterator<Item = S>,
        S: ToString,
    {
        let mut count = 0;
        for e in elements {
            if self.s.insert(e.to_string()) {
                count += 1;
            }
        }

        count
    }

    pub fn members(&self) -> Vec<String> {
        self.s.iter().cloned().collect()
    }
}

impl Default for SetDS {
    fn default() -> Self {
        Self { s: Default::default() }
    }
}
