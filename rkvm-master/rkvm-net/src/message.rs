use bincode::{DefaultOptions, Options};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub trait Message: Sized {
    async fn decode<R: AsyncRead + Send + Unpin>(stream: &mut R) -> Result<Self, Error>;

    async fn encode<W: AsyncWrite + Send + Unpin>(&self, stream: &mut W) -> Result<(), Error>;
}

impl<T: DeserializeOwned + Serialize + Sync> Message for T {
    async fn decode<R: AsyncRead + Send + Unpin>(stream: &mut R) -> Result<Self, Error> {
        let length = stream.read_u16().await?;

        let mut data = vec![0; length.into()];
        stream.read_exact(&mut data).await?;

        let data = options()
            .deserialize(&data)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;

        tracing::trace!("Read {} bytes", 2 + length);

        Ok(data)
    }

    async fn encode<W: AsyncWrite + Send + Unpin>(&self, stream: &mut W) -> Result<(), Error> {
        let data = options()
            .serialize(self)
            .map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;

        let length = data
            .len()
            .try_into()
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Data too large"))?;

        stream.write_u16(length).await?;
        stream.write_all(&data).await?;

        tracing::trace!("Wrote {} bytes", 2 + data.len());

        Ok(())
    }
}

/// Cancel-safe frame decoder.
///
/// `Message::decode` performs two awaited reads; dropping the future between
/// them would silently eat bytes and desynchronize the stream, which is fatal
/// when the read sits in a `tokio::select!` racing timers. This decoder keeps
/// partially received frames in an internal buffer and only consumes bytes
/// once a whole frame is available.
#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn recv<T: DeserializeOwned, R: AsyncRead + Send + Unpin>(
        &mut self,
        stream: &mut R,
    ) -> Result<T, Error> {
        loop {
            if self.buf.len() >= 2 {
                let length = u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize;
                if self.buf.len() >= 2 + length {
                    let data = &self.buf[2..2 + length];
                    let msg = options()
                        .deserialize(data)
                        .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
                    self.buf.drain(..2 + length);
                    tracing::trace!("Read {} bytes", 2 + length);
                    return Ok(msg);
                }
            }

            let read = stream.read_buf(&mut self.buf).await?;
            if read == 0 {
                return Err(Error::new(ErrorKind::UnexpectedEof, "stream closed"));
            }
        }
    }
}

fn options() -> impl Options {
    DefaultOptions::new().with_limit(u16::MAX.into())
}
