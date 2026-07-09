use std::{io, sync::Arc};

use anyhow::Context;
use bytes::BytesMut;
use chrono::DateTime;
use prost::Message as _;
use struckout_proto::{
    DetectionsPacket, ReadPacketError, TcpClientPacket, TcpServerPacket, read_packet,
    tcp_client_packet, tcp_server_packet, write_packet,
};
use thiserror::Error;
use tokio::{
    net::{TcpListener, UdpSocket, tcp},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::{info, warn};

use crate::{
    CameraLocationStore,
    detection_input::{DetectionInput, FramePairMatcher, PairedFrames},
    types::CameraId,
};

const FRAME_UDP_ADDR_DEFAULT: &str = "0.0.0.0:5050";
const CAMERA_LOC_TCP_ADDR_DEFAULT: &str = "0.0.0.0:6060";
const PAKCET_CHANNEL_BUF: usize = 5;

pub struct NetworkDetectionInput {
    udp_transport: UdpTransport,
    tcp_transport: TcpTransport,
    matcher: FramePairMatcher,
}

impl NetworkDetectionInput {
    pub async fn new(
        camera_locs: Arc<CameraLocationStore>,
    ) -> Result<Self, NetworkDetectionInputCreationError> {
        let udp_transport = UdpTransport::new()
            .await
            .map_err(|e| NetworkDetectionInputCreationError::Udp(e))?;
        let tcp_transport = TcpTransport::new(camera_locs)
            .await
            .map_err(|e| NetworkDetectionInputCreationError::Tcp(e))?;
        Ok(Self {
            udp_transport,
            tcp_transport,
            matcher: FramePairMatcher::new(),
        })
    }
}

impl DetectionInput for NetworkDetectionInput {
    async fn start(mut self, pair_tx: mpsc::Sender<PairedFrames>) -> std::io::Result<()> {
        let (packet_tx, mut packet_rx) = mpsc::channel(PAKCET_CHANNEL_BUF);
        tokio::spawn(async move { self.udp_transport.start(packet_tx).await });
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
        tokio::spawn(async move { self.tcp_transport.listen().await });
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum NetworkDetectionInputCreationError {
    #[error("failed to create TcpTransport: {:?}",.0)]
    Tcp(#[source] std::io::Error),
    #[error("failed to create UdpTransport: {:?}",.0)]
    Udp(#[source] std::io::Error),
}

/// UDP Socket to receive frames from cameras.
pub struct UdpTransport {
    socket: UdpSocket,
    buf: BytesMut,
}

impl UdpTransport {
    pub async fn new() -> std::io::Result<Self> {
        let socket = UdpSocket::bind(FRAME_UDP_ADDR_DEFAULT).await?;
        Ok(Self {
            socket,
            buf: BytesMut::new(),
        })
    }

    pub async fn start(&mut self, frame_tx: mpsc::Sender<DetectionsPacket>) -> std::io::Result<()> {
        loop {
            let (_len, _addr) = self.socket.recv_from(&mut self.buf).await?;

            let packet = DetectionsPacket::decode(&mut self.buf)?;
            frame_tx.send(packet).await.unwrap();
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
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection");
                    let (reader, writer) = stream.into_split();

                    self.init_conneciton(writer).await;

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

    async fn init_conneciton(&mut self, mut writer: tcp::OwnedWriteHalf) {
        let next_camera_id = self.camera_locs.next() as u32;
        info!("camera id for new device is {}", next_camera_id);
        let packet = TcpServerPacket {
            data: Some(tcp_server_packet::Data::CameraId(next_camera_id)),
        };

        write_packet(packet, &mut writer).await.unwrap();
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
                    info!(value = ?camera_loc,"camera location updated");
                    camera_locs.insert(CameraId::new(loc_data.camera_id), camera_loc);
                }
                None => {
                    warn!("TcpClientPacket was empty");
                }
            };
        }
    }
}
