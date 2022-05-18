use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};

use crate::{
    connection::{flush, FrameReader},
    database::Database,
};

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

impl Session {
    fn new(socket: TcpStream, db: Arc<Database>) -> Self {
        Session { socket, db }
    }

    async fn handle(self) -> anyhow::Result<()> {
        let (reader, mut writer) = self.socket.into_split();
        let mut write_buf = Vec::new();
        let mut connection = FrameReader::new(reader);

        loop {
            while let Some(frame) = connection.next_buffered_frame::<Vec<&str>>()? {
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
