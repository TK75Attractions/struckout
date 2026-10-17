//! TCP の 5052 番で、カメラから動画を受け取る。
//!
//! 接続ごとに、[`VideoStreamServerPacket`] で引き受けたことを知らせ、そのあと [`VideoStreamPacket`] を読み続ける。
//! 設定 ([`VideoStreamConfig`]) を受け取ったら [`Pipeline`] を作り、フレームを渡していく。

use chrono::{DateTime, Utc};
use struckout_proto::{
    DetectionsPacket, ReadPacketError, VideoStreamConfig, VideoStreamPacket, VideoStreamServerPacket, read_packet,
    video_stream_packet, write_packet,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tracing::{info, warn};

use super::{VideoOptions, analysis::AnalysisSettings, pipeline::Pipeline, recorder::Recorder};

pub const VIDEO_ADDR_DEFAULT: &str = "0.0.0.0:5052";

pub struct VideoTransport {
    listener: TcpListener,
    options: VideoOptions,
}

impl VideoTransport {
    /// 5052 番で待ち受ける。
    pub async fn new(options: VideoOptions) -> std::io::Result<Self> {
        info!(port = VIDEO_ADDR_DEFAULT, "trying to bind port for `VideoTransport`");
        let listener = TcpListener::bind(VIDEO_ADDR_DEFAULT).await?;
        info!("succeed to bind port for `VideoTransport`");
        Ok(Self { listener, options })
    }

    /// 接続を受け付け続ける。解析した検出は、受け取った時刻と一緒に `packet_tx` に送る
    /// (アプリから検出を受け取ったときと同じ経路)。
    pub async fn listen(self, packet_tx: mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection in VideoTransport");
                    tokio::spawn(handle_connection(stream, self.options.clone(), packet_tx.clone()));
                }
                Err(e) => warn!(error = ?e, "failed to accept a connection in VideoTransport"),
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    options: VideoOptions,
    packet_tx: mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
) {
    if let Err(e) = stream.set_nodelay(true) {
        warn!(error = ?e, "failed to set TCP_NODELAY");
    }
    let (mut reader, mut writer) = stream.into_split();
    if let Err(e) = write_packet(VideoStreamServerPacket { accepted: true }, &mut writer).await {
        warn!(error = ?e, "failed to answer a video stream");
        return;
    }

    let mut pipeline: Option<Pipeline> = None;
    let mut recorder: Option<Recorder> = None;
    loop {
        let packet = match read_packet::<VideoStreamPacket, _>(&mut reader).await {
            Ok(packet) => packet,
            Err(ReadPacketError::ReadFailed(e)) => {
                info!(error = ?e, "video stream disconnected");
                break;
            }
            Err(ReadPacketError::DecodeFailed(e)) => {
                warn!(error = ?e, "failed to decode a VideoStreamPacket");
                continue;
            }
        };
        let received_at = Utc::now();
        if let (Some(dir), None, Some(video_stream_packet::Data::Config(config))) =
            (&options.record_dir, &recorder, &packet.data)
        {
            match Recorder::create(dir, config.camera_id, received_at) {
                Ok(created) => recorder = Some(created),
                Err(e) => warn!(error = ?e, dir = %dir.display(), "failed to start recording the video stream"),
            }
        }
        if let Some(recorder) = &recorder {
            recorder.record(received_at, &packet);
        }
        match packet.data {
            Some(video_stream_packet::Data::Config(config)) => {
                pipeline = apply_config(pipeline, &config, &options, &packet_tx);
            }
            Some(video_stream_packet::Data::Frame(frame)) => {
                if let Some(pipeline) = pipeline.as_mut() {
                    pipeline.push(frame, received_at);
                }
            }
            None => warn!("VideoStreamPacket was empty"),
        }
    }
    // pipeline を捨てると ffmpeg も止まる。recorder を捨てると、残りを書いてファイルを閉じる
    drop(pipeline);
    drop(recorder);
    // 書き込み側はここまで持っておく (先に閉じると、端末によっては接続全体が切れたとみなす)
    drop(writer);
}

/// 設定を当てる。解析画像の大きさが変わったときと、最初の設定では、ffmpeg を起動し直す。
fn apply_config(
    pipeline: Option<Pipeline>,
    config: &VideoStreamConfig,
    options: &VideoOptions,
    packet_tx: &mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
) -> Option<Pipeline> {
    let settings = match AnalysisSettings::from_config(config) {
        Ok(settings) => settings,
        Err(e) => {
            warn!(camera_id = config.camera_id, error = e, "ignored an invalid VideoStreamConfig");
            return pipeline;
        }
    };
    if let Some(pipeline) = pipeline {
        if pipeline.update_settings(settings.clone()) {
            return Some(pipeline);
        }
    }
    if config.mime_type != "video/avc" {
        warn!(camera_id = config.camera_id, mime_type = config.mime_type, "unsupported video format");
        return None;
    }
    info!(
        camera_id = config.camera_id,
        video = format!("{}x{} {}fps", config.video_width, config.video_height, config.frame_rate),
        analysis = format!("{}x{}", settings.width, settings.height),
        "received video stream config"
    );
    match Pipeline::spawn(options, settings, packet_tx.clone()) {
        Ok(pipeline) => Some(pipeline),
        Err(e) => {
            warn!(error = ?e, ffmpeg = options.ffmpeg, "failed to start ffmpeg; is it installed? (see --ffmpeg)");
            None
        }
    }
}
