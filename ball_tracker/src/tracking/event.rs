use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::{
    detection_input::CameraFrame,
    tracking::{
        PredictedCollision, RejectReason, SeedDiagnostics, TrackId, ball_track::BallTrack,
        rays_per_camera,
    },
    types::CollisionPoint3D,
};

const JSON_LOGGER_FLASH_DURATION: Duration = Duration::minutes(10);

/// Logs tracking events.
pub trait EventLogger {
    fn push_events(&mut self, events: TrackingEventsDto);

    fn push_frame(&mut self, frame: &CameraFrame);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingEventsDto {
    /// イベントが起きた時刻 (この PC の時計)。着弾は、この時刻に報告を送り出した
    pub timestamp: DateTime<Utc>,
    pub events: Vec<TrackingEventBodyDto>,
}

/// トラックのその時点の状態。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSummaryDto {
    pub track_id: TrackId,
    /// カメラの番号ごとの視線の本数
    pub rays: BTreeMap<u32, usize>,
    /// 視線 1 本の誤差の見積もり (m)
    pub noise: f64,
    /// 予測している着弾。ボールとみなす条件を満たしていないときは `None`
    pub predicted_collision: Option<PredictedCollisionDto>,
    /// 条件を満たしていないときの、その理由
    pub rejected: Option<RejectReason>,
}

impl TrackSummaryDto {
    pub(super) fn new(
        track: &BallTrack,
        prediction: &Result<PredictedCollision, RejectReason>,
    ) -> Self {
        Self {
            track_id: track.id(),
            rays: rays_per_camera(track.rays()),
            noise: track.noise(),
            predicted_collision: prediction
                .as_ref()
                .ok()
                .map(|predicted| PredictedCollisionDto {
                    time: predicted.time,
                    y: predicted.point.y,
                    z: predicted.point.z,
                }),
            rejected: prediction.as_ref().err().copied(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedCollisionDto {
    pub time: DateTime<Utc>,
    pub y: f64,
    pub z: f64,
}

/// 報告した投球の、時刻合わせの手がかり。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrowDiagnosticsDto {
    pub track_id: TrackId,
    /// カメラの番号ごとの視線の本数
    pub rays: BTreeMap<u32, usize>,
    /// 視線と軌道の距離の二乗平均平方根 (m)
    pub residual_rms: f64,
    /// 右のカメラ (位置の y が大きいほう) の時刻を何 ms ずらすと、視線が軌道に最もよく乗るか
    pub best_time_shift_ms: i64,
    /// そのときの、視線と軌道の距離の二乗平均平方根 (m)
    pub best_residual_rms: f64,
}

/// 予測した着弾を報告しなかった理由。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SuppressReason {
    /// 直前に報告した着弾に近い。着弾のあとの跳ね返りや影
    TooSoonAfterReport,
    /// 同じころに着弾する、よりボールらしいトラックがある
    OutrankedByAnotherTrack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrackingEventBodyDto {
    NewTrack(TrackSummaryDto),
    /// 新しいトラックを探した結果。候補を 1 つも試さなかったフレームでは出さない
    SeedDiagnostics(SeedDiagnostics),
    /// トラックに視線を足した
    UpdateTrack(TrackSummaryDto),
    /// 視線が増えないまま時間がたったトラックを捨てた
    DropTrack(TrackId),
    /// 着弾を報告した
    Collision(HashMap<TrackId, CollisionPoint3D>),
    /// 報告した投球の、時刻合わせの手がかり
    ThrowDiagnostics(ThrowDiagnosticsDto),
    /// 予測した着弾を、重複とみなして報告しなかった
    SuppressedCollision {
        track_id: TrackId,
        reason: SuppressReason,
    },
}

/// Logs tracking events to local json files.
pub struct JsonEventLogger {
    dir: PathBuf,
    buf: Vec<TrackingEventsDto>,
    events_file: File,
    frames_file: File,
}

impl JsonEventLogger {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let events_file = {
            let fname = format!("events_{}", Local::now().format("%Y%m%d_%H%M"));
            let fpath = dir.join(fname);
            fs::create_dir_all(fpath.parent().unwrap()).expect("faild to create parent dirs");
            File::create(fpath).expect("failed to create file")
        };
        let frames_file = {
            let fname = format!("frames_{}", Local::now().format("%Y%m%d_%H%M"));
            let fpath = dir.join(fname);
            File::create(fpath).expect("failed to create frames file")
        };

        Self {
            dir,
            buf: Vec::new(),
            events_file,
            frames_file,
        }
    }
}

impl EventLogger for JsonEventLogger {
    fn push_events(&mut self, events: TrackingEventsDto) {
        serde_json::to_writer_pretty(&mut self.events_file, &events)
            .expect("failed to write events to file");
        self.events_file
            .flush()
            .expect("failed to flush events to file");
    }

    /// 受け取ったフレームを 1 行に 1 つずつ書く。あとから追跡をやり直すときの入力になる。
    fn push_frame(&mut self, frame: &CameraFrame) {
        serde_json::to_writer(&mut self.frames_file, &frame)
            .expect("failed to write a frame to file");
        self.frames_file
            .write_all(b"\n")
            .expect("failed to write a frame to file");
    }
}

/// [`EventLogger`] to stdout.
pub struct FmtEventLogger;

impl EventLogger for FmtEventLogger {
    fn push_events(&mut self, events: TrackingEventsDto) {
        trace!(?events, "new event");
    }

    fn push_frame(&mut self, _frame: &CameraFrame) {}
}
