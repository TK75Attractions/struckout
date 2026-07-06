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
        fs::create_dir_all(&self.csv_path)
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
            file.write_all(format!("{},{},{}", coll.x, coll.y, coll.z).as_bytes())
                .await
                .with_context(|| "failed to write collision to CSV file")
                .unwrap();
        }
    }
}
