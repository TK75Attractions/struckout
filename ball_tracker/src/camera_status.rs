//! 接続と受信の状態を一か所で管理し、ランチャーに公開する。

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use struckout_proto::{
    CameraStatus, ListCameraStatusRequest, ListCameraStatusResponse,
    camera_status_service_server::CameraStatusService,
};
use tokio::time::Instant;
use tonic::{Request, Response, Status};

use crate::{camera_config::CameraConfig, types::CameraId};

const FPS_INTERVAL: Duration = Duration::from_secs(1);

struct Entry {
    status: CameraStatus,
    connections: usize,
    started: Instant,
    frames: u32,
}

impl Entry {
    fn new(camera_id: CameraId, name: String, device_id: String) -> Self {
        Self {
            status: CameraStatus {
                camera_id: camera_id.value(),
                name,
                device_id,
                ..Default::default()
            },
            connections: 0,
            started: Instant::now(),
            frames: 0,
        }
    }

    fn refresh_fps(&mut self, now: Instant) {
        if self.connections == 0 {
            self.status.fps = 0.0;
            return;
        }
        let elapsed = now.duration_since(self.started);
        if elapsed >= FPS_INTERVAL {
            // 区間を固定し、受信が止まっても問い合わせ時に 0 に進める。
            self.status.fps = if elapsed >= FPS_INTERVAL * 2 {
                0.0
            } else {
                f64::from(self.frames) / FPS_INTERVAL.as_secs_f64()
            };
            self.frames = 0;
            self.started =
                now - Duration::from_nanos((elapsed.as_nanos() % FPS_INTERVAL.as_nanos()) as u64);
        }
    }
}

/// カメラ設定を先に登録し、未接続の端末も必ず返す。
///
/// まだ接続していない端末は設定どおりの未接続を返す。設定自体がなければ、最初の接続までは空のリストになる。
/// 短い同期ロックだけを使い、ロック中に await しない。
pub struct CameraStatusStore {
    entries: Mutex<BTreeMap<u32, Entry>>,
    now: fn() -> DateTime<Utc>,
}

impl CameraStatusStore {
    pub fn new(cameras: Option<&CameraConfig>) -> Self {
        let entries = cameras
            .into_iter()
            .flat_map(|config| config.entries().iter().enumerate())
            .map(|(index, camera)| {
                let id = index as u32;
                (
                    id,
                    Entry::new(
                        CameraId::new(id),
                        camera.name.clone(),
                        camera.device_id.clone().unwrap_or_default(),
                    ),
                )
            })
            .collect();
        Self {
            entries: Mutex::new(entries),
            now: Utc::now,
        }
    }

    /// 接続の寿命に合わせて保持する。最後の接続が終わると切断時刻を記録する。
    pub fn connect(self: &Arc<Self>, id: CameraId, device_id: &str) -> CameraConnection {
        let mut entries = self.entries.lock();
        let entry = entries
            .entry(id.value())
            .or_insert_with(|| Entry::new(id, String::new(), String::new()));
        if !device_id.is_empty() {
            entry.status.device_id = device_id.to_owned();
        }
        if entry.connections == 0 {
            entry.started = Instant::now();
            entry.frames = 0;
            entry.status.fps = 0.0;
        }
        entry.connections += 1;
        entry.status.connected = true;
        CameraConnection {
            store: self.clone(),
            id,
        }
    }

    /// 生の受信時刻を保存する。解析できなかった映像も受信の確認には使う。
    pub fn received(&self, id: CameraId, at: DateTime<Utc>) {
        if let Some(entry) = self.entries.lock().get_mut(&id.value()) {
            entry.status.last_frame_at = Some(prost_types::Timestamp {
                seconds: at.timestamp(),
                nanos: at.timestamp_subsec_nanos() as i32,
            });
        }
    }

    /// 検出パケット、または解析を終えた映像フレームを 1 回だけ数える。
    ///
    /// FPS は接続後の固定 1 秒区間で集計し、問い合わせの頻度によらない値にする。
    pub fn processed(&self, id: CameraId) {
        if let Some(entry) = self.entries.lock().get_mut(&id.value())
            && entry.connections > 0
        {
            entry.refresh_fps(Instant::now());
            entry.frames += 1;
        }
    }

    pub fn list(&self) -> Vec<CameraStatus> {
        let mut entries = self.entries.lock();
        let now = Instant::now();
        entries
            .values_mut()
            .map(|entry| {
                entry.refresh_fps(now);
                entry.status.clone()
            })
            .collect()
    }

