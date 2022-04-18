use std::{collections::HashMap, sync::Mutex};

use crate::{
    memds::{MemDS, StringDS},
    Error,
};

pub struct Database {
    data: Mutex<HashMap<String, MemDS>>,
}

impl Database {
    pub fn new() -> Self {
        Database {
            data: Mutex::new(Default::default()),
        }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, Error> {
        self.data
            .lock()
            .unwrap()
            .get(key)
            .map(|v| v.string(key).map(StringDS::fetch))
            .transpose()
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_owned(), MemDS::String(StringDS::from(value)));

        Ok(())
    }
}
