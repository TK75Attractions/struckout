use anyhow::Context;
use bytes::BytesMut;
use clap::Args;
use prost::{DecodeError, Message};
use sqlx::sqlite::SqlitePoolOptions;
use struckout_proto::{UdpPacket, read_packet_raw};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt},
    net::TcpListener,
};

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
            let mut raw = read_packet_raw(&mut stream)
                .await
                .with_context(|| "failed to read packet")?;
            let packet = UdpPacket::decode(&mut raw).with_context(|| "failed to decode packet")?;
            let raw: &[u8] = &raw;
            sqlx::query!("INSERT INTO frames VALUES (?, ?)", packet.timestamp, raw)
                .execute(&pool)
                .await
                .with_context(|| "failed to insert received frame into database")?;
        }

        stream
            .write_u32(0)
            .await
            .with_context(|| "failed to write delimiter")?;
        println!("succeed to sync frames");
        Ok(())
    }
}
