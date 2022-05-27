use anyhow::Context;
use command_args::CommandArgs;
use serde::{de::DeserializeOwned, Serialize};
use tokio::net::{tcp::OwnedWriteHalf, TcpStream, ToSocketAddrs};

use crate::{
    command::CommandHandler,
    connection::{flush, FrameReader},
};

pub struct Client {
    frame_reader: FrameReader,
    writer: OwnedWriteHalf,
    write_buf: Vec<u8>,
    command_buf: Vec<String>,
}

impl Client {
    pub async fn from_addr<A: ToSocketAddrs>(addr: A) -> anyhow::Result<Self> {
        let conn = TcpStream::connect(addr).await?;

        Ok(Self::new(conn))
    }

    pub fn new(socket: TcpStream) -> Self {
        let (reader, writer) = socket.into_split();
        let frame_reader = FrameReader::new(reader);
        let write_buf = Vec::new();
        let command_buf = Vec::new();

        Client {
            frame_reader,
            writer,
            write_buf,
            command_buf,
        }
    }

    pub async fn execute<'a, C>(
        &mut self,
        command: &C,
    ) -> anyhow::Result<<C as CommandHandler>::Output>
    where
        C: CommandHandler + CommandArgs<'a>,
        <C as CommandHandler>::Output: DeserializeOwned,
    {
        self.command_buf.clear();
        command
            .encode(&mut self.command_buf)
            .context("Failed to encode command")?;

        let mut serializer = deseresp::from_write(&mut self.write_buf);
        self.command_buf
            .serialize(&mut serializer)
            .context("Failed to serialize command to RESP")?;

        flush(&mut self.writer, &mut self.write_buf).await;

        loop {
            if let Some(resp) = self.frame_reader.next_buffered_frame::<C::Output>()? {
                return Ok(resp);
            }
            let read_bytes = self.frame_reader.read_to_buf().await?;

            if read_bytes == 0 {
                tracing::info!("Session ended");
                break;
            }
        }

        anyhow::bail!("Server connection ended");
    }
}
