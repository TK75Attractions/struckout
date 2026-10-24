//! 縮小した 1 フレームを解析して、カメラアプリが送るのと同じ [`DetectionsPacket`] を作る。

use struckout_proto::{Detection, DetectionsPacket, VideoStreamConfig};

use super::tracker::{Blob, ObjectTracker, Rect, TrackerParams};

/// 検出から視線を作るときの、検出の位置の取り方。カメラアプリとは関係なく、PC の解析だけで使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionCenter {
    /// 枠の中心を整数に切り捨てる (カメラアプリと同じ)
    #[default]
    Box,
    /// 前景の画素の重心 (画素より細かい)
    Centroid,
}

impl std::str::FromStr for DetectionCenter {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text {
            "box" => Ok(Self::Box),
            "centroid" => Ok(Self::Centroid),
            _ => Err(format!("expected box or centroid, got {text}")),
        }
    }
}

/// 画像の端に接した検出 (ボールの一部が画像の外にあり、枠や重心がボールの中心からずれている) の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeDetections {
    /// そのまま使う (カメラアプリと同じ)
    #[default]
    Keep,
    /// 捨てる。視線どうしの食い違いは減るが、ボールがカメラの近くを通る球で視線が足りなくなる
    Drop,
    /// 欠けていない向きの幅をボールの直径とみなし、欠けていない側の縁から半径だけ内側を中心にする
    Extrapolate,
}

impl std::str::FromStr for EdgeDetections {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text {
            "keep" => Ok(Self::Keep),
            "drop" => Ok(Self::Drop),
            "extrapolate" => Ok(Self::Extrapolate),
            _ => Err(format!("expected keep, drop or extrapolate, got {text}")),
        }
    }
}

/// PC の解析だけで使う、検出の扱い方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DetectionOptions {
    pub center: DetectionCenter,
    pub edge: EdgeDetections,
}

/// 視線の計算と、検出を受け付ける範囲。カメラが送ってきた [`VideoStreamConfig`] から作る。
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisSettings {
    pub camera_id: u32,
    pub session_id: String,
    pub video_width: usize,
    pub video_height: usize,
    pub width: usize,
    pub height: usize,
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
    /// 端末の座標からカメラの座標への回転 (行優先)
    pub device_to_camera: [f64; 9],
    /// アプリの Detection Area (プレビューの左、上、右、下から無視する幅、%)
    pub ignored_percent: [f64; 4],
    pub tracker: TrackerParams,
}

impl AnalysisSettings {
    pub fn from_config(config: &VideoStreamConfig) -> Result<Self, String> {
        if config.video_width == 0 || config.video_height == 0 {
            return Err("video size is zero".into());
        }
        if config.analysis_width == 0 || config.analysis_height == 0 {
            return Err("analysis size is zero".into());
        }
        if config.fx <= 0.0 || config.fy <= 0.0 {
            return Err(format!(
                "focal length must be positive: fx={}, fy={}",
                config.fx, config.fy
            ));
        }
        let device_to_camera: [f64; 9] =
            config.device_to_camera.as_slice().try_into().map_err(|_| {
                format!(
                    "device_to_camera must have 9 elements, got {}",
                    config.device_to_camera.len()
                )
            })?;
        if config.foreground_persistence_frames == 0 {
            return Err("foreground_persistence_frames must be positive".into());
        }
        Ok(Self {
            camera_id: config.camera_id,
            session_id: config.session_id.clone(),
            video_width: config.video_width as usize,
            video_height: config.video_height as usize,
            width: config.analysis_width as usize,
            height: config.analysis_height as usize,
            fx: config.fx,
            fy: config.fy,
            cx: config.cx,
            cy: config.cy,
            device_to_camera,
            ignored_percent: [
                config.ignored_left_percent,
                config.ignored_top_percent,
                config.ignored_right_percent,
                config.ignored_bottom_percent,
            ],
            tracker: TrackerParams {
                accumulate_weight: config.accumulate_weight,
                binary_threshold: config.binary_threshold,
                min_contour_area: config.min_contour_area,
                foreground_persistence_frames: config.foreground_persistence_frames,
                min_color_shift: config.min_color_shift,
            },
        })
    }

