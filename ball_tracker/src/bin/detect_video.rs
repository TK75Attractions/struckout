//! 録画した動画に、PC 側の解析 (`ball_tracker::video::tracker`) をかけて、検出を CSV に書く。
//!
//! `scripts/recording_analysis/detect_objects.py` (OpenCV を使う版) と同じ切り出し方・縮め方にしてあるので、
//! 2 つの結果を比べて、アプリの解析を正しく移せているかを確かめるのに使う。
//!
//! ```text
//! detect_video <動画> --first 1779 --last 4260 --min-color-shift 0 --persistence 15 --out out.csv
//! ```

use std::{
    io::{BufWriter, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

use ball_tracker::video::tracker::{ObjectTracker, TrackerParams};
use clap::Parser;

#[derive(Parser)]
struct Cli {
    video: PathBuf,
    /// 最初に処理するコマの番号 (動画の先頭を 0 として、表示順に数える)
    #[arg(long, default_value_t = 0)]
    first: u64,
    /// 最後に処理するコマの番号
    #[arg(long)]
    last: u64,
    #[arg(long, default_value_t = 640)]
    width: usize,
    #[arg(long, default_value_t = 360)]
    height: usize,
    #[arg(long, default_value_t = 10.0)]
    min_color_shift: f64,
    #[arg(long, default_value_t = 15)]
    persistence: u32,
    #[arg(long, default_value = "ffmpeg")]
    ffmpeg: String,
    #[arg(long)]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let (w, h) = (cli.width, cli.height);
    // detect_objects.py と同じく、縦持ちの印を無視してセンサーの向きのまま、先頭から復号して番号で選ぶ
    let mut child = Command::new(&cli.ffmpeg)
        .args(["-v", "fatal", "-display_rotation", "0", "-i"])
        .arg(&cli.video)
        .args([
            "-vf",
            &format!("select='between(n\\,{}\\,{})',scale={w}:{h}:flags=area", cli.first, cli.last),
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut tracker = ObjectTracker::new(TrackerParams {
        foreground_persistence_frames: cli.persistence,
        min_color_shift: cli.min_color_shift,
        ..TrackerParams::APP_DEFAULT
    });
    let mut out = BufWriter::new(std::fs::File::create(&cli.out)?);
    writeln!(out, "frame,x,y,width,height")?;
    let mut frame = vec![0u8; w * h * 3];
    let mut number = cli.first;
    let started = std::time::Instant::now();
    let mut analysis = std::time::Duration::ZERO;
    while stdout.read_exact(&mut frame).is_ok() {
        let begin = std::time::Instant::now();
        let rects = tracker.next_frame(&frame, w, h);
        analysis += begin.elapsed();
        for r in rects {
            writeln!(out, "{number},{},{},{},{}", r.x, r.y, r.width, r.height)?;
        }
        number += 1;
    }
    child.wait()?;
    let frames = number - cli.first;
    eprintln!(
        "{frames} フレーム、解析は平均 {:.2} ms/フレーム (全体 {:.1} 秒)",
        analysis.as_secs_f64() * 1000.0 / frames.max(1) as f64,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}
