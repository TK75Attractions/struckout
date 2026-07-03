pub struct SqliteDetectionInput {}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use bytes::BytesMut;
    use chrono::{DateTime, Datelike, Timelike};
    use prost::Message;
    use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
    use struckout_proto::UdpPacket;
    use tempfile::NamedTempFile;

    use super::*;

    fn insert_sample_data(pool: Pool<Sqlite>) {
        let fixtures = vec![
            // session_id, millis
            (1, 511),
            (2, 513),
            (1, 527),
            (2, 530),
            (1, 540),
            (2, 548),
        ];
        fixtures.iter().for_each(|(session_id, timestamp_millis)| {
            let time = DateTime::UNIX_EPOCH
                .with_year(2025)
                .unwrap()
                .with_month(7)
                .unwrap()
                .with_day(15)
                .unwrap()
                .with_hour(12)
                .unwrap()
                .with_minute(30)
                .unwrap()
                .with_second(5)
                .unwrap()
                .with_nanosecond(timestamp_millis * 100_000)
                .unwrap()
                .timestamp_millis();
            let mut buf = BytesMut::new();
            UdpPacket {
                camera_id: 0,
                timestamp: time,
                frame_id: 0,
                detected_objects: Vec::new(),
            }
            .encode(&mut buf)
            .unwrap();
            let buf: &[u8] = &buf;
            sqlx::query!("INSERT INTO frames VALUES (?, ?)", time, buf);
        });
    }

    #[tokio::test]
    async fn sqlite_detection_input_works() {
        let file = NamedTempFile::with_suffix("db").unwrap();
        println!("tempfile created in {:?}", file.path());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&file.path().to_string_lossy())
            .await
            .unwrap();
        sqlx::migrate!("../xtask/migrations/")
            .run(&pool)
            .await
            .unwrap();

        let res = sqlx::query!(
            "
            SELECT * FROM frames f 
            WHERE EXISTS (
                SELECT 1 FROM frames f2
                WHERE f2.timestamp BETWEEN f.timestamp - 5 AND f.timestamp + 5
                AND f2.session_id <> f.session_id
            )
            ORDER BY timestamp
        "
        )
        .fetch_all(&pool)
        .await
        .unwrap();
    }
}