    /// 検出の中心が、受け付ける範囲に入っているか。アプリの `DetectionArea.contains` と同じ。
    ///
    /// プレビューは解析画像を時計回りに 90 度回したものなので、プレビューの上下が解析画像の左右に、
    /// プレビューの右左が解析画像の上下に対応する。
    pub fn accepts(&self, rect: Rect) -> bool {
        let [left, top, right, bottom] = self.ignored_percent;
        let (w, h) = (self.width as f64, self.height as f64);
        let x = f64::from(rect.x) + f64::from(rect.width) / 2.0;
        let y = f64::from(rect.y) + f64::from(rect.height) / 2.0;
        x >= w * top / 100.0
            && x <= w * (1.0 - bottom / 100.0)
            && y >= h * right / 100.0
            && y <= h * (1.0 - left / 100.0)
    }

    /// 検出の枠から、端末の座標での視線を求める。アプリの `WorldDirectionCalculator.calc` と同じ。
    ///
    /// 枠の中心は整数に切り捨てる (アプリが `rect.x + rect.width / 2` を整数で計算しているため)。
    pub fn ray(&self, rect: Rect) -> [f64; 3] {
        self.ray_at(
            f64::from(rect.x + rect.width / 2),
            f64::from(rect.y + rect.height / 2),
        )
    }

    /// 画像の点 (u, v) の視線。
    pub fn ray_at(&self, u: f64, v: f64) -> [f64; 3] {
        let n = [(u - self.cx) / self.fx, (v - self.cy) / self.fy, 1.0];
        let r = &self.device_to_camera;
        // R^T n
        [
            r[0] * n[0] + r[3] * n[1] + r[6] * n[2],
            r[1] * n[0] + r[4] * n[1] + r[7] * n[2],
            r[2] * n[0] + r[5] * n[1] + r[8] * n[2],
        ]
    }

    pub fn detection(&self, rect: Rect) -> Detection {
        let [lay_x, lay_y, lay_z] = self.ray(rect);
        Detection {
            bbox_width: rect.width,
            bbox_height: rect.height,
            lay_x,
            lay_y,
            lay_z,
            image_x: ((f64::from(rect.x) + f64::from(rect.width) / 2.0) / self.width as f64) as f32,
            image_y: ((f64::from(rect.y) + f64::from(rect.height) / 2.0) / self.height as f64)
                as f32,
        }
    }

    /// [`DetectionOptions`] に従って検出を作る。重心を使うときは、画像の上での位置 (行ごとの露光の時刻に使う) も重心にする。
    pub fn detection_with(&self, blob: &Blob, options: DetectionOptions) -> Detection {
        let extrapolated = (options.edge == EdgeDetections::Extrapolate && blob.touches_edge)
            .then(|| self.extrapolated_center(blob))
            .flatten();
        match (options.center, extrapolated) {
            (DetectionCenter::Box, None) => self.detection(blob.rect),
            (_, Some(_)) | (DetectionCenter::Centroid, None) => {
                let (u, v) = extrapolated.unwrap_or(blob.centroid);
                let [lay_x, lay_y, lay_z] = self.ray_at(u, v);
                Detection {
                    bbox_width: blob.rect.width,
                    bbox_height: blob.rect.height,
                    lay_x,
                    lay_y,
                    lay_z,
                    image_x: ((u + 0.5) / self.width as f64) as f32,
                    image_y: ((v + 0.5) / self.height as f64) as f32,
                }
            }
        }
    }
}

