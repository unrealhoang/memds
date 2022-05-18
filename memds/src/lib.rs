pub mod client;
pub mod command;
pub mod connection;
pub mod database;
pub mod memds;
mod server;
pub mod storage;
mod wal;

use std::fmt::Display;

pub use server::Server;

#[derive(Debug)]
pub enum Error {
    Parse(command_args::Error),
    Handle(String),
    Serialize(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}
