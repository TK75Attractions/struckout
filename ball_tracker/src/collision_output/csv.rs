use std::path::PathBuf;

use anyhow::Context;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    sync::mpsc,
};

use crate::{collision_output::CollisionOutput, types::CollisionPoint3D};

pub struct CsvCollisionOutput {
    csv_path: PathBuf,
}

impl CsvCollisionOutput {
    pub fn new(csv_path: impl Into<PathBuf>) -> Self {
        let csv_path = csv_path.into();
        Self { csv_path }
    }
}

impl CollisionOutput for CsvCollisionOutput {
    async fn start(self, mut collision_rx: mpsc::Receiver<CollisionPoint3D>) {
        fs::create_dir_all(self.csv_path.parent().unwrap())
            .await
            .with_context(|| {
                format!(
                    "failed to create parent directories for {}",
                    self.csv_path.display()
                )
            })
            .unwrap();
        let mut file = File::create(&self.csv_path)
            .await
            .with_context(|| format!("failed to create file at {}", self.csv_path.display()))
            .unwrap();
        loop {
            let coll = collision_rx
                .recv()
                .await
                .with_context(|| "collision channel has been unexpectedly closed")
                .unwrap();
            // 1 行に 1 件。改行がないと、次の着弾の x がこの z の後ろにつながって読めなくなる
            file.write_all(format!("{},{},{}\n", coll.x, coll.y, coll.z).as_bytes())
                .await
                .with_context(|| "failed to write collision to CSV file")
                .unwrap();
            file.flush().await.expect("failed to flush CSV file");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn writes_one_collision_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("collisions.csv");
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(CsvCollisionOutput::new(&path).start(rx));
        tx.send(CollisionPoint3D {
            x: 0.0,
            y: 0.5,
            z: 1.5,
        })
        .await
        .unwrap();
        tx.send(CollisionPoint3D {
            x: 0.0,
            y: 1.25,
            z: 2.0,
        })
        .await
        .unwrap();

        // 書き終わるのを待つ (1 件ごとに flush している)
        let mut text = String::new();
        for _ in 0..100 {
            text = fs::read_to_string(&path).await.unwrap_or_default();
            if text.lines().count() == 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(text, "0,0.5,1.5\n0,1.25,2\n");
    }
}