impl AnalysisSettings {
    /// 画像の端で欠けたボールの中心の見積もり (画素の中心を整数とする座標)。1 辺だけで欠けているときに限る。
    /// 欠けていない向きの幅を直径とみなし、欠けていない側の縁から半径だけ内側を中心にする。
    fn extrapolated_center(&self, blob: &Blob) -> Option<(f64, f64)> {
        let r = blob.rect;
        let (w, h) = (self.width as i32, self.height as i32);
        let (left, top, right, bottom) =
            (r.x == 0, r.y == 0, r.x + r.width == w, r.y + r.height == h);
        let (cx, cy) = blob.centroid;
        let radius = |diameter: i32| f64::from(diameter - 1) / 2.0;
        match (left, top, right, bottom) {
            (false, false, false, true) if r.height < r.width => {
                Some((cx, f64::from(r.y) + radius(r.width)))
            }
            (false, true, false, false) if r.height < r.width => {
                Some((cx, f64::from(r.y + r.height - 1) - radius(r.width)))
            }
            (false, false, true, false) if r.width < r.height => {
                Some((f64::from(r.x) + radius(r.height), cy))
            }
            (true, false, false, false) if r.width < r.height => {
                Some((f64::from(r.x + r.width - 1) - radius(r.height), cy))
            }
            _ => None,
        }
    }
}

/// 1 台のカメラの解析。フレームを順に渡す。
pub struct Analyzer {
    settings: AnalysisSettings,
    tracker: ObjectTracker,
    frame_id: u64,
    options: DetectionOptions,
}

/// 1 フレームの解析結果。
pub struct AnalyzedFrame {
    /// 受け付けた検出。なければ `None` (アプリと同じく、検出のないフレームは送らない)
    pub packet: Option<DetectionsPacket>,
    /// 範囲の外で捨てた検出の数
    pub ignored: usize,
}

impl Analyzer {
    pub fn new(settings: AnalysisSettings) -> Self {
        let tracker = ObjectTracker::new(settings.tracker);
        Self {
            settings,
            tracker,
            frame_id: 0,
            options: DetectionOptions::default(),
        }
    }

    /// 検出の扱い方を変える (既定はカメラアプリと同じ)。
    pub fn with_options(mut self, options: DetectionOptions) -> Self {
        self.options = options;
        self
    }

    pub fn settings(&self) -> &AnalysisSettings {
        &self.settings
    }

    /// 設定を差し替える。解析の設定が変わったときだけ、背景を作り直す。
    pub fn update(&mut self, settings: AnalysisSettings) {
        if settings.tracker != self.tracker.params()
            || settings.width != self.settings.width
            || settings.height != self.settings.height
        {
            self.tracker = ObjectTracker::new(settings.tracker);
        }
        self.settings = settings;
    }

