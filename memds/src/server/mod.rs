use std::sync::Arc;

use anyhow::bail;
use bytes::BytesMut;
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::database::Database;

/// 100KB buffer size for pipelined write
const WRITE_BUF_SIZE_LIMIT: usize = 1024 * 100;

pub struct Server {
    port: u16,
    db: Arc<Database>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            port: 6901,
            db: Arc::new(Database::new()),
        }
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", self.port)).await?;

        let addr = listener.local_addr().unwrap();
        tracing::info!("Listening... {}", addr);
        loop {
            let (socket, _) = listener.accept().await?;
            let conn = Connection::new(socket, Arc::clone(&self.db));

            tokio::spawn(async move {
                if let Err(e) = conn.handle().await {
                    tracing::error!("Error: {}", e);
                };
            });
        }
    }
}

struct Connection {
    socket: TcpStream,
    db: Arc<Database>,
}

async fn flush<T>(mut writer: T, write_buf: &mut Vec<u8>)
where
    T: AsyncWrite + Unpin,
{
    if !write_buf.is_empty() {
        writer.write_all(&write_buf[..]).await.unwrap();
        write_buf.clear();
    }
}
impl Connection {
    fn new(socket: TcpStream, db: Arc<Database>) -> Self {
        Connection { socket, db }
    }

    async fn handle(mut self) -> anyhow::Result<()> {
        let (mut reader, mut writer) = self.socket.split();
        let mut read_buf = BytesMut::with_capacity(4096); // 1KB
        let mut write_buf = Vec::new();
        'read: loop {
            flush(&mut writer, &mut write_buf).await;
            let len = reader.read_buf(&mut read_buf).await?;
            if len == 0 {
                if read_buf.is_empty() {
                    tracing::info!("Client exited");
                    return Ok(());
                } else {
                    bail!("connection reset by peer");
                }
            }
            'parse: loop {
                tracing::info!(
                    "received: {}",
                    std::str::from_utf8(&read_buf).unwrap_or(&String::from("invalid utf8"))
                );
                let mut deserializer = deseresp::from_slice(&read_buf);
                let command_vec: Vec<&str> = match Deserialize::deserialize(&mut deserializer) {
                    Ok(deserialized) => deserialized,
                    Err(deseresp::Error::EOF) => {
                        continue 'read;
                    }
                    Err(e) => {
                        tracing::error!(
                            "Error parsing command: {}, e: {}",
                            std::str::from_utf8(&read_buf).unwrap_or(&String::from("invalid utf8")),
                            e
                        );
                        return Err(e.into());
                    }
                };

                tracing::info!(
                    "done deserializing, parse command & handle: {:?}",
                    &command_vec
                );
                match crate::command::parse_and_handle(&command_vec[..], &self.db, &mut write_buf) {
                    Ok(need_flush) => {
                        if need_flush || write_buf.len() > WRITE_BUF_SIZE_LIMIT {
                            flush(&mut writer, &mut write_buf).await;
                        }

                        let consumed_bytes = deserializer.get_consumed_bytes();
                        let _ = read_buf.split_to(consumed_bytes);

                        continue 'parse;
                    }
                    Err(e) => {
                        tracing::error!("Error handling command: {:?}, e: {}", &command_vec, e);
                        return Err(e.into());
                    }
                }
            }
        }
    }
}
