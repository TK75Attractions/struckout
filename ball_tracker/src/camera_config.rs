//! PC が持つ、カメラ用スマホごとの設定 (`scripts/cameras.json`、`--cameras`)。
//!
//! スマホはつないだ直後に端末の識別子 (`Hello.device_id`) を送ってくる。ここに書いた端末なら、
//! - つないだ順によらず、書いた順の camera_id (0, 1, ...) を振る
//! - 位置と向きは、スマホが送ってくる値ではなく、ここに書いた値を使う
//! - 位置と向き、焦点距離、検出の範囲をスマホに送り、スマホの設定に入れてもらう
//!
//! ので、開く順番も、スマホの設定が消えることも気にしなくてよい。`device_id` は `scripts/camera-remote.ps1 config`
//! が端末から聞いて書き込む。

use std::path::Path;

use anyhow::Context;
use serde::Deserialize;
use struckout_proto::{CameraPose, DetectionArea, tcp_server_packet::CameraSettings};

#[derive(Debug, Deserialize)]
struct CameraConfigFile {
    cameras: Vec<CameraEntry>,
}

/// 1 台ぶんの設定。`cameras.json` のほかの項目 (adb のシリアル、出力先など) は使わない。
#[derive(Debug, Clone, Deserialize)]
pub struct CameraEntry {
    /// ログに出す名前 ("left" など)
    pub name: String,
    /// 端末の識別子 (`Settings.Secure.ANDROID_ID`)。無ければこの端末は見分けられない
    #[serde(default)]
    pub device_id: Option<String>,
    /// 焦点距離の、解析画像の幅に対する比。0 なら端末が返す値から計算する
    #[serde(default)]
    pub focal_ratio: f64,
    /// x, y, z (m)、回転 x, y, z (度)
    pub pose: [f64; 6],
    /// 検出を受け付ける範囲 (左、上、右、下の各辺から無視する幅、%)
    #[serde(default)]
    pub area: Option<[f64; 4]>,
}

impl CameraEntry {
    pub fn camera_pose(&self) -> CameraPose {
        let [
            x,
            y,
            z,
            rotation_x_degrees,
            rotation_y_degrees,
            rotation_z_degrees,
        ] = self.pose;
        CameraPose {
            x,
            y,
            z,
            rotation_x_degrees,
            rotation_y_degrees,
            rotation_z_degrees,
        }
    }

    /// スマホに送る設定。
    pub fn settings(&self) -> CameraSettings {
        CameraSettings {
            camera_pose: Some(self.camera_pose()),
            focal_length_ratio: self.focal_ratio,
            detection_area: self.area.map(|[left, top, right, bottom]| DetectionArea {
                left_percent: left,
                top_percent: top,
                right_percent: right,
                bottom_percent: bottom,
            }),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CameraConfig {
    entries: Vec<CameraEntry>,
}

impl CameraConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn parse(text: &str) -> anyhow::Result<Self> {
        // PowerShell で書いた JSON は先頭に BOM が付くことがある
        let file: CameraConfigFile = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
        Ok(Self {
            entries: file.cameras,
        })
    }

    /// 書いた順。添え字がそのまま camera_id になる。
    pub fn entries(&self) -> &[CameraEntry] {
        &self.entries
    }

    /// 端末の識別子から、camera_id と設定を引く。
    pub fn find(&self, device_id: &str) -> Option<(u32, &CameraEntry)> {
        if device_id.is_empty() {
            return None;
        }
        self.entries
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.device_id.as_deref() == Some(device_id))
            .map(|(index, entry)| (index as u32, entry))
    }

    /// 見分けられなかった端末に振る camera_id の始まり。ここに書いた端末の番号とは重ならない。
    pub fn first_unknown_id(&self) -> u32 {
        self.entries.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"{
        "description": ["ignored"],
        "package": "com.example",
        "cameras": [
            {"serial": "A", "name": "left", "device_id": "aaa", "output": "network", "focal_ratio": 0.97,
             "pose": [0.11, 0.12, 1.23, 136.9, -0.4, -32.3]},
            {"serial": "B", "name": "right", "output": "network", "focal_ratio": 0.97,
             "pose": [0.11, 1.06, 1.23, 135.1, -1.5, -148.3], "area": [1, 2, 3, 4]}
        ]
    }"#;

    #[test]
    fn finds_cameras_by_device_id_in_the_written_order() {
        let config = CameraConfig::parse(&format!("\u{feff}{JSON}")).unwrap();
        let (id, entry) = config.find("aaa").unwrap();
        assert_eq!((id, entry.name.as_str()), (0, "left"));
        assert_eq!(entry.camera_pose().y, 0.12);
        // device_id がまだ書かれていない端末と、空の識別子は見分けない
        assert!(config.find("").is_none());
        assert!(config.find("bbb").is_none());
        assert_eq!(config.first_unknown_id(), 2);
    }

    #[test]
    fn sends_the_area_only_when_written() {
        let config = CameraConfig::parse(JSON).unwrap();
        assert!(config.entries()[0].settings().detection_area.is_none());
        let area = config.entries()[1].settings().detection_area.unwrap();
        assert_eq!((area.left_percent, area.bottom_percent), (1.0, 4.0));
        assert_eq!(config.entries()[1].settings().focal_length_ratio, 0.97);
    }
}
