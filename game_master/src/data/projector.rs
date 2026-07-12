use struckout_proto::{MasterPacket, WritePacketError, master_packet, write_packet};
use thiserror::Error;
use tokio::{
    net::{TcpListener, tcp},
    sync::{mpsc, oneshot},
};
use tracing::info;

use crate::{ui, worker::WorkerThread};

// TODO: set actual value
const PROJECTOR_PORT: &str = "192.10.100.10:5252";
const COMMAND_CHANNEL_BUF: usize = 8;
const MSG_CHANNEL_BUF: usize = 8;

pub trait ProjectorConnection {
    fn listen(&self) -> std::io::Result<()>;
    fn start_game(
        &mut self,
        difficulty: impl Into<struckout_proto::Difficulty>,
        cb: impl FnOnce(Result<(), StartGameError>) + 'static,
    );
}

pub struct ProjectorConnectionImpl {
    msg_tx: mpsc::Sender<(Command, oneshot::Sender<Response>)>,
}

impl ProjectorConnectionImpl {
    /// Binds TCP port.
    pub async fn new(worker: &WorkerThread) -> Result<Self, std::io::Error> {
        let (msg_tx, mut msg_rx) = mpsc::channel(MSG_CHANNEL_BUF);
        let mut inner = ProjectorTransportInner::bind().await?;
        worker.spawn(async move {
            loop {
                let (msg, res_tx): (Command, oneshot::Sender<Response>) =
                    msg_rx.recv().await.unwrap();
                match msg {
                    Command::StartGame(difficulty) => {
                        let res = inner.start_game(difficulty).await;
                        res_tx.send(Response::StartGame(res)).unwrap();
                    }
                }
            }
        });

        Ok(Self { msg_tx })
    }
}

impl ProjectorConnection for ProjectorConnectionImpl {
    fn listen(&self) -> std::io::Result<()> {
        todo!();
    }

    fn start_game(
        &mut self,
        difficulty: impl Into<struckout_proto::Difficulty>,
        cb: impl FnOnce(Result<(), StartGameError>) + 'static,
    ) {
        let difficulty = difficulty.into();
        let (res_tx, res_rx) = oneshot::channel();
        self.msg_tx
            .blocking_send((Command::StartGame(difficulty), res_tx))
            .unwrap();
        slint::spawn_local(async move {
            #[allow(irrefutable_let_patterns)] // 後で他のも追加するつもりなので
            let Response::StartGame(res) = res_rx.await.unwrap() else {
                panic!("Command::StartGame should return Response::StartGame");
            };
            cb(res);
        })
        .unwrap();
    }
}

#[derive(Debug)]
enum Command {
    StartGame(struckout_proto::Difficulty),
}

#[derive(Debug)]
enum Response {
    StartGame(Result<(), StartGameError>),
}

struct ProjectorTransportInner {
    listener: TcpListener,
    conn_state: ConnectionState,
}

impl ProjectorTransportInner {
    async fn bind() -> std::io::Result<Self> {
        let listener = TcpListener::bind(PROJECTOR_PORT).await?;

        Ok(Self {
            listener,
            conn_state: ConnectionState::DisConnected,
        })
    }

    async fn listen(&mut self) -> std::io::Result<()> {
        let (stream, addr) = self.listener.accept().await?;
        info!(?addr, "accepted TCP connection with projector");
        let (reader, writer) = stream.into_split();
        self.conn_state = ConnectionState::Connected { reader, writer };
        Ok(())
    }

    async fn start_game(
        &mut self,
        difficulty: struckout_proto::Difficulty,
    ) -> Result<(), StartGameError> {
        let ConnectionState::Connected { reader: _, writer } = &mut self.conn_state else {
            return Err(StartGameError::NotConnected);
        };

        let packet = MasterPacket {
            payload: Some(master_packet::Payload::StartGame(
                struckout_proto::StartGame {
                    difficulty: difficulty.into(),
                },
            )),
        };
        if let Err(e) = write_packet(packet, writer).await {
            match e {
                WritePacketError::EncodeFailed(e) => {
                    panic!("encode failed, {:?}", e)
                }
                WritePacketError::WriteFailed(e) => return Err(StartGameError::Tcp(e)),
            }
        };
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum StartGameError {
    #[error("TCP is not connected")]
    NotConnected,
    #[error(transparent)]
    Tcp(#[from] std::io::Error),
}

enum ConnectionState {
    Connected {
        reader: tcp::OwnedReadHalf,
        writer: tcp::OwnedWriteHalf,
    },
    DisConnected,
}

impl From<ui::Difficulity> for struckout_proto::Difficulty {
    fn from(value: ui::Difficulity) -> Self {
        match value {
            ui::Difficulity::Normal => struckout_proto::Difficulty::Normal,
            ui::Difficulity::Hard => struckout_proto::Difficulty::Hard,
            ui::Difficulity::VeryHard => struckout_proto::Difficulty::Veryhard,
        }
    }
}
