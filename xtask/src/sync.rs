use anyhow::Context;
use bytes::BytesMut;
use clap::Args;
use prost::{DecodeError, Message};
use sqlx::sqlite::SqlitePoolOptions;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt},
    net::TcpListener,
};

use crate::proto;

const TCP_PORT: &str = "0.0.0.0:6262";

#[derive(Args)]
pub struct SyncArgs {}

impl SyncArgs {
    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(TCP_PORT)
            .await
            .with_context(|| format!("failed to bind port to {}", TCP_PORT))?;
        println!("waiting for client...");
        let (mut stream, addr) = listener
            .accept()
            .await
            .with_context(|| "failed to accept connection")?;
        println!("connected with {:?}", addr);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite:///home/taichi765/.config/struckout/xtask.db")
            .await
            .with_context(|| "failed to conenct to database")?;

        let data_length = stream
            .read_u32_le()
            .await
            .with_context(|| "failed to read header")?;
        println!("data length: {}", data_length);

        for _ in 0..data_length {
            let (packet, raw): (proto::UdpPacket, _) = read_packet(&mut stream)
                .await
                .with_context(|| "failed to read packet")?;
            println!("{:?}", packet.timestamp);
            let raw: &[u8] = &raw;
            sqlx::query!("INSERT INTO frames VALUES (?, ?)", packet.timestamp, raw)
                .execute(&pool)
                .await
                .with_context(|| "failed to insert received frame into database")?;
        }

        stream
            .write_u8(0)
            .await
            .with_context(|| "failed to write delimiter")?;
        println!("succeed to sync frames");
        Ok(())
    }
}

/// Reads a protobuf message from `input`.
///
/// Note that this function allocates buffer every time so it might not be efficient when the function is called frequently.
async fn read_packet<T: Message + Default, I: AsyncRead + Unpin>(
    input: &mut I,
) -> Result<(T, BytesMut), ReadPacketError> {
    let len = input.read_u32_le().await?;
    let mut buf = BytesMut::zeroed(len as usize);
    input.read_exact(&mut buf).await?;
    let packet = T::decode(&mut buf)?;
    Ok((packet, buf))
}

#[derive(Debug, Error)]
enum ReadPacketError {
    #[error(transparent)]
    ReadFailed(#[from] std::io::Error),
    #[error(transparent)]
    DecodeFailed(#[from] DecodeError),
}
