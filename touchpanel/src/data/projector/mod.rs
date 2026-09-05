use std::net::SocketAddr;

use prost::DecodeError;

use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{ToSocketAddrs, tcp},
    time::error::Elapsed,
};

use crate::ui;

mod impls;
pub use impls::ProjectorTransport;

const PROJECTOR_PORT: &str = "0.0.0.0:5001";
const MSG_CHANNEL_BUF: usize = 8;
const SCORE_CHANNEL_BUF: usize = 8;

pub trait TcpListenerTrait: Sized + Send {
    type Stream: TcpStreamTrait;

    fn bind<A: tokio::net::ToSocketAddrs + Send>(
        addr: A,
    ) -> impl std::future::Future<Output = Result<Self, std::io::Error>> + Send;

    fn accept(
        &self,
    ) -> impl std::future::Future<Output = Result<(Self::Stream, SocketAddr), std::io::Error>> + Send;
}

impl TcpListenerTrait for tokio::net::TcpListener {
    type Stream = tokio::net::TcpStream;

    fn bind<A: ToSocketAddrs + Send>(
        addr: A,
    ) -> impl std::future::Future<Output = Result<Self, std::io::Error>> + Send {
        Self::bind(addr)
    }

    fn accept(
        &self,
    ) -> impl std::future::Future<Output = Result<(Self::Stream, SocketAddr), std::io::Error>> + Send
    {
        Self::accept(&self)
    }
}

pub trait TcpStreamTrait: Send {
    type Reader<'a>: AsyncRead + Unpin + Send
    where
        Self: 'a;
    type Writer<'a>: AsyncWrite + Unpin + Send
    where
        Self: 'a;

    fn split<'a>(&'a mut self) -> (Self::Reader<'a>, Self::Writer<'a>);
}

impl TcpStreamTrait for tokio::net::TcpStream {
    type Reader<'a> = tcp::ReadHalf<'a>;
    type Writer<'a> = tcp::WriteHalf<'a>;

    fn split<'a>(&'a mut self) -> (Self::Reader<'a>, Self::Writer<'a>) {
        tokio::net::TcpStream::split(self)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub enum ProjectorTransportStatus {
    /// Port is not bound for TCP.
    #[default]
    NotBound,
    /// Port is bound, but not accepting connection from client.
    Bound,
    /// Port is bound and waiting for connection from client.
    WaitingForConnection,
    /// Connected with client.
    Connected,
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
    #[error("already connected with projector")]
    AlreadyConnected,
    #[error("already waiting for connection")]
    AlreadyWaitingForConnection,
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

impl From<ui::Difficulity> for struckout_proto::Difficulty {
    fn from(value: ui::Difficulity) -> Self {
        match value {
            ui::Difficulity::Normal => struckout_proto::Difficulty::Normal,
            ui::Difficulity::Hard => struckout_proto::Difficulty::Hard,
            ui::Difficulity::VeryHard => struckout_proto::Difficulty::Veryhard,
        }
    }
}
