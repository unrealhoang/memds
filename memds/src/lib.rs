pub mod client;
pub mod command;
pub mod connection;
pub mod database;
pub mod memds;
mod server;
pub mod storage;
mod wal;

pub use server::Server;

use std::{fmt::Display, future::Future, pin::Pin};

use futures::future::FutureExt;

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

/// A future that will terminate its service on poll
pub struct Terminator {
    f: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl Terminator {
    fn from_future<F: Future<Output = ()> + Send + 'static>(f: F) -> Self {
        Terminator { f: f.boxed() }
    }
}

impl Future for Terminator {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.f.as_mut().poll(cx)
    }
}
