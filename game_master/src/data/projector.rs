use std::{cell::OnceCell, time::Duration};
use struckout_proto::{MasterPacket, WritePacketError, master_packet, write_packet};
use thiserror::Error;
use tokio::{
    net::{TcpListener, tcp},
    sync::{mpsc, oneshot},
    time::{error::Elapsed, timeout},
};
use tracing::info;

use crate::{ui, worker::WorkerThread};

// TODO: set actual value
const PROJECTOR_PORT: &str = "192.10.100.10:5252";
const COMMAND_CHANNEL_BUF: usize = 8;
const MSG_CHANNEL_BUF: usize = 8;

pub trait ProjectorConnection {
    fn bind<F>(&self, cb: F)
    where
        F: FnOnce(Result<(), BindError>) + 'static;

    fn listen<F>(&self, cb: F)
    where
        F: FnOnce(Result<(), ListenError>) + 'static;

    fn start_game<F>(&mut self, difficulty: impl Into<struckout_proto::Difficulty>, cb: F)
    where
        F: FnOnce(Result<(), StartGameError>) + 'static;
}

pub struct ProjectorConnectionImpl {
    msg_tx: mpsc::Sender<(Command, oneshot::Sender<Response>)>,
}

impl ProjectorConnectionImpl {
    pub fn new(worker: &WorkerThread) -> Self {
        let (msg_tx, mut msg_rx) = mpsc::channel(MSG_CHANNEL_BUF);
        let mut inner = ProjectorTransportInner::new();
        worker.spawn(async move {
            loop {
                let (msg, res_tx): (Command, oneshot::Sender<Response>) =
                    msg_rx.recv().await.unwrap();
                match msg {
                    Command::StartGame(difficulty) => {
                        let res = inner.start_game(difficulty).await;
                        res_tx.send(Response::StartGame(res)).unwrap();
                    }
                    Command::Listen => {
                        let res = inner.listen().await;
                        res_tx.send(Response::Listen(res)).unwrap();
                    }
                    Command::Bind => {
                        let res = inner.bind().await;
                        res_tx.send(Response::Bind(res)).unwrap();
                    }
                }
            }
        });

        Self { msg_tx }
    }
}

impl ProjectorConnection for ProjectorConnectionImpl {
    async_wrapper!(bind());

    async_wrapper!(
        /// Callback is called when client requested connection and the connection was established.
        listen() -> ListenError
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
}

#[derive(Debug)]
enum Command {
    /// See [`ProjectorTransportInner::start_game()`]
    StartGame(struckout_proto::Difficulty),
    /// See [`ProjectorTransportInner::listen()`]
    Listen,
    /// See [`ProjectorTransportInner::bind()`]
    Bind,
}

#[derive(Debug)]
enum Response {
    /// See [`ProjectorTransportInner::start_game()`]
    StartGame(Result<(), StartGameError>),
    /// See [`ProjectorTransportInner::listen()`]
    Listen(Result<(), ListenError>),
    /// See [`ProjectorTransportInner::bind()`]
    Bind(Result<(), BindError>),
}

struct ProjectorTransportInner {
    listener: OnceCell<TcpListener>,
    conn_state: ConnectionState,
}

impl ProjectorTransportInner {
    fn new() -> Self {
        Self {
            listener: OnceCell::new(),
            conn_state: ConnectionState::DisConnected,
        }
    }

    async fn bind(&mut self) -> Result<(), BindError> {
        let listener = TcpListener::bind(PROJECTOR_PORT).await?;
        self.listener
            .set(listener)
            .map_err(|_| BindError::AlreadyBound)?;
        Ok(())
    }

    async fn listen(&mut self) -> Result<(), ListenError> {
        let listener = self.listener.get().ok_or(ListenError::PortNotBound)?;
        let (stream, addr) = timeout(Duration::from_mins(1), listener.accept()).await??;
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
pub enum ListenError {
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
