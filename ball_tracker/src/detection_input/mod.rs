use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;
use struckout_proto::{Detection, DetectionsPacket};
use tokio::sync::mpsc;

mod db;
mod network;
pub use network::{NetworkDetectionInput, NetworkDetectionInputCreationError};

#[cfg(feature = "input-sqlite")]
mod sqlite;
#[cfg(feature = "input-sqlite")]
pub use sqlite::SqliteDetectionInput;

pub trait DetectionInput {
    fn start(
        self,
        tx: mpsc::Sender<CameraFrame>,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

/// 1 台のカメラの 1 フレームぶんの検出。
///
/// 2 台のフレームは組にしない。シャッターは同期しておらず、速い球は片方のカメラにしか映っていない時間が
/// 長いので、組にできたフレームだけを使うと視線の大半を捨てることになる。
#[derive(Debug, Clone, Serialize)]
pub struct CameraFrame {
    /// 撮影された (画像の最初の行の露光が始まった) 時刻。[`crate::clock_sync::ClockSync`] が
    /// この PC の時計に直したもので、ほかのカメラのフレームと比べられる。
    pub time: DateTime<Utc>,
    /// 画像の上端の行と下端の行の露光開始の時刻の差 (ns)。
    pub row_skew_ns: i64,
    /// このフレームを受け取った時刻。
    pub received_at: DateTime<Utc>,
    pub packet: DetectionsPacket,
}

impl CameraFrame {
    /// 検出 1 件が撮影された時刻。センサーは行を上から順に読み出すので、画像の下にあるものほど
    /// 遅く写っている。検出の位置を送ってこない送信元では、フレームの時刻と同じになる。
    pub fn detection_time(&self, detection: &Detection) -> DateTime<Utc> {
        let delay_ns = self.row_skew_ns as f64 * f64::from(detection.image_y);
        self.time + TimeDelta::nanoseconds(delay_ns.round() as i64)
    }
}
