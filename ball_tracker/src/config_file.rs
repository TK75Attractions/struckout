//! 設定ファイル (YAML) を読み、コマンドライン引数の形に直す。
//!
//! 設定ファイルのキーは長いオプション名 (`--` を除いたもの) で、値をそのまま引数にする。引数の定義を `Cli`
//! の 1 か所に保つため、設定ファイル用の構造体は作らない。引数を足しても、設定ファイル側を直す必要はない

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use serde_json::Value;
use thiserror::Error;

/// 設定ファイルの場所。`--config` で明示したものか、既定の場所か
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLocation {
    Explicit(PathBuf),
    /// `<dirs::config_dir()>/struckout/ball-tracker/config.yaml`。無ければ読まずに続ける
    Default(PathBuf),
}

impl ConfigLocation {
    /// コマンドライン引数 (先頭はプログラム名) から `--config` を探す。無ければ既定の場所。
    ///
    /// `Cli` を解釈する前に知る必要があるので、ここだけ自分で探す (clap の `ignore_errors` では、知らない引数の
    /// 後ろにある `--config` を拾えない)。`Cli` と同じく、複数あれば最後のものを使う
    pub fn from_args(args: &[OsString]) -> Option<Self> {
        let mut explicit = None;
        let mut args = args.iter().skip(1);
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--") => break,
                Some("--config") => explicit = args.next().map(PathBuf::from),
                Some(arg) => {
                    if let Some(path) = arg.strip_prefix("--config=") {
                        explicit = Some(PathBuf::from(path));
                    }
                }
                None => {}
            }
        }
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
    #[error("設定ファイル {path} を YAML として読めません: {source}")]
    Parse {
        path: PathBuf,
        // serde_saphyr::Error は大きいので、Result が大きくならないように Box に入れる
        #[source]
        source: Box<serde_saphyr::Error>,
    },
    #[error(
        "設定ファイル {path} の {key} には、文字列、数値、真偽値、null のどれかを書いてください"
    )]
    InvalidValue { path: PathBuf, key: String },
}

/// 設定ファイルを読み、コマンドライン引数の形にして返す。既定の場所にファイルが無ければ `None`
pub fn load_args(location: &ConfigLocation) -> Result<Option<Vec<OsString>>, LoadConfigError> {
    let path = location.path();
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return match location {
                ConfigLocation::Explicit(_) => Err(LoadConfigError::NotFound {
                    path: path.to_path_buf(),
                }),
                ConfigLocation::Default(_) => Ok(None),
            };
        }
        Err(source) => {
            return Err(LoadConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let values: BTreeMap<String, Value> =
        serde_saphyr::from_str(&content).map_err(|source| LoadConfigError::Parse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
    values
        .into_iter()
        .filter_map(|(key, value)| match value {
            // `--key=value` の形にして、負の数のように `-` で始まる値も 1 つの引数として渡す
            Value::String(value) => Some(Ok(format!("--{key}={value}").into())),
            Value::Number(value) => Some(Ok(format!("--{key}={value}").into())),
            Value::Bool(true) => Some(Ok(format!("--{key}").into())),
            Value::Bool(false) | Value::Null => None,
            Value::Array(_) | Value::Object(_) => Some(Err(LoadConfigError::InvalidValue {
                path: path.to_path_buf(),
                key,
            })),
        })
        .collect::<Result<_, _>>()
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    fn write_config(content: &str) -> (tempfile::TempDir, ConfigLocation) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, content).unwrap();
        (dir, ConfigLocation::Explicit(path))
    }

    #[test]
    fn converts_values_to_arguments() {
        let (_dir, location) = write_config(
            "input: network\nreport-delay-ms: -30\njson: true\nauto-time-trim: false\ncameras: null\n",
        );
        assert_eq!(
            load_args(&location).unwrap().unwrap(),
            os_args(&["--input=network", "--json", "--report-delay-ms=-30"])
        );
    }

    #[test]
    fn rejects_sequences_and_mappings() {
        for value in ["[network]", "{kind: network}"] {
            let (_dir, location) = write_config(&format!("input: {value}"));
            assert!(matches!(
                load_args(&location),
                Err(LoadConfigError::InvalidValue { key, .. }) if key == "input"
            ));
        }
    }

    #[test]
    fn missing_explicit_file_is_error_but_missing_default_file_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        assert!(matches!(
            load_args(&ConfigLocation::Explicit(path.clone())),
            Err(LoadConfigError::NotFound { .. })
        ));
        assert!(load_args(&ConfigLocation::Default(path)).unwrap().is_none());
    }

    #[test]
    fn finds_config_among_other_arguments() {
        for args in [
            &[
                "ball_tracker",
                "--input",
                "network",
                "--config",
                "a.yaml",
                "--json",
            ][..],
            &[
                "ball_tracker",
                "--input",
                "network",
                "--config=a.yaml",
                "--json",
            ],
        ] {
            assert_eq!(
                ConfigLocation::from_args(&os_args(args)),
                Some(ConfigLocation::Explicit("a.yaml".into()))
            );
        }
    }

    #[test]
    fn uses_default_location_without_config_argument() {
        assert!(matches!(
            ConfigLocation::from_args(&os_args(&["ball_tracker", "--input", "network"])),
            Some(ConfigLocation::Default(_)) | None
        ));
    }
}
