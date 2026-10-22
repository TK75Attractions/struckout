//! 動画の解析 ([`Analyzer`]) が 1 フレームにかかる時間を、録画で測る。
//!
//! ```text
//! bench_analysis <録画 (.vsp)> [--skip 1200] [--frames 600] [--rounds 3] [--cache frames.raw]
//! ```
//!
//! 録画の `--skip` 枚目から `--frames` 枚を、その場の PC 解析と同じ ffmpeg の設定でデコードして、メモリに置く。
//! そのあと解析だけを `--rounds` 回くり返し、1 フレームあたりの時間を出す。デコードの時間は含まない。
//! 途中の区間はデコードし直すと時間がかかる (数分) ので、`--cache` を付けると、デコードした画像をそのファイルに
//! 書き、次からはそれを読む。
//!
//! 検出の結果 (枠、重心から作った視線、画像の上の位置) の指紋も出す。解析を速くしたときに、結果が
//! 変わっていないことを、この指紋が同じであることで確かめる。
//!
//! 120fps のカメラ 1 台の予算は 8.33 ms。その場では 2 台を別のスレッドで解析するので、`--threads 2` で
//! 2 台ぶんを同時に回すと、その場に近い負荷 (熱やメモリの帯域の取り合い) で測れる。

use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use ball_tracker::video::{
    analysis::{AnalysisSettings, Analyzer, DetectionCenter, DetectionOptions, EdgeDetections},
    recorder::RecordingReader,
};
use clap::Parser;
use struckout_proto::{DetectionsPacket, video_stream_packet};

#[derive(Parser)]
struct Cli {
    /// 録画 (`--video-record-dir` が書いた `.vsp`)
    recording: PathBuf,
    /// 読み飛ばすフレームの数 (投球のある区間から測るため)
    #[arg(long, default_value_t = 0)]
    skip: usize,
    /// 測るフレームの数 (640x360 で 1 枚 0.7 MB をメモリに置く)
    #[arg(long, default_value_t = 600)]
    frames: usize,
    /// 解析をくり返す回数
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    /// 同時に解析するスレッドの数 (その場ではカメラ 1 台に 1 本)
    #[arg(long, default_value_t = 1)]
    threads: usize,
    #[arg(long, default_value = "ffmpeg")]
    ffmpeg: String,
    /// デコードした画像を置くファイル。あれば読み、なければデコードして書く
    #[arg(long)]
    cache: Option<PathBuf>,
}

/// 録画の一部を、その場の PC 解析と同じ大きさと画素の形式 (RGB) にデコードする。`--cache` があれば使う。
fn load(cli: &Cli) -> Result<(AnalysisSettings, Vec<Vec<u8>>)> {
    let Some(cache) = &cli.cache else {
        return decode(cli);
    };
    if cache.exists() {
        let settings = read_settings(cli)?;
        let size = settings.width * settings.height * 3;
        let bytes =
            std::fs::read(cache).with_context(|| format!("cannot read {}", cache.display()))?;
        if bytes.len() % size != 0 {
            bail!(
                "{}: the size does not match {}x{} frames",
                cache.display(),
                settings.width,
                settings.height
            );
        }
        let images = bytes
            .chunks_exact(size)
            .take(cli.frames)
            .map(<[u8]>::to_vec)
            .collect();
        return Ok((settings, images));
    }
    let (settings, images) = decode(cli)?;
    std::fs::write(cache, images.concat())
        .with_context(|| format!("cannot write {}", cache.display()))?;
    Ok((settings, images))
}

/// 録画の最初の設定。
fn read_settings(cli: &Cli) -> Result<AnalysisSettings> {
    for item in RecordingReader::open(&cli.recording)? {
        if let Some(video_stream_packet::Data::Config(config)) = item?.1.data {
            return AnalysisSettings::from_config(&config).map_err(anyhow::Error::msg);
        }
    }
    bail!("{}: no VideoStreamConfig", cli.recording.display())
}