    /// `rgb` は解析画像の大きさ (幅 x 高さ x 3) の画像。センサーの向きのまま。
    pub fn analyze(
        &mut self,
        rgb: &[u8],
        sensor_timestamp_ns: i64,
        timestamp_ms: i64,
    ) -> AnalyzedFrame {
        let (w, h) = (self.settings.width, self.settings.height);
        let blobs = self.tracker.next_frame_blobs(rgb, w, h);
        let drop_edge = self.options.edge == EdgeDetections::Drop;
        let (accepted, ignored): (Vec<Blob>, Vec<Blob>) = blobs
            .into_iter()
            .partition(|b| self.settings.accepts(b.rect) && !(drop_edge && b.touches_edge));
        if accepted.is_empty() {
            return AnalyzedFrame {
                packet: None,
                ignored: ignored.len(),
            };
        }
        self.frame_id += 1;
        let packet = DetectionsPacket {
            camera_id: self.settings.camera_id,
            session_id: self.settings.session_id.clone(),
            timestamp: timestamp_ms,
            frame_id: self.frame_id,
            detections: accepted
                .iter()
                .map(|b| self.settings.detection_with(b, self.options))
                .collect(),
            sensor_timestamp_ns,
        };
        AnalyzedFrame {
            packet: Some(packet),
            ignored: ignored.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use super::*;

    fn settings() -> AnalysisSettings {
        // 既定の背面カメラの向き (センサーの向き 90 度)。アプリの defaultBackCameraRotation(90)
        AnalysisSettings {
            camera_id: 3,
            session_id: "s".into(),
            video_width: 1280,
            video_height: 720,
            width: 640,
            height: 360,
            fx: 482.0,
            fy: 482.0,
            cx: 320.0,
            cy: 180.0,
            device_to_camera: [0.0, -1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0],
            ignored_percent: [0.0; 4],
            tracker: TrackerParams::APP_DEFAULT,
        }
    }

    #[test]
    fn ray_of_the_image_center_points_out_of_the_back_of_the_device() {
        let s = settings();
        let ray = s.ray(Rect {
            x: 310,
            y: 170,
            width: 20,
            height: 20,
        });
        assert_relative_eq!(ray[0], 0.0);
        assert_relative_eq!(ray[1], 0.0);
        assert_relative_eq!(ray[2], -1.0);
    }

    #[test]
    fn ray_uses_the_truncated_center_like_the_app() {
        let s = settings();
        // 中心は 100 + 21 / 2 = 110 (整数)、50 + 9 / 2 = 54
        let ray = s.ray(Rect {
            x: 100,
            y: 50,
            width: 21,
            height: 9,
        });
        let n = [(110.0 - 320.0) / 482.0, (54.0 - 180.0) / 482.0];
        // R^T (nx, ny, 1) = (-ny, -nx, -1)
        assert_relative_eq!(ray[0], -n[1]);
        assert_relative_eq!(ray[1], -n[0]);
        assert_relative_eq!(ray[2], -1.0);
        let d = s.detection(Rect {
            x: 100,
            y: 50,
            width: 21,
            height: 9,
        });
        assert_relative_eq!(d.image_x, 110.5 / 640.0);
        assert_relative_eq!(d.image_y, 54.5 / 360.0);
    }

    #[test]
    fn detection_area_follows_the_rotated_preview() {
        let mut s = settings();
        // プレビューの上を 25% 無視 = 解析画像の左 25% を無視
        s.ignored_percent = [0.0, 25.0, 0.0, 0.0];
        assert!(!s.accepts(Rect {
            x: 100,
            y: 100,
            width: 10,
            height: 10
        }));
        assert!(s.accepts(Rect {
            x: 200,
            y: 100,
            width: 10,
            height: 10
        }));
        // プレビューの左を 50% 無視 = 解析画像の下半分を無視
        s.ignored_percent = [50.0, 0.0, 0.0, 0.0];
        assert!(!s.accepts(Rect {
            x: 200,
            y: 250,
            width: 10,
            height: 10
        }));
        assert!(s.accepts(Rect {
            x: 200,
            y: 100,
            width: 10,
            height: 10
        }));
    }

    #[test]
    fn extrapolates_the_center_of_a_ball_cut_off_by_the_image_edge() {
        let s = settings();
        // 下端 (高さ 360) で欠けたボール: 横幅 21 が直径。上の縁 (y = 345) から半径 10 だけ下が中心
        let blob = Blob {
            rect: Rect {
                x: 100,
                y: 345,
                width: 21,
                height: 15,
            },
            centroid: (110.0, 351.0),
            touches_edge: true,
        };
        assert_eq!(s.extrapolated_center(&blob), Some((110.0, 355.0)));
        let options = DetectionOptions {
            center: DetectionCenter::Centroid,
            edge: EdgeDetections::Extrapolate,
        };
        let d = s.detection_with(&blob, options);
        let expected = s.ray_at(110.0, 355.0);
        assert_relative_eq!(d.lay_x, expected[0]);
        assert_relative_eq!(d.lay_y, expected[1]);
        // 欠けていない (縦のほうが長い) ときや、角で 2 辺が欠けたときは推定しない
        let tall = Blob {
            rect: Rect {
                x: 100,
                y: 330,
                width: 15,
                height: 30,
            },
            ..blob
        };
        assert_eq!(s.extrapolated_center(&tall), None);
        let corner = Blob {
            rect: Rect {
                x: 0,
                y: 345,
                width: 21,
                height: 15,
            },
            ..blob
        };
        assert_eq!(s.extrapolated_center(&corner), None);
    }
}
