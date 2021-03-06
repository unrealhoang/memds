use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter},
};

use crate::{memds::MemDS, Error};

// TODO: change to storage Error
pub fn save(db_path: &str, db: &HashMap<String, MemDS>) -> Result<(), Error> {
    let file = File::create(db_path)
        .map_err(|e| Error::Handle(format!("Failed to open db file {}", e)))?;
    let writer = BufWriter::new(file);

    bincode::serialize_into(writer, db).map_err(|e| Error::Handle(format!("Failed to save {}", e)))
}

pub fn load(db_path: &str) -> Result<HashMap<String, MemDS>, Error> {
    let file =
        File::open(db_path).map_err(|e| Error::Handle(format!("Failed to open db file {}", e)))?;
    let reader = BufReader::new(file);

    bincode::deserialize_from(reader).map_err(|e| Error::Handle(format!("Failed to load {}", e)))
}
