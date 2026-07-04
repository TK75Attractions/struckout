use bytes::BytesMut;
use clap::Args;
use prost::Message;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use struckout_proto::{DetectionsPacket, UploadResult, read_packet_raw, upload_result};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
};

const TCP_PORT: &str = "0.0.0.0:6262";

const DB_PATH_DEFAULT: &str = "sqlite:///home/taichi765/.config/struckout/xtask.db";

#[derive(Args)]
pub struct SyncArgs {
    /// detectionsを保存するデータベース
    db_url: Option<String>,
}

impl SyncArgs {
    /// Returns `true` if an unrecoverable error occured and the proccess should exit with failure.
    pub async fn run(self) -> bool {
        let db_url = self.db_url.unwrap_or(DB_PATH_DEFAULT.to_string());
        let pool = match SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!("failed to connect to database at {}: {e:?}", db_url);
                return true;
            }
        };

        let listener = match TcpListener::bind(TCP_PORT).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("failed to listen TCP on {}: {e:?}", TCP_PORT);
                return true;
            }
        };
        println!("waiting for client...");
        let (mut stream, addr) = match listener.accept().await {
            Ok(ret) => ret,
            Err(e) => {
                eprintln!("failed to accept connection: {e:?}");
                return true;
            }
        };
        println!("connected with {:?}", addr);

        let res = handle_inputs(&pool, &mut stream).await;
        let mut res_packet = BytesMut::new();
        res.encode(&mut res_packet).unwrap();

        if let Err(e) = stream.write_all(&res_packet).await {
            eprintln!("failed to write result to the client: {e:?}");
            return true;
        };

        println!("succeed to sync frames");
        return false;
    }
}

async fn handle_inputs(pool: &Pool<Sqlite>, stream: &mut TcpStream) -> UploadResult {
    let total = match stream.read_u32_le().await {
        Ok(i) => i,
        Err(e) => {
            eprintln!("failed to read header: {e:?}");
            return UploadResult {
                data: Some(upload_result::Data::TcpError(e.to_string())),
            };
        }
    };
    println!("total: {}", total);

    for _ in 0..total {
        let mut raw = match read_packet_raw(stream).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("failed to read packet from stream: {e:?}");
                return UploadResult {
                    data: Some(upload_result::Data::TcpError(e.to_string())),
                };
            }
        };
        let packet = match DetectionsPacket::decode(&mut raw) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("failed to decode proto packet: {e:?}");
                return UploadResult {
                    data: Some(upload_result::Data::PacketDecodeError(e.to_string())),
                };
            }
        };

        let session_id = packet.session_id;
        let raw: &[u8] = &raw;
        let res = sqlx::query!(
            "INSERT INTO frames (timestamp, session_id, data) VALUES (?, ?, ?)",
            packet.timestamp,
            session_id,
            raw
        )
        .execute(pool)
        .await;

        if let Err(e) = res {
            eprintln!("failed to insert data into database: {e:?}");
            return UploadResult {
                data: Some(upload_result::Data::DbInsertError(e.to_string())),
            };
        };
    }

    UploadResult {
        data: Some(upload_result::Data::Success(())),
    }
}
