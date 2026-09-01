use std::{io, sync::Arc};

use anyhow::Context;
use chrono::DateTime;
use struckout_proto::{
    DetectionsPacket, ReadPacketError, TcpClientPacket, TcpServerPacket, read_packet,
    tcp_client_packet, tcp_server_packet, write_packet,
};
use thiserror::Error;
use tokio::{
    net::{TcpListener, tcp},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::{debug, info, warn};

use crate::{
    CameraLocationStore,
    detection_input::{DetectionInput, FramePairMatcher, PairedFrames},
    types::CameraId,
};

const DATA_ADDR_DEFAULT: &str = "0.0.0.0:5050";
const CAMERA_LOC_TCP_ADDR_DEFAULT: &str = "0.0.0.0:6060";
const PAKCET_CHANNEL_BUF: usize = 5;

pub struct NetworkDetectionInput {
    data_transport: DataTransport,
    tcp_transport: TcpTransport,
    matcher: FramePairMatcher,
}

impl NetworkDetectionInput {
    pub async fn new(
        camera_locs: Arc<CameraLocationStore>,
    ) -> Result<Self, NetworkDetectionInputCreationError> {
        let data_transport = DataTransport::new()
            .await
            .map_err(|e| NetworkDetectionInputCreationError::Data(e))?;
        let tcp_transport = TcpTransport::new(camera_locs)
            .await
            .map_err(|e| NetworkDetectionInputCreationError::Tcp(e))?;
        Ok(Self {
            data_transport,
            tcp_transport,
            matcher: FramePairMatcher::new(),
        })
    }
}

impl DetectionInput for NetworkDetectionInput {
    async fn start(mut self, pair_tx: mpsc::Sender<PairedFrames>) -> std::io::Result<()> {
        let (packet_tx, mut packet_rx) = mpsc::channel(PAKCET_CHANNEL_BUF);
        tokio::spawn(async move { self.data_transport.listen(packet_tx).await });
        tokio::spawn(async move {
            loop {
                let packet = packet_rx
                    .recv()
                    .await
                    .with_context(|| "packet channel has been unexpectedly closed")
                    .unwrap();
                let time = DateTime::from_timestamp(packet.timestamp, 0).unwrap();
                let pair = self.matcher.pair_frame(time, packet);
                if let Some(pair) = pair {
                    pair_tx
                        .send(pair)
                        .await
                        .with_context(|| "pair channel has been unexpectedly closed")
                        .unwrap();
                }
            }
        });
        info!("starting TCP listener");
        tokio::spawn(async move { self.tcp_transport.listen().await });
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum NetworkDetectionInputCreationError {
    #[error("failed to create TcpTransport: {:?}",.0)]
    Tcp(#[source] std::io::Error),
    #[error("failed to create DataTransport: {:?}",.0)]
    Data(#[source] std::io::Error),
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

    pub async fn listen(&mut self, packet_tx: mpsc::Sender<DetectionsPacket>) {
        let mut writers = Vec::new();
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection in DataTransport");
                    let (reader, writer) = stream.into_split();

                    writers.push(writer);
                    let join = tokio::spawn(Self::handle_input(packet_tx.clone(), reader));
                    self.join_handles.push(join);
                }
                Err(_e) => {
                    todo!("handle errors")
                }
            }
        }
    }

    pub async fn handle_input(
        packet_tx: mpsc::Sender<DetectionsPacket>,
        mut reader: tcp::OwnedReadHalf,
    ) -> Result<(), ReadPacketError> {
        loop {
            let packet: DetectionsPacket = read_packet(&mut reader).await?;

            packet_tx.send(packet).await.unwrap();
        }
    }
}

/// TCP socket to receive camera location from cameras.
pub struct TcpTransport {
    listener: TcpListener,
    join_handles: Vec<JoinHandle<()>>,
    camera_locs: Arc<CameraLocationStore>,
}

impl TcpTransport {
    pub async fn new(camera_locs: Arc<CameraLocationStore>) -> std::io::Result<Self> {
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
        })
    }

    pub async fn listen(&mut self) {
        let mut writers = Vec::new();
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection for TcpTransport");
                    let (reader, mut writer) = stream.into_split();

                    self.init_conneciton(&mut writer).await;
                    debug!(?addr, "initialized connection");
                    writers.push(writer);

                    let join =
                        tokio::spawn(Self::handle_stream(Arc::clone(&self.camera_locs), reader));
                    self.join_handles.push(join);
                }
                Err(_e) => {
                    todo!("handle errors")
                }
            }
        }
    }

    async fn init_conneciton(&mut self, writer: &mut tcp::OwnedWriteHalf) {
        let next_camera_id = self.camera_locs.next() as u32;
        info!("camera id for new device is {}", next_camera_id);
        let packet = TcpServerPacket {
            data: Some(tcp_server_packet::Data::CameraId(next_camera_id)),
        };

        write_packet(packet, writer).await.unwrap();
    }

    async fn handle_stream(camera_locs: Arc<CameraLocationStore>, mut reader: tcp::OwnedReadHalf) {
        loop {
            let res = read_packet::<TcpClientPacket, _>(&mut reader).await;

            if let Err(ReadPacketError::ReadFailed(e)) = &res {
                match e.kind() {
                    io::ErrorKind::ConnectionReset => {
                        info!("connection reseted by peer");
                        break;
                    }
                    _ => (),
                }
            }
            let packet = res.unwrap();

            match packet.data {
                Some(tcp_client_packet::Data::CameraLoc(loc_data)) => {
                    if loc_data.camera_location.is_none() {
                        warn!("camera_location field is missing for TcpClientPacket");
                        continue;
                    }
                    let camera_loc = loc_data.camera_location.unwrap(); // checked above
                    info!(id = loc_data.camera_id, value = ?camera_loc, "camera location updated");
                    camera_locs.insert(CameraId::new(loc_data.camera_id), camera_loc);
                }
                None => {
                    warn!("TcpClientPacket was empty");
                }
            };
        }
    }
}