fn decode(cli: &Cli) -> Result<(AnalysisSettings, Vec<Vec<u8>>)> {
    let mut packets = RecordingReader::open(&cli.recording)?;
    let mut frames = Vec::new();
    let settings = loop {
        let Some(item) = packets.next() else {
            bail!("{}: no VideoStreamConfig", cli.recording.display());
        };
        match item?.1.data {
            Some(video_stream_packet::Data::Config(config)) => {
                break AnalysisSettings::from_config(&config).map_err(anyhow::Error::msg)?;
            }
            Some(video_stream_packet::Data::Frame(frame)) => frames.push(frame),
            None => {}
        }
    };
    let (width, height) = (settings.width, settings.height);
    let mut child = Command::new(&cli.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "h264",
            "-i",
            "pipe:0",
        ])
        .args([
            "-vf",
            &format!("scale={width}:{height}:flags=area"),
            "-fps_mode",
            "passthrough",
        ])
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("cannot start ffmpeg ({})", cli.ffmpeg))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = child.stdout.take().expect("piped stdout");
    // 書くのと読むのを分けないと、ffmpeg の出力がたまって止まる
    let writer = thread::spawn(move || -> Result<()> {
        for frame in frames {
            stdin.write_all(&frame.data)?;
        }
        for item in packets {
            if let Some(video_stream_packet::Data::Frame(frame)) = item?.1.data
                && stdin.write_all(&frame.data).is_err()
            {
                // 欲しい枚数を読み終えて、ffmpeg を止めた
                break;
            }
        }
        Ok(())
    });
    let mut images = Vec::with_capacity(cli.frames);
    let mut image = vec![0u8; width * height * 3];
    let mut decoded = 0;
    while images.len() < cli.frames && stdout.read_exact(&mut image).is_ok() {
        if decoded >= cli.skip {
            images.push(image.clone());
        }
        decoded += 1;
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = writer.join();
    if images.is_empty() {
        bail!(
            "{}: no frames after skipping {}",
            cli.recording.display(),
            cli.skip
        );
    }
    Ok((settings, images))
}

/// 検出の結果の指紋 (FNV-1a)。浮動小数はビットのまま混ぜるので、計算の順が変わっただけでも違いが出る。
fn fingerprint(hash: &mut u64, frame: usize, packet: &DetectionsPacket) {
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            *hash ^= u64::from(b);
            *hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    mix(&frame.to_le_bytes());
    for d in &packet.detections {
        mix(&d.bbox_width.to_le_bytes());
        mix(&d.bbox_height.to_le_bytes());
        for v in [d.lay_x, d.lay_y, d.lay_z] {
            mix(&v.to_bits().to_le_bytes());
        }
        mix(&d.image_x.to_bits().to_le_bytes());
        mix(&d.image_y.to_bits().to_le_bytes());
    }
}

struct Round {
    times: Vec<Duration>,
    frames_with_detections: usize,
    detections: usize,
    fingerprint: u64,
}

fn run_round(settings: &AnalysisSettings, images: &[Vec<u8>]) -> Round {
    // その場の設定 (scripts/pc1.json) と同じ
    let options = DetectionOptions {
        center: DetectionCenter::Centroid,
        edge: EdgeDetections::Extrapolate,
    };
    let mut analyzer = Analyzer::new(settings.clone()).with_options(options);
    let mut round = Round {
        times: Vec::with_capacity(images.len()),
        frames_with_detections: 0,
        detections: 0,
        fingerprint: 0xcbf2_9ce4_8422_2325,
    };
    for (i, image) in images.iter().enumerate() {
        let begin = Instant::now();
        let analyzed = analyzer.analyze(image, i as i64, i as i64);
        round.times.push(begin.elapsed());
        if let Some(packet) = analyzed.packet {
            round.frames_with_detections += 1;
            round.detections += packet.detections.len();
            fingerprint(&mut round.fingerprint, i, &packet);
        }
    }
    round
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let begin = Instant::now();
    let (settings, images) = load(&cli)?;
    println!(
        "{}: {} フレーム ({}x{}) を用意しました ({:.1} 秒)",
        cli.recording.display(),
        images.len(),
        settings.width,
        settings.height,
        begin.elapsed().as_secs_f64()
    );
    let mut fingerprints = Vec::new();
    for round in 1..=cli.rounds {
        let results: Vec<Round> = thread::scope(|scope| {
            let handles: Vec<_> = (0..cli.threads)
                .map(|_| scope.spawn(|| run_round(&settings, &images)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("analysis thread panicked"))
                .collect()
        });
        let mut times: Vec<Duration> = results
            .iter()
            .flat_map(|r| r.times.iter().copied())
            .collect();
        times.sort_unstable();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let mean = times.iter().map(|&d| ms(d)).sum::<f64>() / times.len() as f64;
        let pick = |p: f64| ms(times[((times.len() - 1) as f64 * p).round() as usize]);
        let first = &results[0];
        println!(
            "{round} 回目 ({} スレッド): 平均 {mean:.2} ms、中央値 {:.2} ms、95% {:.2} ms、最大 {:.2} ms / フレーム \
             (検出のあるフレーム {}、検出 {}、指紋 {:016x})",
            cli.threads,
            pick(0.5),
            pick(0.95),
            ms(*times.last().expect("at least one frame")),
            first.frames_with_detections,
            first.detections,
            first.fingerprint
        );
        fingerprints.extend(results.iter().map(|r| r.fingerprint));
    }
    if fingerprints.windows(2).any(|w| w[0] != w[1]) {
        bail!("同じ入力なのに、回によって検出の結果が違います");
    }
    Ok(())
}
