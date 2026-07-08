use bytes::BytesMut;
use clap::Args;
use prost::Message;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use struckout_proto::{DetectionsPacket, UploadResult, read_packet_raw, upload_result};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, tcp},
    sync::mpsc,
    try_join,
};

const TCP_PORT: &str = "0.0.0.0:6262";

const DB_PATH_DEFAULT: &str = "sqlite:///home/taichi765/.config/struckout/dev.db";

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
        let (stream, addr) = match listener.accept().await {
            Ok(ret) => ret,
            Err(e) => {
                eprintln!("failed to accept connection: {e:?}");
                return true;
            }
        };
        println!("connected with {:?}", addr);
        let (reader, mut writer) = stream.into_split();

        let res = handle_inputs(pool, reader).await;
        let mut res_packet = BytesMut::new();
        res.encode(&mut res_packet).unwrap();

        if let Err(e) = writer.write_all(&res_packet).await {
            eprintln!("failed to write result to the client: {e:?}");
            return true;
        };

        if res
            .data
            .is_some_and(|d| matches!(d, upload_result::Data::Success(())))
        {
            println!("succeed to sync frames");
            return false;
        } else {
            return true;
        }
    }
}

async fn handle_inputs(pool: Pool<Sqlite>, mut stream: tcp::OwnedReadHalf) -> UploadResult {
    let total = match stream.read_u32_le().await {
        Ok(i) => i as usize,
        Err(e) => {
            eprintln!("failed to read header: {e:?}");
            return UploadResult {
                data: Some(upload_result::Data::TcpError(e.to_string())),
            };
        }
    };
    println!("total: {}", total);

    let (tx, rx) = mpsc::channel(total);
    let res = try_join! {
        read_inputs(stream, total, tx),
        insert_data(pool, rx)
    };
    match res {
        Ok(_) => UploadResult {
            data: Some(upload_result::Data::Success(())),
        },
        Err(e) => e,
    }
}

/// Invariant: [`UploadResult`] is never [`Success`][upload_result::Data::Success].
async fn read_inputs(
    mut stream: tcp::OwnedReadHalf,
    total: usize,
    tx: mpsc::Sender<(BytesMut, DetectionsPacket)>,
) -> Result<(), UploadResult> {
    for _ in 0..total {
        let raw = match read_packet_raw(&mut stream).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("failed to read packet from stream: {e:?}");
                return Err(UploadResult {
                    data: Some(upload_result::Data::TcpError(e.to_string())),
                });
            }
        };

        // OPTIM: ここのクローンどうにかならんかな
        let packet = match DetectionsPacket::decode(raw.clone()) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("failed to decode proto packet: {e:?}");
                return Err(UploadResult {
                    data: Some(upload_result::Data::PacketDecodeError(e.to_string())),
                });
            }
        };
        tx.send((raw, packet)).await.unwrap();
    }
    Ok(())
}

/// Invariant: [`UploadResult`] is always [`DbInsertError`][upload_result::Data::DbInsertError].
async fn insert_data(
    pool: Pool<Sqlite>,
    mut rx: mpsc::Receiver<(BytesMut, DetectionsPacket)>,
) -> Result<(), UploadResult> {
    while let Some((raw, packet)) = rx.recv().await {
        let session_id = packet.session_id;
        let raw: &[u8] = &raw;
        let res = sqlx::query!(
            "INSERT INTO frames (timestamp, session_id, data) VALUES (?, ?, ?)",
            packet.timestamp,
            session_id,
            raw
        )
        .execute(&pool)
        .await;

        if let Err(e) = res {
            eprintln!("failed to insert data into database: {e:?}");
            return Err(UploadResult {
                data: Some(upload_result::Data::DbInsertError(e.to_string())),
            });
        }
    }
    Ok(())
}
