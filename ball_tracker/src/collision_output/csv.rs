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
        while let Some(coll) = collision_rx.recv().await {
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
    use super::*;

    #[tokio::test]
    async fn writes_each_collision_as_a_separate_csv_record() {
        let directory = tempfile::tempdir().expect("temporary directory should be available");
        let path = directory.path().join("collisions.csv");
        let (sender, receiver) = mpsc::channel(2);
        let writer = tokio::spawn(CsvCollisionOutput::new(&path).start(receiver));
        let points = [[1.0, -2.5, 3.25], [4.5, 5.0, -6.75]];
        for [x, y, z] in points {
            sender
                .send(CollisionPoint3D { x, y, z })
                .await
                .expect("CSV writer should accept collisions");
        }
        drop(sender);
        let result = writer.await;
        let contents = fs::read_to_string(path)
            .await
            .expect("CSV file should be readable after writing");
        let rows: Vec<Vec<f64>> = contents
            .lines()
            .map(|line| {
                line.split(',')
                    .map(|value| value.parse().expect("CSV coordinates should be numbers"))
                    .collect()
            })
            .collect();
        assert_eq!(rows, points.map(Vec::from));
        assert!(contents.ends_with('\n'));
        result.expect("CSV writer should finish when all senders close");
    }
}
