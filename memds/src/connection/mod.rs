use bytes::{BytesMut, Buf};
use serde::Deserialize;
use tokio::{net::tcp::OwnedReadHalf, io::{AsyncWrite, AsyncWriteExt, AsyncReadExt}};

fn parse<'a, D: Deserialize<'a>>(read_buf: &'a mut BytesMut) -> anyhow::Result<Option<(D, usize)>> {
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

/// A redis buffered frame reader
pub struct FrameReader {
    /// source reader
    reader: OwnedReadHalf,
    /// Read buffer
    read_buf: BytesMut,
    /// Number of bytes consumed to decode the last frame
    last_frame_bytes_consumed: usize,
}

impl FrameReader {
    /// Creates new frame reader from OwnedReadHalf
    pub fn new(reader: OwnedReadHalf) -> Self {
        FrameReader {
            reader,
            read_buf: BytesMut::with_capacity(4096),
            last_frame_bytes_consumed: 0,
        }
    }

    /// Read from reader to the read buffer
    pub async fn read_to_buf(&mut self) -> anyhow::Result<usize> {
        Ok(self.reader.read_buf(&mut self.read_buf).await?)
    }

    /// Read a frame out of the read buffer, if not enough data to read a full frame,
    /// returns None.
    ///
    /// Advances read buffer by number of bytes consumed from decoding last frame.
    /// If decode frame successful, set new number of bytes to be advanced before decoding next
    /// frame.
    pub fn next_buffered_frame<'a, D: Deserialize<'a>>(&'a mut self) -> anyhow::Result<Option<D>> {
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

/// Write the buffer to the writer and clear it
pub async fn flush<T>(mut writer: T, write_buf: &mut Vec<u8>)
where
    T: AsyncWrite + Unpin,
{
    if !write_buf.is_empty() {
        writer.write_all(&write_buf[..]).await.unwrap();
        write_buf.clear();
    }
}
