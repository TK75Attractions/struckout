use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use struckout_proto::{
    DetectionsPacket, ReadPacketError, TcpClientPacket, TcpServerPacket, read_packet,
    tcp_client_packet, tcp_server_packet, write_packet,
};
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream, tcp},
    sync::mpsc,
    task::{JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, trace_span, warn};

use crate::{
    CameraLocationStore,
    camera_config::CameraConfig,
    clock_sync::ClockSync,
    detection_input::{CameraFrame, DetectionInput},
    types::CameraId,
    video::{VideoOptions, VideoTransport},
};

const DATA_ADDR_DEFAULT: &str = concat!("0.0.0.0:", env!("CAMERA_DATA_PORT"));
const CAMERA_LOC_TCP_ADDR_DEFAULT: &str = concat!("0.0.0.0:", env!("CAMERA_CTRL_PORT"));
// PC で動画を解析するときは、2 台から 120fps で検出が届くことがある
const PAKCET_CHANNEL_BUF: usize = 64;
/// 接続した直後は、時刻合わせの要求をこの間隔で [`TIME_SYNC_BURST`] 回送る。
const TIME_SYNC_BURST_INTERVAL: Duration = Duration::from_millis(50);
const TIME_SYNC_BURST: u64 = 8;
/// そのあとの時刻合わせの間隔。端末の時計は PC と進み方が違うので、測り続ける。
const TIME_SYNC_INTERVAL: Duration = Duration::from_millis(500);
/// 返事のない要求がこの数だけたまり、この時間だけ返事が来なければ、時刻合わせに対応していない
/// 送信元とみなして要求を送るのをやめる。
const TIME_SYNC_MAX_UNANSWERED: usize = 5;
const TIME_SYNC_GIVE_UP_AFTER: Duration = Duration::from_secs(2);
/// つないだ直後に、端末が名乗る (`Hello`) のを待つ時間。名乗らない送信元 (古いアプリ、再生用のスクリプト) には、
/// これだけ待ってから、つないだ順の camera_id を振る。
const HELLO_TIMEOUT: Duration = Duration::from_millis(500);

pub struct NetworkDetectionInput {
    data_transport: DataTransport,
    tcp_transport: TcpTransport,
    /// カメラから動画を受け取って、この PC で解析する。`None` なら受け付けない
    video_transport: Option<VideoTransport>,
    clock: Arc<ClockSync>,
}

impl NetworkDetectionInput {
    /// `video` を渡すと、5052 番でカメラから動画を受け取り、この PC で解析する。
    /// `cameras` を渡すと、そこに書いた端末を見分けて、決まった camera_id と位置を使う ([`CameraConfig`])。
    pub async fn new(
        camera_locs: Arc<CameraLocationStore>,
        clock: Arc<ClockSync>,
        video: Option<VideoOptions>,
        cameras: Option<Arc<CameraConfig>>,
    ) -> Result<Self, NetworkDetectionInputCreationError> {
        let data_transport = DataTransport::new()
            .await
            .map_err(NetworkDetectionInputCreationError::Data)?;
        let tcp_transport = TcpTransport::new(camera_locs, clock.clone(), cameras)
            .await
            .map_err(NetworkDetectionInputCreationError::Tcp)?;
        let video_transport = match video {
            Some(video) => Some(
                VideoTransport::new(video)
                    .await
                    .map_err(NetworkDetectionInputCreationError::Video)?,
            ),
            None => None,
        };
        Ok(Self {
            data_transport,
            tcp_transport,
            video_transport,
            clock,
        })
    }
}

