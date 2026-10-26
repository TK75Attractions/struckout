//! カメラから受け取った動画を、この PC で解析する (TCP の 5052 番)。
//!
//! カメラアプリは高速撮影の動画を H.264 にしてそのまま送ってくる。ここでデコードして縮小し、アプリと同じ方法で
//! 解析して、アプリが送るのと同じ [`struckout_proto::DetectionsPacket`] を作る。これより先 (時刻合わせ、追跡) は
//! アプリから検出を受け取ったときと変わらない。
//!
//! スマホで解析すると 1 枚 17ms ほどかかり、120fps に追いつかない (KYV45)。

pub mod analysis;
pub mod decode;
pub mod hwaccel;
mod pipeline;
pub mod recorder;
pub mod tracker;
mod transport;
use std::path::PathBuf;

pub use transport::{VIDEO_ADDR_DEFAULT, VideoTransport};

/// PC で解析するときの設定。
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// ffmpeg の実行ファイル (PATH にあれば名前だけでよい)
    pub ffmpeg: String,
    /// 指定すると、解析を始めるたびに最初の解析画像をこのフォルダーに BMP で保存する
    /// (画像の向きや範囲が、カメラアプリの画面と合っているかの確認用)
    pub snapshot_dir: Option<PathBuf>,
    /// 指定すると、受け取った動画をこのフォルダーに保存する (あとで解析し直すため。[`recorder`])
    pub record_dir: Option<PathBuf>,
    /// デコードに使う GPU の方式 (ffmpeg の `-hwaccel`)。`None` なら CPU でデコードする。
    /// [`hwaccel::select`] で、その PC で使えるものを選んでおく
    pub hwaccel: Option<String>,
    /// 検出の位置の取り方と、画像の端に接した検出の扱い
    pub detection: analysis::DetectionOptions,
}
