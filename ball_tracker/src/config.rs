//! ball_tracker の設定。設定ファイル (YAML) から読み、コマンドライン引数で上書きする。

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use ball_tracker::video::analysis::{DetectionCenter, EdgeDetections};
use serde::Deserialize;
use thiserror::Error;

use crate::{Cli, CollisionOutputKind, DetectionInputKind};

/// 設定ファイルの場所。`--config` で明示したものか、既定の場所か
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLocation {
    Explicit(PathBuf),
    /// `<dirs::config_dir()>/struckout/ball-tracker/config.yaml`。無ければ読まずに続ける
    Default(PathBuf),
}

impl ConfigLocation {
    /// `--config` があればそれ、無ければ既定の場所
    pub fn new(explicit: Option<PathBuf>) -> Option<Self> {
        match explicit {
            Some(path) => Some(Self::Explicit(path)),
            None => dirs::config_dir().map(|dir| {
                Self::Default(
                    dir.join("struckout")
                        .join("ball-tracker")
                        .join("config.yaml"),
                )
            }),
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::Explicit(path) | Self::Default(path) => path,
        }
    }
}

/// 設定。各項目の意味は [`Cli`] の同じ名前のフィールドを見る。
///
/// 設定ファイルのキーは、コマンドライン引数の長い名前 (`--` を除いたもの) と同じにする。
/// 書かなかった項目は既定値になる。
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Config {
    /// [`Config::apply_cli`] の後は必ず `Some`
    #[serde(rename = "input")]
    pub detection_input: Option<DetectionInputKind>,
    /// [`Config::apply_cli`] の後は必ず `Some`
    #[serde(rename = "output")]
    pub collision_output: Option<CollisionOutputKind>,
    #[serde(rename = "json")]
    pub output_to_json: bool,
    #[serde(rename = "csv_path")]
    pub csv_path: Option<PathBuf>,
    pub time_trim_ms: f64,
    pub rolling_shutter_skew_ms: Option<f64>,
    pub ffmpeg: String,
    pub no_video_analysis: bool,
    pub video_snapshot_dir: Option<PathBuf>,
    pub video_record_dir: Option<PathBuf>,
    pub report_delay_ms: f64,
    pub auto_time_trim: bool,
    pub video_hwaccel: String,
    pub detection_center: DetectionCenter,
    pub edge_detections: EdgeDetections,
    pub cameras: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            detection_input: None,
            collision_output: None,
            output_to_json: false,
            csv_path: None,
            time_trim_ms: 0.0,
            rolling_shutter_skew_ms: None,
            ffmpeg: "ffmpeg".to_owned(),
            no_video_analysis: false,
            video_snapshot_dir: None,
            video_record_dir: None,
            report_delay_ms: -30.0,
            auto_time_trim: false,
            video_hwaccel: "auto".to_owned(),
            detection_center: DetectionCenter::default(),
            edge_detections: EdgeDetections::default(),
            cameras: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum LoadConfigError {
    #[error("設定ファイル {path} がありません")]
    NotFound { path: PathBuf },
    #[error("設定ファイル {path} を読めません: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("設定ファイル {path} を読めません: {source}")]
    Parse {
        path: PathBuf,
        // serde_saphyr::Error は大きいので、Result が大きくならないように Box に入れる
        #[source]
        source: Box<serde_saphyr::Error>,
    },
}

#[derive(Debug, Error)]
pub enum ApplyCliError {
    #[error("--{0} か、設定ファイルの {0} が必要です")]
    Missing(&'static str),
}

impl Config {
    /// 設定ファイルを読む。既定の場所にファイルが無ければ、すべて既定値にする
    pub fn from_file(location: &ConfigLocation) -> Result<Self, LoadConfigError> {
        let path = location.path();
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return match location {
                    ConfigLocation::Explicit(_) => Err(LoadConfigError::NotFound {
                        path: path.to_path_buf(),
                    }),
                    ConfigLocation::Default(_) => Ok(Self::default()),
                };
            }
            Err(source) => {
                return Err(LoadConfigError::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        serde_saphyr::from_str(&content).map_err(|source| LoadConfigError::Parse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })
    }

    /// コマンドライン引数で指定したものを、設定ファイルの値より優先する。
    ///
    /// フラグ (`--json` など) は、付けたときだけ有効にする。設定ファイルで `true` にしたものを、
    /// コマンドラインから無効にすることはできない
    pub fn apply_cli(self, cli: Cli) -> Result<Self, ApplyCliError> {
        let config = Self {
            detection_input: cli.detection_input.or(self.detection_input),
            collision_output: cli.collision_output.or(self.collision_output),
            output_to_json: cli.output_to_json || self.output_to_json,
            csv_path: cli.csv_path.or(self.csv_path),
            time_trim_ms: cli.time_trim_ms.unwrap_or(self.time_trim_ms),
            rolling_shutter_skew_ms: cli.rolling_shutter_skew_ms.or(self.rolling_shutter_skew_ms),
            ffmpeg: cli.ffmpeg.unwrap_or(self.ffmpeg),
            no_video_analysis: cli.no_video_analysis || self.no_video_analysis,
            video_snapshot_dir: cli.video_snapshot_dir.or(self.video_snapshot_dir),
            video_record_dir: cli.video_record_dir.or(self.video_record_dir),
            report_delay_ms: cli.report_delay_ms.unwrap_or(self.report_delay_ms),
            auto_time_trim: cli.auto_time_trim || self.auto_time_trim,
            video_hwaccel: cli.video_hwaccel.unwrap_or(self.video_hwaccel),
            detection_center: cli.detection_center.unwrap_or(self.detection_center),
            edge_detections: cli.edge_detections.unwrap_or(self.edge_detections),
            cameras: cli.cameras.or(self.cameras),
        };
        if config.detection_input.is_none() {
            return Err(ApplyCliError::Missing("input"));
        }
        if config.collision_output.is_none() {
            return Err(ApplyCliError::Missing("output"));
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn write_config(content: &str) -> (tempfile::TempDir, ConfigLocation) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, content).unwrap();
        (dir, ConfigLocation::Explicit(path))
    }

    #[test]
    fn reads_keys_named_after_long_options() {
        let (_dir, location) = write_config(
            "input: network\noutput: csv\njson: true\ncsv_path: a.csv\nreport-delay-ms: -10\ndetection-center: centroid\n",
        );
        let config = Config::from_file(&location).unwrap();
        assert_eq!(config.detection_input, Some(DetectionInputKind::Network));
        assert_eq!(config.collision_output, Some(CollisionOutputKind::Csv));
        assert!(config.output_to_json);
        assert_eq!(config.csv_path, Some(PathBuf::from("a.csv")));
        assert_eq!(config.report_delay_ms, -10.0);
        assert_eq!(config.detection_center, DetectionCenter::Centroid);
        // 書かなかった項目は既定値
        assert_eq!(config.ffmpeg, "ffmpeg");
        assert_eq!(config.edge_detections, EdgeDetections::Keep);
    }

    #[test]
    fn rejects_unknown_keys() {
        let (_dir, location) = write_config("input: network\nreport-delay: 10\n");
        assert!(matches!(
            Config::from_file(&location),
            Err(LoadConfigError::Parse { .. })
        ));
    }

    #[test]
    fn rejects_removed_sqlite_input() {
        let (_dir, location) = write_config("input: sqlite\noutput: network\n");
        assert!(matches!(
            Config::from_file(&location),
            Err(LoadConfigError::Parse { .. })
        ));
        assert!(Cli::try_parse_from(["ball_tracker", "--input", "sqlite"]).is_err());
    }

    #[test]
    fn missing_explicit_file_is_error_but_missing_default_file_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        assert!(matches!(
            Config::from_file(&ConfigLocation::Explicit(path.clone())),
            Err(LoadConfigError::NotFound { .. })
        ));
        assert_eq!(
            Config::from_file(&ConfigLocation::Default(path)).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn command_line_overrides_config_file() {
        let (_dir, location) = write_config(
            "input: network\noutput: csv\nreport-delay-ms: -10\nffmpeg: /opt/ffmpeg\n",
        );
        let cli = Cli::try_parse_from([
            "ball_tracker",
            "--output",
            "network",
            "--report-delay-ms",
            "5",
        ])
        .unwrap();
        let config = Config::from_file(&location)
            .unwrap()
            .apply_cli(cli)
            .unwrap();
        assert_eq!(config.detection_input, Some(DetectionInputKind::Network));
        assert_eq!(config.collision_output, Some(CollisionOutputKind::Network));
        assert_eq!(config.report_delay_ms, 5.0);
        assert_eq!(config.ffmpeg, "/opt/ffmpeg");
    }

    #[test]
    fn input_and_output_are_required_from_either_source() {
        let cli = Cli::try_parse_from(["ball_tracker", "--input", "network"]).unwrap();
        assert!(matches!(
            Config::default().apply_cli(cli),
            Err(ApplyCliError::Missing("output"))
        ));
    }
}
