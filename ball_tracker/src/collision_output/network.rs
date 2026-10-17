//! projector に着弾位置を送る (TCP の 5000 番)。
//!
//! projector はいつつないでも、つなぎ直してもよい。つないでいる projector すべてに送る
//! (ポートの確認などでつないだだけの接続があっても、本物の projector に届かなくなることはない)。
//! つないでいる projector が無いあいだの着弾は捨てる。

use std::net::SocketAddr;

use struckout_proto::{CollisionPoint, ProjectorPacket, projector_packet, write_packet};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc},
};
use tracing::{error, info, warn};

use crate::{collision_output::CollisionOutput, types::CollisionPoint3D};

const PROJECTOR_ADDR: &str = "0.0.0.0:5000";
/// 書き込みの遅い projector のために、ためておける着弾の数。超えたぶんはその projector には送らない
const PROJECTOR_BUF: usize = 16;

pub struct NetworkCollisionOutput {
    listener: TcpListener,
}

impl NetworkCollisionOutput {
    /// 5000 番で待ち受ける。projector がつなぐのは待たない。
    pub async fn bind() -> Result<Self, std::io::Error> {
        Self::bind_to(PROJECTOR_ADDR).await
    }

    pub async fn bind_to(addr: &str) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(addr).await?;
        info!(addr, "listening for projector");
        Ok(Self { listener })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

impl CollisionOutput for NetworkCollisionOutput {
    async fn start(self, mut collision_rx: mpsc::Receiver<CollisionPoint3D>) {
        let (packet_tx, _) = broadcast::channel::<ProjectorPacket>(PROJECTOR_BUF);
        let accept_tx = packet_tx.clone();
        let listener = self.listener;
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        info!(?addr, "accepted TCP connection from projector");
                        tokio::spawn(serve_projector(stream, addr, accept_tx.subscribe()));
                    }
                    Err(e) => warn!(error = ?e, "failed to accept a connection from projector"),
                }
            }
        });

        loop {
            let coll = match collision_rx.recv().await {
                Some(c) => c,
                None => {
                    error!("collision channel has been unexpectedly closed");
                    std::process::exit(1);
                }
            };
            let packet = ProjectorPacket {
                // projector には的の面の上での位置を送る。的の面は世界座標の x = 0 なので、
                // 面に沿った横方向が y、高さが z になる。
                payload: Some(projector_packet::Payload::Point(CollisionPoint {
                    x: coll.y,
                    y: coll.z,
                })),
            };
            if packet_tx.send(packet).is_err() {
                warn!(y = coll.y, z = coll.z, "no projector is connected; dropped a collision");
            }
        }
    }
}

/// 1 台の projector に送り続ける。切れたら終わる。
async fn serve_projector(stream: TcpStream, addr: SocketAddr, mut packet_rx: broadcast::Receiver<ProjectorPacket>) {
    if let Err(e) = stream.set_nodelay(true) {
        warn!(error = ?e, "failed to set TCP_NODELAY");
    }
    let (mut reader, mut writer) = stream.into_split();
    // projector からは何も届かないが、読んでおくと切れたことがすぐ分かる
    let mut discard = [0u8; 256];
    loop {
        tokio::select! {
            packet = packet_rx.recv() => match packet {
                Ok(packet) => {
                    if let Err(e) = write_packet(packet, &mut writer).await {
                        info!(?addr, error = ?e, "projector disconnected");
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(?addr, skipped, "projector is too slow; skipped collisions");
                }
                Err(broadcast::error::RecvError::Closed) => return,
            },
            read = reader.read(&mut discard) => match read {
                Ok(0) | Err(_) => {
                    info!(?addr, "projector disconnected");
                    return;
                }
                Ok(_) => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use struckout_proto::read_packet;

    use super::*;

    async fn next_point(stream: &mut TcpStream) -> (f64, f64) {
        let packet: ProjectorPacket = tokio::time::timeout(Duration::from_secs(2), read_packet(stream))
            .await
            .expect("no collision arrived")
            .unwrap();
        match packet.payload {
            Some(projector_packet::Payload::Point(point)) => (point.x, point.y),
            other => panic!("unexpected packet {other:?}"),
        }
    }

    fn collision(y: f64, z: f64) -> CollisionPoint3D {
        CollisionPoint3D { x: 0.0, y, z }
    }

    #[tokio::test]
    async fn sends_to_every_projector_and_survives_one_leaving() {
        let output = NetworkCollisionOutput::bind_to("127.0.0.1:0").await.unwrap();
        let addr = output.local_addr().unwrap();
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(output.start(rx));

        // projector がつないでいないあいだの着弾は捨てる (落ちない)
        tx.send(collision(9.0, 9.0)).await.unwrap();

        let mut first = TcpStream::connect(addr).await.unwrap();
        let mut second = TcpStream::connect(addr).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        tx.send(collision(0.5, 1.5)).await.unwrap();
        assert_eq!(next_point(&mut first).await, (0.5, 1.5));
        assert_eq!(next_point(&mut second).await, (0.5, 1.5));

        drop(first);
        tokio::time::sleep(Duration::from_millis(100)).await;
        tx.send(collision(0.6, 1.6)).await.unwrap();
        assert_eq!(next_point(&mut second).await, (0.6, 1.6));

        // つなぎ直した projector にも届く
        let mut again = TcpStream::connect(addr).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        tx.send(collision(0.7, 1.7)).await.unwrap();
        assert_eq!(next_point(&mut again).await, (0.7, 1.7));
        assert_eq!(next_point(&mut second).await, (0.7, 1.7));
    }
}
