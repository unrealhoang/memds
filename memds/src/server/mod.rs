use std::{net::SocketAddr, sync::Arc};

use futures::{future, stream::FuturesUnordered, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast,
};

use crate::{
    connection::{flush, FrameReader},
    database::Database,
    Terminator,
};

/// 100KB buffer size for pipelined write
const WRITE_BUF_SIZE_LIMIT: usize = 1024 * 100;

pub struct Server {
    port: u16,
    db: Arc<Database>,
}

async fn accept_loop(
    db: Arc<Database>,
    listener: TcpListener,
    addr: SocketAddr,
    shutdown_tx: broadcast::Sender<()>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    // TODO: use tokio's JoinSet when stable
    let mut sessions = FuturesUnordered::new();

    tracing::info!("Listening... {}", addr);
    loop {
        tokio::select! {
            Some(_) = sessions.next() => {
                tracing::debug!("Session ended");
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((socket, _)) => {
                        let conn =
                            Session::new(socket, db.clone(), shutdown_tx.subscribe());

                        sessions.push(tokio::spawn(async move {
                            if let Err(e) = conn.handle().await {
                                tracing::error!("Error: {}", e);
                            };
                        }));
                    }
                    Err(e) => {
                        tracing::error!("Failed to accept request: {}", e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                break
            }
        }
    }

    tracing::info!("Wait for sessions to end ...");
    while sessions.next().await.is_some() {}
    tracing::info!("All sessions ended.");
}

impl Server {
    pub fn new(port: u16, db_path: String) -> Self {
        Server {
            port,
            db: Arc::new(Database::new(db_path)),
        }
    }

    pub async fn service(self) -> anyhow::Result<(SocketAddr, Terminator)> {
        let listener = TcpListener::bind(("127.0.0.1", self.port)).await?;

        let addr = listener.local_addr().unwrap();

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let shutdown_tx_terminator = shutdown_tx.clone();
        let service_handler = tokio::spawn(async move {
            accept_loop(self.db.clone(), listener, addr, shutdown_tx, shutdown_rx).await;

            tracing::info!("Saving DB");
            if let Err(e) = self.db.save() {
                tracing::error!("Failed to save DB: {}", e);
            }
        });

        let terminator = Terminator::from_future(async move {
            if let Err(e) = shutdown_tx_terminator.send(()) {
                tracing::error!("Failed to send shutdow signal: {}", e);
            }
            if let Err(e) = service_handler.await {
                tracing::error!("Failed to wait for server to shutdown: {}", e);
            }
        });

        Ok((addr, terminator))
    }
}

struct Session {
    socket: TcpStream,
    db: Arc<Database>,
    shutdown: broadcast::Receiver<()>,
}

impl Session {
    fn new(socket: TcpStream, db: Arc<Database>, shutdown: broadcast::Receiver<()>) -> Self {
        Session {
            socket,
            db,
            shutdown,
        }
    }

    async fn handle(mut self) -> anyhow::Result<()> {
        let (reader, mut writer) = self.socket.into_split();
        let mut write_buf = Vec::new();
        let mut connection = FrameReader::new(reader);

        'main: loop {
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

            let read_write =
                future::join(connection.read_to_buf(), flush(&mut writer, &mut write_buf));
            tokio::pin!(read_write);

            tokio::select! {
                _ = self.shutdown.recv() => {
                    tracing::info!("Receive shutdown request, end session.");
                    break 'main;
                }
                (read_bytes, ()) = &mut read_write => {
                    if read_bytes? == 0 {
                        tracing::info!("Session ended by client.");
                        break 'main;
                    }
                }
            }
        }

        Ok(())
    }
}
