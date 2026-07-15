use prost::DecodeError;
use std::{cell::OnceCell, time::Duration};
use struckout_proto::{
    MasterProjectorPacket, ProjectorMasterPacket, ReadPacketError, WritePacketError,
    master_projector_packet, projector_master_packet, read_packet, write_packet,
};
use thiserror::Error;
use tokio::{
    net::{TcpListener, tcp},
    sync::{mpsc, oneshot},
    time::{error::Elapsed, timeout},
};
use tracing::{debug, info};

use crate::{ui, worker::WorkerThread};

// TODO: set actual value
const PROJECTOR_PORT: &str = "0.0.0.0:5252";
const MSG_CHANNEL_BUF: usize = 8;
const SCORE_CHANNEL_BUF: usize = 8;

pub trait ProjectorConnection {
    fn bind<F>(&self, cb: F)
    where
        F: FnOnce(Result<(), BindError>) + 'static;

    fn connect<F>(&self, cb: F)
    where
        F: FnOnce(Result<(), ConnectError>) + 'static;

    fn start_game<F>(&mut self, difficulty: impl Into<struckout_proto::Difficulty>, cb: F)
    where
        F: FnOnce(Result<(), StartGameError>) + 'static;

    fn take_rx(&mut self) -> Option<mpsc::Receiver<Result<u32, ScoreReceivedError>>>;
}

pub struct ProjectorConnectionImpl {
    msg_tx: mpsc::Sender<(Command, oneshot::Sender<Response>)>,
    score_rx: Option<mpsc::Receiver<Result<u32, ScoreReceivedError>>>,
}

impl ProjectorConnectionImpl {
    pub fn new(worker: &WorkerThread) -> Self {
        let (msg_tx, mut msg_rx) = mpsc::channel(MSG_CHANNEL_BUF);
        let (score_tx, score_rx) = mpsc::channel(SCORE_CHANNEL_BUF);
        let mut inner = ProjectorTransportInner::new(score_tx);
        worker.spawn(async move {
            loop {
                let (msg, res_tx): (Command, oneshot::Sender<Response>) =
                    msg_rx.recv().await.unwrap();
                match msg {
                    Command::StartGame(difficulty) => {
                        let res = inner.start_game(difficulty).await;
                        res_tx.send(Response::StartGame(res)).unwrap();
                    }
                    Command::Connect => {
                        let res = inner.connect().await;
                        res_tx.send(Response::Connect(res)).unwrap();
                    }
                    Command::Bind => {
                        let res = inner.bind().await;
                        res_tx.send(Response::Bind(res)).unwrap();
                    }
                }
            }
        });

        Self {
            msg_tx,
            score_rx: Some(score_rx),
        }
    }
}

impl ProjectorConnection for ProjectorConnectionImpl {
    async_wrapper!(bind());

    async_wrapper!(
        /// Callback is called when client requested connection and the connection was established.
        connect() -> ConnectError
    );

    fn start_game<F>(&mut self, difficulty: impl Into<struckout_proto::Difficulty>, cb: F)
    where
        F: FnOnce(Result<(), StartGameError>) + 'static,
    {
        let difficulty = difficulty.into();
        let (res_tx, res_rx) = oneshot::channel();
        self.msg_tx
            .blocking_send((Command::StartGame(difficulty), res_tx))
            .unwrap();
        slint::spawn_local(async move {
            let Response::StartGame(res) = res_rx.await.unwrap() else {
                panic!("Command::StartGame should return Response::StartGame");
            };
            cb(res);
        })
        .unwrap();
    }

    fn take_rx(&mut self) -> Option<mpsc::Receiver<Result<u32, ScoreReceivedError>>> {
        self.score_rx.take()
    }
}

