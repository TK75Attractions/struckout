use anyhow::Context;
use prost::Message;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use struckout_proto::UdpPacket;
use tokio::sync::mpsc;
use tracing::warn;

use crate::detection_input::{DetectionInput, db::FrameEntity};

use super::PairedFrames;
use itertools::Itertools as _;

pub struct SqliteDetectionInput {}

impl DetectionInput for SqliteDetectionInput {
    async fn start(self, tx: mpsc::Sender<PairedFrames>) -> std::io::Result<()> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("~/.config/struckout/dev.db") // TODO: 差し替え可能にする
            .await
            .unwrap();
        let frames = fetch_frames(&pool).await.unwrap();

        for (a, b) in frames.into_iter().tuples() {
            let data: &[u8] = &a.data;
            let a_packet = UdpPacket::decode(data).unwrap(); // TODO: handle error
            let data: &[u8] = &b.data;
            let b_packet = UdpPacket::decode(data).unwrap();
            if a.timestamp != a_packet.timestamp {
                warn!("invalid record: timestamp is different");
                continue;
            }
            if b.timestamp != b_packet.timestamp {
                warn!("invalid record: timestamp is different");
                continue;
            }
            let p = PairedFrames::new(a_packet, b_packet);
            tx.send(p)
                .await
                .with_context(|| "frame channel has been unexpectedly closed")
                .unwrap();
        }
        Ok(())
    }
}

async fn fetch_frames(pool: &Pool<Sqlite>) -> Result<Vec<FrameEntity>, sqlx::Error> {
    let frames = sqlx::query_as!(
        FrameEntity,
        "
            SELECT * FROM frames f1
            WHERE EXISTS (
                SELECT 1 FROM frames f2
                WHERE f2.timestamp BETWEEN f1.timestamp - 5 AND f1.timestamp + 5
                AND f2.session_id <> f1.session_id
            )
            ORDER BY timestamp
        "
    )
    .fetch_all(pool)
    .await?;
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use bytes::BytesMut;
    use chrono::{DateTime, Datelike, TimeDelta, TimeZone, Timelike, Utc};
    use futures::{StreamExt, stream};
    use prost::Message;
    use rand::seq::SliceRandom;
    use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
    use tempfile::NamedTempFile;

    use super::*;

    const SESSION_ID_1: &str = "87f97563-4af2-4052-827b-440351764edc";
    const SESSION_ID_2: &str = "c67a452e-9dfa-45a9-a205-2d7f25b66a77";

    async fn insert_sample_data(pool: &Pool<Sqlite>) {
        let mut fixtures = vec![
            // session_id, millis
            (1, 511),
            (2, 513),
            (1, 527),
            (2, 530),
            (1, 546),
            (2, 546),
        ];
        let mut rng = rand::rng();
        fixtures.shuffle(&mut rng);

        tokio_stream::iter(fixtures)
            .for_each(|(session_id, timestamp_millis)| async move {
                let time = Utc.with_ymd_and_hms(2026, 7, 15, 12, 30, 5).unwrap()
                    + TimeDelta::milliseconds(timestamp_millis);
                let timestamp = time.timestamp_millis();

                let mut buf = BytesMut::new();
                UdpPacket {
                    camera_id: 0,
                    timestamp,
                    frame_id: 0,
                    detected_objects: Vec::new(),
                }
                .encode(&mut buf)
                .unwrap();
                let buf: &[u8] = &buf;

                let sessino_id = match session_id {
                    1 => SESSION_ID_1,
                    2 => SESSION_ID_2,
                    _ => panic!("can't map unknown session id"),
                };
                sqlx::query!(
                    "INSERT INTO frames (timestamp, session_id, data) VALUES (?, ?, ?)",
                    timestamp,
                    session_id,
                    buf
                )
                .execute(pool)
                .await
                .unwrap();
            })
            .await;
    }

    #[tokio::test]
    async fn fetch_frames_fetches_in_right_order() {
        let file = NamedTempFile::new().unwrap();

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&file.path().to_string_lossy())
            .await
            .unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();
        insert_sample_data(&pool).await;

        let frames = fetch_frames(&pool).await.unwrap();

        assert_eq!(frames.len(), 6);
        assert!(frames[0].timestamp - frames[1].timestamp < 5);
        assert_ne!(frames[0].session_id, frames[1].session_id);

        assert!(frames[2].timestamp - frames[3].timestamp < 5);
        assert_ne!(frames[2].session_id, frames[3].session_id);

        assert!(frames[4].timestamp - frames[5].timestamp < 5);
        assert_ne!(frames[4].session_id, frames[5].session_id);
    }
}
