use std::sync::Arc;

use bytes::{Buf, BytesMut};
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{tcp::OwnedReadHalf, TcpListener, TcpStream},
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
            let conn = Session::new(socket, Arc::clone(&self.db));

            tokio::spawn(async move {
                if let Err(e) = conn.handle().await {
                    tracing::error!("Error: {}", e);
                };
            });
        }
    }
}

struct Session {
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

fn parse(read_buf: &mut BytesMut) -> anyhow::Result<Option<(Vec<&'_ str>, usize)>> {
    let mut deserializer = deseresp::from_slice(read_buf);
    match Deserialize::deserialize(&mut deserializer) {
        Ok(deserialized) => {
            let data = deserialized;
            let bytes_consumed = deserializer.get_consumed_bytes();
            Ok(Some((data, bytes_consumed)))
        }
        Err(deseresp::Error::EOF) => Ok(None),
        Err(e) => {
            tracing::error!(
                "Error parsing command: {}, e: {}",
                std::str::from_utf8(&read_buf).unwrap_or(&String::from("invalid utf8")),
                e
            );
            return Err(e.into());
        }
    }
}

struct Connection {
    reader: OwnedReadHalf,
    read_buf: BytesMut,
    last_frame_bytes_consumed: usize,
}

impl Connection {
    pub fn new(reader: OwnedReadHalf) -> Self {
        Connection {
            reader,
            read_buf: BytesMut::with_capacity(4096),
            last_frame_bytes_consumed: 0,
        }
    }

    pub async fn read_to_buf(&mut self) -> anyhow::Result<usize> {
        Ok(self.reader.read_buf(&mut self.read_buf).await?)
    }

    pub fn next_buffered_frame<'a>(&'a mut self) -> anyhow::Result<Option<Vec<&'a str>>> {
        self.read_buf.advance(self.last_frame_bytes_consumed);
        self.last_frame_bytes_consumed = 0;

        Ok(match parse(&mut self.read_buf)? {
            Some((frame, bytes_consumed)) => {
                self.last_frame_bytes_consumed = bytes_consumed;
                Some(frame)
            }
            None => None,
        })
    }
}

impl Session {
    fn new(socket: TcpStream, db: Arc<Database>) -> Self {
        Session { socket, db }
    }

    async fn handle(self) -> anyhow::Result<()> {
        let (reader, mut writer) = self.socket.into_split();
        let mut write_buf = Vec::new();
        let mut connection = Connection::new(reader);

        loop {
            while let Some(frame) = connection.next_buffered_frame()? {
                tracing::info!("Received frame: {:?}", frame);
                match crate::command::parse_and_handle(&frame[..], &self.db, &mut write_buf) {
                    Ok(need_flush) => {
                        if need_flush || write_buf.len() > WRITE_BUF_SIZE_LIMIT {
                            flush(&mut writer, &mut write_buf).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error handling command: {:?}, e: {}", &frame, e);
                        return Err(e.into());
                    }
                }
            }

            let (read_bytes, ()) =
                tokio::join!(connection.read_to_buf(), flush(&mut writer, &mut write_buf));
            if read_bytes? == 0 {
                tracing::info!("Session ended");
                break;
            }
        }

        Ok(())
    }
}