    /// 動画ログも RPC と同じ集計結果を使い、FPS の集計を重複させない。
    pub fn fps(&self, id: CameraId) -> f64 {
        let mut entries = self.entries.lock();
        let Some(entry) = entries.get_mut(&id.value()) else {
            return 0.0;
        };
        entry.refresh_fps(Instant::now());
        entry.status.fps
    }
}

pub struct CameraConnection {
    store: Arc<CameraStatusStore>,
    id: CameraId,
}

impl Drop for CameraConnection {
    fn drop(&mut self) {
        let mut entries = self.store.entries.lock();
        if let Some(entry) = entries.get_mut(&self.id.value()) {
            entry.connections -= 1;
            if entry.connections == 0 {
                entry.status.connected = false;
                let at = (self.store.now)();
                entry.status.disconnected_at = Some(prost_types::Timestamp {
                    seconds: at.timestamp(),
                    nanos: at.timestamp_subsec_nanos() as i32,
                });
                entry.status.fps = 0.0;
                entry.frames = 0;
            }
        }
    }
}

pub struct CameraStatusServiceImpl(pub Arc<CameraStatusStore>);

#[tonic::async_trait]
impl CameraStatusService for CameraStatusServiceImpl {
    async fn list_camera_status(
        &self,
        _request: Request<ListCameraStatusRequest>,
    ) -> Result<Response<ListCameraStatusResponse>, Status> {
        Ok(Response::new(ListCameraStatusResponse {
            cameras: self.0.list(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> CameraConfig {
        CameraConfig::parse(r#"{"cameras":[{"name":"left","device_id":"aaa","pose":[0,0,0,0,0,0]},{"name":"right","pose":[0,0,0,0,0,0]}]}"#).expect("テストの設定は有効")
    }

    #[test]
    fn includes_cameras_that_never_connected() {
        let store = CameraStatusStore::new(Some(&config()));
        let list = store.list();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (
                list[0].camera_id,
                list[0].name.as_str(),
                list[0].device_id.as_str()
            ),
            (0, "left", "aaa")
        );
        assert_eq!(list[1].camera_id, 1);
        assert!(list.iter().all(|c| !c.connected
            && c.fps == 0.0
            && c.last_frame_at.is_none()
            && c.disconnected_at.is_none()));
        assert!(CameraStatusStore::new(None).list().is_empty());
    }

    #[test]
    fn records_receive_and_last_disconnect_and_keeps_unknown_cameras() {
        let at = DateTime::from_timestamp(100, 0).expect("有効な時刻");
        let mut store = CameraStatusStore::new(Some(&config()));
        store.now = || DateTime::from_timestamp(200, 0).expect("有効な時刻");
        let store = Arc::new(store);
        let id = CameraId::new(0);
        let control = store.connect(id, "aaa");
        let data = store.connect(id, "");
        assert!(store.list()[0].connected);
        assert!(store.list()[0].last_frame_at.is_none());
        store.received(id, at);
        assert_eq!(
            store.list()[0].last_frame_at.as_ref().map(|t| t.seconds),
            Some(100)
        );
        drop(control);
        assert!(store.list()[0].connected);
        drop(data);
        let camera = &store.list()[0];
        assert!(!camera.connected);
        assert_eq!(
            camera.disconnected_at.as_ref().map(|t| t.seconds),
            Some(200)
        );
        let reconnect = store.connect(id, "aaa");
        assert!(store.list()[0].connected);
        assert_eq!(store.list()[0].disconnected_at, camera.disconnected_at);
        let unknown = store.connect(CameraId::new(2), "unknown");
        let list = store.list();
        assert_eq!(list[2].device_id, "unknown");
        assert!(list[2].name.is_empty());
        drop((reconnect, unknown));
    }

    #[tokio::test(start_paused = true)]
    async fn fps_expires_without_new_frames() {
        let store = Arc::new(CameraStatusStore::new(None));
        let id = CameraId::new(0);
        let connection = store.connect(id, "aaa");
        store.processed(id);
        store.processed(id);
        tokio::time::advance(FPS_INTERVAL).await;
        assert_eq!(store.list()[0].fps, 2.0);
        tokio::time::advance(FPS_INTERVAL).await;
        assert_eq!(store.list()[0].fps, 0.0);
        drop(connection);
        store.processed(id);
        assert_eq!(store.list()[0].fps, 0.0);
    }
}
