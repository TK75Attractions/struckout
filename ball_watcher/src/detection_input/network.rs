use std::{io, sync::Arc};

use bytes::BytesMut;
use parking_lot::RwLock;
use prost::Message as _;
use struckout_proto::{
    ReadPacketError, TcpClientPacket, TcpServerPacket, UdpPacket, read_packet, tcp_client_packet,
    tcp_server_packet, write_packet,
};
use thiserror::Error;
use tokio::{
    net::{TcpListener, UdpSocket, tcp},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::{info, warn};

use crate::{State, detection_input::DetectionInput, types::CameraId};

const FRAME_UDP_ADDR_DEFAULT: &str = "0.0.0.0:5050";
const CAMERA_LOC_TCP_ADDR_DEFAULT: &str = "0.0.0.0:6060";

pub struct NetworkDetectionInput {
    udp_transport: UdpTransport,
    tcp_transport: TcpTransport,
}

impl NetworkDetectionInput {
    pub async fn new(
        state: Arc<RwLock<State>>,
    ) -> Result<Self, NetworkDetectionInputCreationError> {
        let udp_transport = UdpTransport::new()
            .await
            .map_err(|e| NetworkDetectionInputCreationError::Udp(e))?;
        let tcp_transport = TcpTransport::new(state)
            .await
            .map_err(|e| NetworkDetectionInputCreationError::Tcp(e))?;
        Ok(Self {
            udp_transport,
            tcp_transport,
        })
    }
}

impl DetectionInput for NetworkDetectionInput {
    async fn start(mut self, frame_tx: mpsc::Sender<UdpPacket>) -> std::io::Result<()> {
        tokio::spawn(async move { self.udp_transport.start(frame_tx).await });
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
        // TODO: retry with other port if port is already used
        let socket = UdpSocket::bind(FRAME_UDP_ADDR_DEFAULT).await?;
        Ok(Self {
            socket,
            buf: BytesMut::new(),
        })
    }

    pub async fn start(&mut self, frame_tx: mpsc::Sender<UdpPacket>) -> std::io::Result<()> {
        loop {
            let (_len, _addr) = self.socket.recv_from(&mut self.buf).await?;

            let packet = UdpPacket::decode(&mut self.buf)?;
            frame_tx.send(packet).await.unwrap();
        }
    }
}

/// TCP socket to receive camera location from cameras.
pub struct TcpTransport {
    listener: TcpListener,
    join_handles: Vec<JoinHandle<()>>,
    state: Arc<RwLock<State>>,
}

impl TcpTransport {
    pub async fn new(state: Arc<RwLock<State>>) -> std::io::Result<Self> {
        // TODO: retry with other port if port is already used
        info!(
            port = CAMERA_LOC_TCP_ADDR_DEFAULT,
            "trying to bind port for `CameraLocationListener`"
        );
        let listener = TcpListener::bind(CAMERA_LOC_TCP_ADDR_DEFAULT).await?;
        info!("succeed to bind port for `CameraLocationListener`");
        Ok(Self {
            listener,
            join_handles: Vec::new(),
            state,
        })
    }

    pub async fn listen(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection");
                    let (reader, writer) = stream.into_split();

                    self.init_conneciton(writer).await;

                    let join = tokio::spawn(Self::handle_stream(Arc::clone(&self.state), reader));
                    self.join_handles.push(join);
                }
                Err(_e) => {
                    // TODO: handle errors
                    todo!()
                }
            }
        }
    }

    async fn init_conneciton(&mut self, mut writer: tcp::OwnedWriteHalf) {
        let next_camera_id = self.state.read().camera_locs.len();
        info!("camera id for new device is {}", next_camera_id);
        let packet = TcpServerPacket {
            data: Some(tcp_server_packet::Data::CameraId(next_camera_id as u32)),
        };

        write_packet(packet, &mut writer).await.unwrap();
    }

    async fn handle_stream(state: Arc<RwLock<State>>, mut reader: tcp::OwnedReadHalf) {
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
                    state
                        .write()
                        .camera_locs
                        .insert(CameraId::new(loc_data.camera_id), camera_loc);
                }
                None => {
                    warn!("TcpClientPacket was empty");
                }
            };
        }
    }
}