impl DetectionInput for NetworkDetectionInput {
    async fn start(
        mut self,
        frame_tx: mpsc::Sender<CameraFrame>,
        shutdown: CancellationToken,
    ) -> std::io::Result<()> {
        let (packet_tx, mut packet_rx) = mpsc::channel(PAKCET_CHANNEL_BUF);
        let mut tasks = JoinSet::new();
        if let Some(video_transport) = self.video_transport {
            let packet_tx = packet_tx.clone();
            let video_shutdown = shutdown.clone();
            tasks.spawn(async move { video_transport.listen(packet_tx, video_shutdown).await });
        }
        let data_shutdown = shutdown.clone();
        tasks.spawn(async move { self.data_transport.listen(packet_tx, data_shutdown).await });
        tasks.spawn(async move {
            while let Some((received_at, packet)) = packet_rx.recv().await {
                // フレームの時刻を決めるのはここだけ。
                if let Some(frame) = self.clock.frame(received_at, packet) {
                    frame_tx
                        .send(frame)
                        .await
                        .with_context(|| "frame channel has been unexpectedly closed")
                        .unwrap();
                }
            }
        });
        info!("starting TCP listener");
        let control_shutdown = shutdown.clone();
        tasks.spawn(async move { self.tcp_transport.listen(control_shutdown).await });

        shutdown.cancelled().await;
        while let Some(result) = tasks.join_next().await {
            if let Err(error) = result
                && !error.is_cancelled()
            {
                warn!(?error, "a network input task failed while shutting down");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum NetworkDetectionInputCreationError {
    #[error("failed to create TcpTransport: {:?}",.0)]
    Tcp(#[source] std::io::Error),
    #[error("failed to create DataTransport: {:?}",.0)]
    Data(#[source] std::io::Error),
    #[error("failed to create VideoTransport: {:?}",.0)]
    Video(#[source] std::io::Error),
}

/// UDP Socket to receive frames from cameras.
pub struct DataTransport {
    listener: TcpListener,
    join_handles: Vec<JoinHandle<Result<(), ReadPacketError>>>,
}

impl DataTransport {
    pub async fn new() -> std::io::Result<Self> {
        info!(
            port = DATA_ADDR_DEFAULT,
            "trying to bind port for `DataTransport`"
        );
        let listener = TcpListener::bind(DATA_ADDR_DEFAULT).await?;
        info!("succeed to bind port for `DataTransport`");
        Ok(Self {
            listener,
            join_handles: Vec::new(),
        })
    }

    pub async fn listen(
        &mut self,
        packet_tx: mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
        shutdown: CancellationToken,
    ) {
        let mut writers = Vec::new();
        loop {
            let accepted = tokio::select! {
                _ = shutdown.cancelled() => break,
                result = self.listener.accept() => result,
            };
            match accepted {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection in DataTransport");
                    let (reader, writer) = stream.into_split();

                    writers.push(writer);
                    let connection_shutdown = shutdown.clone();
                    let connection_packet_tx = packet_tx.clone();
                    let join = tokio::spawn(Self::handle_input(
                        connection_packet_tx,
                        reader,
                        connection_shutdown,
                    ));
                    self.join_handles.push(join);
                }
                Err(e) => {
                    warn!(error = ?e, "failed to accept a connection in DataTransport");
                }
            }
        }
        drop(writers);
        for handle in self.join_handles.drain(..) {
            if let Err(error) = handle.await
                && !error.is_cancelled()
            {
                warn!(
                    ?error,
                    "a camera data connection failed while shutting down"
                );
            }
        }
    }

    pub async fn handle_input(
        packet_tx: mpsc::Sender<(DateTime<Utc>, DetectionsPacket)>,
        mut reader: tcp::OwnedReadHalf,
        shutdown: CancellationToken,
    ) -> Result<(), ReadPacketError> {
        loop {
            let packet: DetectionsPacket = tokio::select! {
                _ = shutdown.cancelled() => return Ok(()),
                packet = read_packet(&mut reader) => packet?,
            };

            if packet_tx.send((Utc::now(), packet)).await.is_err() {
                return Ok(());
            }
        }
    }
}

/// TCP socket to receive camera location from cameras.
///
/// この接続で、端末の時計と PC の時計の差も測る。
pub struct TcpTransport {
    listener: TcpListener,
    join_handles: Vec<JoinHandle<()>>,
    camera_locs: Arc<CameraLocationStore>,
    clock: Arc<ClockSync>,
    /// 端末ごとの設定。書いてある端末には、書いた順の camera_id を振る
    cameras: Option<Arc<CameraConfig>>,
    /// 見分けられなかった端末に次に割り当てる番号。接続ごとに増やす。登録済みの位置の数から決めると、
    /// 2 台がほぼ同時に接続したときに同じ番号になる。
    next_camera_id: Arc<AtomicU32>,
}

impl TcpTransport {
    pub async fn new(
        camera_locs: Arc<CameraLocationStore>,
        clock: Arc<ClockSync>,
        cameras: Option<Arc<CameraConfig>>,
    ) -> std::io::Result<Self> {
        info!(
            port = CAMERA_LOC_TCP_ADDR_DEFAULT,
            "trying to bind port for `CameraLocationListener`"
        );
        let listener = TcpListener::bind(CAMERA_LOC_TCP_ADDR_DEFAULT).await?;
        info!("succeed to bind port for `CameraLocationListener`");
        Ok(Self {
            listener,
            join_handles: Vec::new(),
            camera_locs,
            clock,
            next_camera_id: Arc::new(AtomicU32::new(
                cameras.as_ref().map_or(0, |c| c.first_unknown_id()),
            )),
            cameras,
        })
    }

    pub async fn listen(&mut self, shutdown: CancellationToken) {
        loop {
            let _span = trace_span!("TcpTransport_listen");
            debug!("TcpTransport listening...");
            let accepted = tokio::select! {
                _ = shutdown.cancelled() => break,
                result = self.listener.accept() => result,
            };
            match accepted {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection for TcpTransport");
                    let connection_shutdown = shutdown.clone();
                    let join = tokio::spawn(Self::handle_connection(
                        stream,
                        self.cameras.clone(),
                        Arc::clone(&self.next_camera_id),
                        Arc::clone(&self.camera_locs),
                        Arc::clone(&self.clock),
                        connection_shutdown,
                    ));
                    self.join_handles.push(join);
                }
                Err(e) => {
                    warn!(error = ?e, "failed to accept a connection in TcpTransport");
                }
            }
        }
        for handle in self.join_handles.drain(..) {
            if let Err(error) = handle.await
                && !error.is_cancelled()
            {
                warn!(
                    ?error,
                    "a camera control connection failed while shutting down"
                );
            }
        }
    }

    async fn handle_connection(
        stream: TcpStream,
        cameras: Option<Arc<CameraConfig>>,
        next_camera_id: Arc<AtomicU32>,
        camera_locs: Arc<CameraLocationStore>,
        clock: Arc<ClockSync>,
        shutdown: CancellationToken,
    ) {
        // 時刻合わせは往復時間で精度が決まるので、小さなパケットをためずに送る。
        if let Err(e) = stream.set_nodelay(true) {
            warn!(error = ?e, "failed to set TCP_NODELAY");
        }
        let (mut reader, mut writer) = stream.into_split();

        // 端末が名乗るのを待つ。名乗らない送信元は camera_id を待っているので、こちらからは何も届かない
        let hello_result = tokio::select! {
            _ = shutdown.cancelled() => return,
            result = tokio::time::timeout(
                HELLO_TIMEOUT,
                read_packet::<TcpClientPacket, _>(&mut reader),
            ) => result,
        };
        let hello = match hello_result {
            Ok(Ok(TcpClientPacket {
                data: Some(tcp_client_packet::Data::Hello(hello)),
            })) => Some(hello),
            Ok(Ok(packet)) => {
                warn!(
                    ?packet,
                    "expected a Hello as the first packet; treating the camera as unknown"
                );
                None
            }
            Ok(Err(e)) => {
                info!(error = ?e, "camera disconnected before it was given a camera id");
                return;
            }
            Err(_) => None,
        };
        let device_id = hello.as_ref().map_or("", |hello| hello.device_id.as_str());
        let known = cameras
            .as_deref()
            .and_then(|cameras| cameras.find(device_id));
        let camera_id = match known {
            Some((camera_id, _)) => camera_id,
            None => next_camera_id.fetch_add(1, Ordering::Relaxed),
        };
        let managed = known.is_some();
        match (&hello, known) {
            (_, Some((_, entry))) => {
                info!(camera_id, name = entry.name, device_id, "camera connected")
            }
            (Some(hello), None) if cameras.is_some() => warn!(
                camera_id,
                device_id,
                model = hello.device_model,
                "camera connected, but its device_id is not in --cameras (run scripts/camera_remote.py config \
                 to record it); using the pose the camera sends"
            ),
            (Some(_), None) => info!(camera_id, device_id, "camera connected"),
            (None, _) => info!(camera_id, "camera connected (did not send a Hello)"),
        }
        // 前の接続で測った時計の差は使わない
        clock.reset_camera(camera_id);

        let packet = TcpServerPacket {
            data: Some(tcp_server_packet::Data::CameraId(camera_id)),
        };
        if let Err(e) = write_packet(packet, &mut writer).await {
            warn!(camera_id, error = ?e, "failed to send a camera id");
            return;
        }
        if let Some((_, entry)) = known {
            // 位置と焦点距離は PC が持っている値に合わせてもらう。スマホの設定が消えていても元に戻る
            let packet = TcpServerPacket {
                data: Some(tcp_server_packet::Data::CameraSettings(entry.settings())),
            };
            if let Err(e) = write_packet(packet, &mut writer).await {
                warn!(camera_id, error = ?e, "failed to send the camera settings");
                return;
            }
            debug!(camera_id, settings = ?entry.settings(), "sent the camera settings");
        }
        debug!(camera_id, "initialized connection");

        // 返事を待っている要求の、送った時刻。
        let pending = Arc::new(Mutex::new(HashMap::<u64, DateTime<Utc>>::new()));
        let last_response = Arc::new(Mutex::new(Instant::now()));
        let request_shutdown = shutdown.clone();
        let requests = tokio::spawn(Self::request_time_sync(
            writer,
            camera_id,
            Arc::clone(&pending),
            Arc::clone(&last_response),
            request_shutdown,
        ));

        loop {
            let packet = match tokio::select! {
                _ = shutdown.cancelled() => break,
                packet = read_packet::<TcpClientPacket, _>(&mut reader) => packet,
            } {
                Ok(packet) => packet,
                Err(ReadPacketError::ReadFailed(e)) => {
                    info!(camera_id, error = ?e, "camera disconnected");
                    break;
                }
                Err(ReadPacketError::DecodeFailed(e)) => {
                    // 長さは読めているので、このパケットだけ読み飛ばせる。
                    warn!(camera_id, error = ?e, "failed to decode a TcpClientPacket");
                    continue;
                }
            };
            match packet.data {
                Some(tcp_client_packet::Data::UpdateCameraPose(pose_data)) => {
                    let Some(camera_pose) = pose_data.camera_pose else {
                        warn!("camera_location field is missing for TcpClientPacket");
                        continue;
                    };
                    if managed {
                        // --cameras に書いた位置を使う (送ってきたのは、こちらが送った値のはず)
                        debug!(camera_id, value = ?camera_pose, "ignored the pose sent by a camera listed in --cameras");
                        continue;
                    }
                    info!(id = pose_data.camera_id, value = ?camera_pose, "camera pose updated");
                    camera_locs.insert(CameraId::new(pose_data.camera_id), camera_pose);
                }
                Some(tcp_client_packet::Data::TimeSyncResponse(response)) => {
                    let received_at = Utc::now();
                    let sent_at = pending.lock().remove(&response.sequence);
                    if let Some(sent_at) = sent_at {
                        *last_response.lock() = Instant::now();
                        clock.record_sync(camera_id, sent_at, received_at, &response);
                    }
                }
                Some(tcp_client_packet::Data::Hello(_)) => {
                    warn!(camera_id, "ignored a second Hello");
                }
                Some(tcp_client_packet::Data::CameraInfo(camera_info)) => {
                    info!(camera_id, value = ?camera_info, "camera info updated");
                    clock.record_info(camera_id, &camera_info);
                }
                None => {
                    warn!("TcpClientPacket was empty");
                }
            };
        }
        requests.abort();
        let _ = requests.await;
    }

    /// 時刻合わせの要求を送り続ける。
    async fn request_time_sync(
        mut writer: tcp::OwnedWriteHalf,
        camera_id: u32,
        pending: Arc<Mutex<HashMap<u64, DateTime<Utc>>>>,
        last_response: Arc<Mutex<Instant>>,
        shutdown: CancellationToken,
    ) {
        let mut sequence = 0;
        loop {
            let unanswered = pending.lock().len();
            if unanswered >= TIME_SYNC_MAX_UNANSWERED
                && last_response.lock().elapsed() >= TIME_SYNC_GIVE_UP_AFTER
            {
                info!(
                    camera_id,
                    "the camera does not answer time sync requests; using arrival times instead"
                );
                break;
            }
            let packet = TcpServerPacket {
                data: Some(tcp_server_packet::Data::TimeSyncRequest(
                    tcp_server_packet::TimeSyncRequest { sequence },
                )),
            };
            {
                let mut pending = pending.lock();
                // 返事の来なかった古い要求は忘れる。
                let oldest = sequence.saturating_sub(4 * TIME_SYNC_MAX_UNANSWERED as u64);
                pending.retain(|sent, _| *sent >= oldest);
                pending.insert(sequence, Utc::now());
            }
            if write_packet(packet, &mut writer).await.is_err() {
                return;
            }
            sequence += 1;
            tokio::select! {
                _ = shutdown.cancelled() => return,
                _ = tokio::time::sleep(if sequence < TIME_SYNC_BURST {
                    TIME_SYNC_BURST_INTERVAL
                } else {
                    TIME_SYNC_INTERVAL
                }) => {}
            }
        }
        // 書き込み側を閉じると、端末によっては接続全体が切れたとみなす。接続が終わるまで持っておく。
        std::future::pending::<()>().await;
        drop(writer);
    }
}
