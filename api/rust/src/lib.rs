mod proto {
    include!(concat!(env!("OUT_DIR"), "/tk75attractions.struckout.v1.rs"));
}
use bytes::BytesMut;
use prost::{DecodeError, EncodeError, Message};
pub use proto::*;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

/// Writes a protobuf message to `output`.
///
/// Note that this function allocates buffer every time so it might not be efficient when the function is called frequently.
pub async fn write_packet<T: Message, O: AsyncWrite + Unpin>(
    packet: T,
    output: &mut O,
) -> Result<(), WritePacketError> {
    let mut buf = BytesMut::new();
    packet.encode(&mut buf)?;
    let len: u32 = buf
        .len()
        .try_into()
        .expect("packet size is too large so that it cannot be fit in header");

    output.write_all(&len.to_le_bytes()).await?;
    output.write_all(&buf).await?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum WritePacketError {
    #[error(transparent)]
    EncodeFailed(#[from] EncodeError),
    #[error(transparent)]
    WriteFailed(#[from] std::io::Error),
}

/// Reads a protobuf message from `input`.
///
/// Note that this function allocates buffer every time so it might not be efficient when the function is called frequently.
pub async fn read_packet<T: Message + Default, I: AsyncRead + Unpin>(
    input: &mut I,
) -> Result<T, ReadPacketError> {
    let mut buf = read_packet_raw(input).await?;
    let packet = T::decode(&mut buf)?;
    Ok(packet)
}

pub async fn read_packet_raw<I: AsyncRead + Unpin>(
    input: &mut I,
) -> Result<BytesMut, std::io::Error> {
    let len = input.read_u32_le().await?;
    let mut buf = BytesMut::zeroed(len as usize);
    input.read_exact(&mut buf).await?;
    Ok(buf)
}

#[derive(Debug, Error)]
pub enum ReadPacketError {
    #[error(transparent)]
    ReadFailed(#[from] std::io::Error),
    #[error(transparent)]
    DecodeFailed(#[from] DecodeError),
}

#[cfg(test)]
mod tests {

    #[test]
    fn data_len_is_serialized_correctly() {
        let len: u32 = 2000;
        let bytes = len.to_le_bytes();
        assert_eq!(bytes, [208, 7, 0, 0]);
    }

    #[test]
    fn data_len_is_deserialized_correctly() {
        let bytes: [u8; 4] = [0xd0, 0x07, 0, 0];
        let len = u32::from_le_bytes(bytes);
        assert_eq!(len, 2000);
    }
}
