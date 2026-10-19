//! 動画のデコードに使う GPU の方式 (ffmpeg の `-hwaccel`) を選ぶ。
//!
//! PC① の機種は決まっていないので、特定の GPU に決め打ちしない。起動時に、ffmpeg が対応している方式
//! (`ffmpeg -hwaccels`) のうち、その PC で実際に初期化できるもの (`-init_hw_device`) を、OS ごとの順に試す。
//!
//! - Windows: `d3d11va` (GPU のメーカーを問わない) → `cuda` (NVIDIA) → `qsv` (Intel) → `dxva2`
//! - macOS: `videotoolbox`
//! - Linux: `vaapi` → `cuda` → `qsv`
//!
//! 2026-10-16 に ARM64 の Windows (Qualcomm の GPU) で、`d3d11va` が CPU の 1 スレッドの約 4 倍の速さだった
//! (KYV45 の 1280x720 の録画、バッテリーで 45fps → 174fps)。

use std::process::{Command, Stdio};

use tracing::{info, warn};

#[cfg(target_os = "windows")]
const PREFERRED: &[&str] = &["d3d11va", "cuda", "qsv", "dxva2"];
#[cfg(target_os = "macos")]
const PREFERRED: &[&str] = &["videotoolbox"];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const PREFERRED: &[&str] = &["vaapi", "cuda", "qsv"];

/// `--video-hwaccel` の値から、使う方式を決める。
///
/// - `none`: CPU でデコードする (`None` を返す)
/// - `auto`: OS ごとの順に試し、最初に初期化できたもの。どれも使えなければ CPU
/// - それ以外: その名前の方式。初期化できなければ警告を出して CPU
pub fn select(ffmpeg: &str, requested: &str) -> Option<String> {
    if requested.eq_ignore_ascii_case("none") {
        info!("decoding video on the CPU (--video-hwaccel none)");
        return None;
    }
    let available = list(ffmpeg);
    let candidates: Vec<&str> = if requested.eq_ignore_ascii_case("auto") {
        PREFERRED.to_vec()
    } else {
        vec![requested]
    };
    for name in candidates {
        if !available.iter().any(|a| a == name) {
            if !requested.eq_ignore_ascii_case("auto") {
                warn!(
                    hwaccel = name,
                    ?available,
                    "this ffmpeg does not support the requested hwaccel; decoding on the CPU"
                );
            }
            continue;
        }
        if works(ffmpeg, name) {
            info!(hwaccel = name, ?available, "decoding video on the GPU");
            return Some(name.to_string());
        }
        warn!(
            hwaccel = name,
            "could not initialize the GPU decoder; trying the next one"
        );
    }
    info!(
        ?available,
        "no usable GPU decoder; decoding video on the CPU"
    );
    None
}

/// `ffmpeg -hwaccels` が並べる方式の名前。
fn list(ffmpeg: &str) -> Vec<String> {
    let Ok(output) = Command::new(ffmpeg)
        .args(["-hide_banner", "-hwaccels"])
        .stdin(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip_while(|line| !line.starts_with("Hardware acceleration methods"))
        .skip(1)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// その方式の装置を初期化できるか (GPU がない、ドライバーがないなどで失敗する)。
fn works(ffmpeg: &str, name: &str) -> bool {
    Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-init_hw_device",
            name,
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x64:d=0.04",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
