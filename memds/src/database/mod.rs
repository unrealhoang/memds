use std::{collections::HashMap, sync::Mutex};

use crate::{
    memds::{MemDS, StringDS, SetDS},
    Error, storage,
};

pub struct Database {
    data: Mutex<HashMap<String, MemDS>>,
}

impl Database {
    pub fn new() -> Self {
        match storage::load() {
            Ok(d) => {
                Database {
                    data: Mutex::new(d),
                }
            }
            Err(e) => {
                tracing::error!("Failed to load data: {}", e);
                Database {
                    data: Mutex::new(Default::default()),
                }
            }
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

    pub fn sadd(&self, key: &str, elements: &[&str]) -> Result<usize, Error> {
        let mut lock = self.data
            .lock()
            .unwrap();
        let set = lock
            .entry(key.to_string())
            .or_insert_with(|| MemDS::Set(SetDS::default()));
        let added = set.set_mut(key)?.add(elements.iter());

        Ok(added)
    }

    pub fn smembers(&self, key: &str) -> Result<Option<Vec<String>>, Error> {
        let lock = self.data
            .lock()
            .unwrap();

        match lock.get(key) {
            None => Ok(None),
            Some(set) => Ok(Some(set.set(key)?.members())),
        }
    }

    pub fn save(&self) -> Result<(), Error> {
        let lock = self.data
            .lock()
            .unwrap();

        storage::save(&lock)
    }
}
