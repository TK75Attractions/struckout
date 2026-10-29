//! TCP の 5052 番で、カメラから動画を受け取る。
//!
//! 接続ごとに、[`VideoStreamServerPacket`] で引き受けたことを知らせ、そのあと [`VideoStreamPacket`] を読み続ける。
//! 設定 ([`VideoStreamConfig`]) を受け取ったら [`Pipeline`] を作り、フレームを渡していく。

use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use struckout_proto::{
    DetectionsPacket, ReadPacketError, VideoStreamConfig, VideoStreamPacket,
    VideoStreamServerPacket, read_packet, video_stream_packet, write_packet,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{camera_status::CameraStatusStore, types::CameraId};

use super::{VideoOptions, analysis::AnalysisSettings, pipeline::Pipeline, recorder::Recorder};

pub const VIDEO_ADDR_DEFAULT: &str = concat!("0.0.0.0:", env!("CAMERA_VIDEO_PORT"));

pub struct VideoTransport {
    listener: TcpListener,
    options: VideoOptions,
    camera_status: Arc<CameraStatusStore>,
}

impl VideoTransport {
    /// 5052 番で待ち受ける。
    pub async fn new(
        options: VideoOptions,
        camera_status: Arc<CameraStatusStore>,
    ) -> std::io::Result<Self> {
        info!(
            port = VIDEO_ADDR_DEFAULT,
            "trying to bind port for `VideoTransport`"
        );
        let listener = TcpListener::bind(VIDEO_ADDR_DEFAULT).await?;
        info!("succeed to bind port for `VideoTransport`");
        Ok(Self {
            listener,
            options,
            camera_status,
        })
    }

    /// 接続を受け付け続ける。解析した検出は、受け取った時刻と一緒に `packet_tx` に送る
    /// (アプリから検出を受け取ったときと同じ経路)。
    pub async fn listen(
        self,
        packet_tx: mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
        shutdown: CancellationToken,
    ) {
        let mut connections = JoinSet::new();
        loop {
            let accepted = tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = connections.join_next(), if !connections.is_empty() => continue,
                result = self.listener.accept() => result,
            };
            match accepted {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection in VideoTransport");
                    connections.spawn(handle_connection(
                        stream,
                        self.options.clone(),
                        self.camera_status.clone(),
                        packet_tx.clone(),
                        shutdown.clone(),
                    ));
                }
                Err(e) => warn!(error = ?e, "failed to accept a connection in VideoTransport"),
            }
        }
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result
                && !error.is_cancelled()
            {
                warn!(?error, "a video connection failed while shutting down");
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    options: VideoOptions,
    camera_status: Arc<CameraStatusStore>,
    packet_tx: mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
    shutdown: CancellationToken,
) {
    if let Err(e) = stream.set_nodelay(true) {
        warn!(error = ?e, "failed to set TCP_NODELAY");
    }
    let (mut reader, mut writer) = stream.into_split();
    if let Err(e) = write_packet(VideoStreamServerPacket { accepted: true }, &mut writer).await {
        warn!(error = ?e, "failed to answer a video stream");
        return;
    }

    let mut connections = HashMap::new();
    let mut current_camera = None;
    let mut pipeline: Option<Pipeline> = None;
    let mut recorder: Option<Recorder> = None;
    loop {
        let packet = match tokio::select! {
            _ = shutdown.cancelled() => break,
            packet = read_packet::<VideoStreamPacket, _>(&mut reader) => packet,
        } {
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
                Err(e) => {
                    warn!(error = ?e, dir = %dir.display(), "failed to start recording the video stream")
                }
            }
        }
        if let Some(recorder) = &recorder {
            recorder.record(received_at, &packet);
        }
        match packet.data {
            Some(video_stream_packet::Data::Config(config)) => {
                let id = CameraId::new(config.camera_id);
                connections
                    .entry(config.camera_id)
                    .or_insert_with(|| camera_status.connect(id, ""));
                current_camera = Some(id);
                pipeline = apply_config(pipeline, &config, &options, &packet_tx, &camera_status);
            }
            Some(video_stream_packet::Data::Frame(frame)) => {
                if !frame.codec_config
                    && let Some(id) = current_camera
                {
                    camera_status.received(id, received_at);
                }
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

/// 設定を当てる。動画か解析画像の大きさが変わったときと、最初の設定では、ffmpeg を起動し直す。
fn apply_config(
    pipeline: Option<Pipeline>,
    config: &VideoStreamConfig,
    options: &VideoOptions,
    packet_tx: &mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
    camera_status: &Arc<CameraStatusStore>,
) -> Option<Pipeline> {
    let settings = match AnalysisSettings::from_config(config) {
        Ok(settings) => settings,
        Err(e) => {
            warn!(
                camera_id = config.camera_id,
                error = e,
                "ignored an invalid VideoStreamConfig"
            );
            return pipeline;
        }
    };
    if let Some(pipeline) = pipeline {
        let updated = pipeline.update_settings(settings.clone());
        if updated {
            return Some(pipeline);
        }
    }
    if config.mime_type != "video/avc" {
        warn!(
            camera_id = config.camera_id,
            mime_type = config.mime_type,
            "unsupported video format"
        );
        return None;
    }
    info!(
        camera_id = config.camera_id,
        video = format!(
            "{}x{} {}fps",
            config.video_width, config.video_height, config.frame_rate
        ),
        analysis = format!("{}x{}", settings.width, settings.height),
        "received video stream config"
    );
    match Pipeline::spawn(options, settings, packet_tx.clone(), camera_status.clone()) {
        Ok(pipeline) => Some(pipeline),
        Err(e) => {
            warn!(error = ?e, ffmpeg = options.ffmpeg, "failed to start ffmpeg; is it installed? (see --ffmpeg)");
            None
        }
    }
}