#[derive(Debug, Error)]
pub enum ScoreReceivedError {
    #[error(transparent)]
    ReadFailed(#[from] std::io::Error),
    #[error(transparent)]
    DecodeFailed(#[from] DecodeError),
    #[error("packet received from projector was empty")]
    EmptyPacket,
}

#[derive(derive_more::Debug)]
enum Command {
    /// See [`ProjectorTransportInner::start_game()`]
    StartGame(struckout_proto::Difficulty),
    /// See [`ProjectorTransportInner::connect()`]
    Connect,
    /// See [`ProjectorTransportInner::bind()`]
    Bind,
}

#[derive(Debug)]
enum Response {
    /// See [`ProjectorTransportInner::start_game()`]
    StartGame(Result<(), StartGameError>),
    /// See [`ProjectorTransportInner::connect()`]
    Connect(Result<(), ConnectError>),
    /// See [`ProjectorTransportInner::bind()`]
    Bind(Result<(), BindError>),
}

struct ProjectorTransportInner {
    listener: OnceCell<TcpListener>,
    conn_state: ConnectionState,
    score_tx: mpsc::Sender<Result<u32, ScoreReceivedError>>,
}

impl ProjectorTransportInner {
    fn new(score_tx: mpsc::Sender<Result<u32, ScoreReceivedError>>) -> Self {
        Self {
            listener: OnceCell::new(),
            conn_state: ConnectionState::DisConnected,
            score_tx,
        }
    }

    async fn bind(&mut self) -> Result<(), BindError> {
        debug!("binding port");
        let listener = TcpListener::bind(PROJECTOR_PORT).await?;
        self.listener
            .set(listener)
            .map_err(|_| BindError::AlreadyBound)?;
        Ok(())
    }

    async fn connect(&mut self) -> Result<(), ConnectError> {
        let listener = self.listener.get().ok_or(ConnectError::PortNotBound)?;
        let (stream, addr) = timeout(Duration::from_secs(30), listener.accept()).await??;
        info!(?addr, "accepted TCP connection with projector");
        let (mut reader, writer) = stream.into_split();
        self.conn_state = ConnectionState::Connected { writer };

        let score_tx = self.score_tx.clone();
        tokio::spawn(async move {
            loop {
                let res: Result<ProjectorMasterPacket, ReadPacketError> =
                    read_packet::<ProjectorMasterPacket, _>(&mut reader).await;
                let res = match res {
                    Ok(ProjectorMasterPacket { payload: None }) => {
                        Err(ScoreReceivedError::EmptyPacket)
                    }
                    Ok(ProjectorMasterPacket {
                        payload: Some(projector_master_packet::Payload::Score(s)),
                    }) => Ok(s),
                    Err(ReadPacketError::ReadFailed(e)) => Err(ScoreReceivedError::ReadFailed(e)),
                    Err(ReadPacketError::DecodeFailed(e)) => {
                        Err(ScoreReceivedError::DecodeFailed(e))
                    }
                };
                score_tx.send(res).await.unwrap();
            }
        });
        Ok(())
    }

    async fn start_game(
        &mut self,
        difficulty: struckout_proto::Difficulty,
    ) -> Result<(), StartGameError> {
        let ConnectionState::Connected { writer } = &mut self.conn_state else {
            return Err(StartGameError::NotConnected);
        };

        let packet = MasterProjectorPacket {
            payload: Some(master_projector_packet::Payload::StartGame(
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

/// Error type used in [ProjectorConnection::start_game()].
#[derive(Debug, Error)]
pub enum StartGameError {
    #[error("TCP is not connected")]
    NotConnected,
    #[error(transparent)]
    Tcp(#[from] std::io::Error),
}

/// Error type used in [ProjectorConnection::listen()].
#[derive(Debug, Error)]
pub enum ConnectError {
    #[error("port is not bound and TCP listner not created")]
    PortNotBound,
    #[error("listener timed out")]
    Timeout(#[from] Elapsed),
    #[error(transparent)]
    Tcp(#[from] std::io::Error),
}

/// Error type used in [ProjectorConnection::bind()].
#[derive(Debug, Error)]
pub enum BindError {
    #[error("TCP port is already bound")]
    AlreadyBound,
    #[error(transparent)]
    Other(#[from] std::io::Error),
}

enum ConnectionState {
    Connected { writer: tcp::OwnedWriteHalf },
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
