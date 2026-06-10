use std::{
    collections::{HashMap, HashSet},
    io,
    marker::Unpin,
    net::SocketAddr,
    sync::Arc,
};

use bytes::BytesMut;
use prost::{DecodeError, EncodeError, Message};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Mutex, RwLock, mpsc},
    task::JoinHandle,
};
use tracing::{info, warn};

use crate::{
    State,
    protobuf::{self, TcpClientPacket, TcpServerPacket, UdpPacket, tcp_client_packet},
    types::CameraId,
};

const FRAME_UDP_ADDR_DEFAULT: &str = "0.0.0.0:5050";
const CAMERA_LOC_TCP_ADDR_DEFAULT: &str = "0.0.0.0:6060";

/// UDP Socket to receive frames from cameras.
pub struct UdpTransport {
    socket: UdpSocket,
    buf: BytesMut,
    tx: mpsc::Sender<UdpPacket>,
    clients: HashSet<SocketAddr>,
}

impl UdpTransport {
    pub async fn new(tx: mpsc::Sender<UdpPacket>) -> std::io::Result<Self> {
        // TODO: retry with other port if port is already used
        let socket = UdpSocket::bind(FRAME_UDP_ADDR_DEFAULT).await?;
        Ok(Self {
            socket,
            buf: BytesMut::new(),
            tx,
            clients: HashSet::new(),
        })
    }

    pub async fn start(&mut self) -> std::io::Result<()> {
        loop {
            let (_len, addr) = self.socket.recv_from(&mut self.buf).await?;
            let is_inserted = self.clients.insert(addr);
            if is_inserted {
                info!(address = ?addr, "received frame from new device");
            }
            let packet = UdpPacket::decode(&mut self.buf)?;
            self.tx.send(packet).await.unwrap();
        }
    }
}

/// TCP socket to receive camera location from cameras.
pub struct TcpTransport {
    listener: TcpListener,
    join_handles: Vec<JoinHandle<()>>,
    streams: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,
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
            streams: Arc::new(Mutex::new(HashMap::new())),
            state,
        })
    }

    pub async fn listen(&mut self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    info!(?addr, "accepted new connection");

                    {
                        let mut streams = self.streams.lock().await;
                        streams.insert(addr, stream);
                    }

                    self.init_conneciton(addr).await;

                    let join = tokio::spawn(Self::handle_stream(
                        Arc::clone(&self.state),
                        Arc::clone(&self.streams),
                        addr,
                    ));
                    self.join_handles.push(join);
                }
                Err(_e) => {
                    // TODO: handle errors
                    todo!()
                }
            }
        }
    }

    async fn init_conneciton(&mut self, addr: SocketAddr) {
        let next_camera_id = self.state.read().await.camera_locs.len();
        info!("camera id for new device is {}", next_camera_id);
        let packet = TcpServerPacket {
            data: Some(protobuf::tcp_server_packet::Data::CameraId(
                next_camera_id as u32,
            )),
        };

        write_packet(
            packet,
            &mut self.streams.lock().await.get_mut(&addr).unwrap(),
        )
        .await
        .unwrap();
    }

    async fn handle_stream(
        state: Arc<RwLock<State>>,
        streams: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,
        addr: SocketAddr,
    ) {
        loop {
            let res = read_packet::<TcpClientPacket, _>(
                &mut streams.lock().await.get_mut(&addr).unwrap(),
            )
            .await;

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
                        .await
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

/// Writes a protobuf message to `output`.
///
/// Note that this function allocates buffer every time so it might not be efficient when the function is called frequently.
async fn write_packet<T: Message, O: AsyncWrite + Unpin>(
    packet: T,
    output: &mut O,
) -> Result<(), WritePacketError> {
    let mut buf = BytesMut::new();
    packet.encode(&mut buf)?;
    let len: u32 = buf
        .len()
        .try_into()
        .expect("packet size is too large so that it cannot be fit in header");

    output.write_all(&len.to_le_bytes()).await?;
    output.write_all(&buf).await?;
    Ok(())
}

#[derive(Debug, Error)]
enum WritePacketError {
    #[error(transparent)]
    EncodeFailed(#[from] EncodeError),
    #[error(transparent)]
    WriteFailed(#[from] std::io::Error),
}

/// Reads a protobuf message from `input`.
///
/// Note that this function allocates buffer every time so it might not be efficient when the function is called frequently.
async fn read_packet<T: Message + Default, I: AsyncRead + Unpin>(
    input: &mut I,
) -> Result<T, ReadPacketError> {
    let len = input.read_u32_le().await?;
    let mut buf = BytesMut::zeroed(len as usize);
    input.read_exact(&mut buf).await?;
    let packet = T::decode(&mut buf)?;
    Ok(packet)
}

#[derive(Debug, Error)]
enum ReadPacketError {
    #[error(transparent)]
    ReadFailed(#[from] std::io::Error),
    #[error(transparent)]
    DecodeFailed(#[from] DecodeError),
}

#[cfg(test)]
mod tests {
    #[test]
    fn hoge() {
        todo!()
    }

    #[test]
    fn data_len_is_serialized_correctly() {
        let len: u32 = 2000;
        let bytes = len.to_le_bytes();
        assert_eq!(bytes, [208, 7, 0, 0]);
    }

    #[test]
    fn data_len_is_deserialized_correctly() {
        let bytes: [u8; 4] = [0xd0, 0x07, 0, 0];
        let len = u32::from_le_bytes(bytes);
        assert_eq!(len, 2000);
    }
}
