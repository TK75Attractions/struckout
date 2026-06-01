use std::{collections::HashSet, net::SocketAddr, sync::Arc};

use bytes::BytesMut;
use prost::Message;
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Mutex, mpsc},
    task::JoinHandle,
    time,
};
use tracing::info;

use crate::protobuf::{TcpPacket, UdpPacket};

const FRAME_UDP_ADDR_DEFAULT: &str = "0.0.0.0:5050";
const CAMERA_LOC_TCP_ADDR_DEFAULT: &str = "0.0.0.0:6060";

/// UDP Socket to receive frames from cameras.
pub struct FrameSocket {
    socket: UdpSocket,
    buf: BytesMut,
    tx: mpsc::Sender<UdpPacket>,
    server_start: time::Instant,
    /// クライアントにserver timeを送る周期
    time_sync_interval: time::Interval,
    clients: HashSet<SocketAddr>,
}

impl FrameSocket {
    pub async fn new(
        tx: mpsc::Sender<UdpPacket>,
        time_sync_interval: time::Duration,
    ) -> std::io::Result<Self> {
        // TODO: retry with other port if port is already used
        let socket = UdpSocket::bind(FRAME_UDP_ADDR_DEFAULT).await?;
        Ok(Self {
            socket,
            buf: BytesMut::new(),
            tx,
            server_start: time::Instant::now(),
            time_sync_interval: time::interval(time_sync_interval),
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

    /// サーバが開始した時刻からのタイムスタンプを全クライアントに送る
    async fn send_server_time(&self) {
        let now = time::Instant::now();
        let server_timestamp = (now - self.server_start).as_nanos();
        for s in &self.clients {
            let time_bytes = server_timestamp.to_le_bytes();
            self.socket.send_to(&time_bytes, s);
        }
    }
}

/// TCP socket to receive camera location from cameras.
pub struct CameraLocationListener {
    listener: TcpListener,
    join_handles: Vec<JoinHandle<()>>,
    tx: mpsc::Sender<TcpPacket>,
    streams: Arc<Mutex<Vec<TcpStream>>>,
    server_start: time::Instant,
}

impl CameraLocationListener {
    pub async fn new(tx: mpsc::Sender<TcpPacket>) -> std::io::Result<Self> {
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
            tx,
            streams: Arc::new(Mutex::new(Vec::new())),
            server_start: time::Instant::now(),
        })
    }

    pub async fn listen(&mut self) {
        match self.listener.accept().await {
            Ok((stream, _addr)) => {
                let stream_pos = {
                    let mut streams = self.streams.lock().await;
                    let stream_pos = streams.len();
                    streams.push(stream);
                    stream_pos
                };
                let streams = Arc::clone(&self.streams);
                let tx = self.tx.clone();
                let join = tokio::spawn(async move {
                    let mut buf = BytesMut::new();
                    loop {
                        {
                            let mut streams = streams.lock().await;
                            let stream = streams.get_mut(stream_pos).unwrap();
                            let _len = stream.read(&mut buf).await.unwrap(); // TODO: handle errors
                        }
                        let packet = TcpPacket::decode(&mut buf).unwrap(); // TODO: handle errors
                        tx.send(packet).await.unwrap(); // TODO: handle error
                    }
                });
                self.join_handles.push(join);
            }
            Err(_e) => {
                // TODO: handle errors
                todo!()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hoge() {
        todo!()
    }
}
