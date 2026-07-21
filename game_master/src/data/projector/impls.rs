use super::{
    BindError, Command, ConnectError, MSG_CHANNEL_BUF, PROJECTOR_PORT, ProjectorTransportStatus,
    Response, SCORE_CHANNEL_BUF, ScoreReceivedError, StartGameError, TcpListenerTrait,
    TcpStreamTrait as _,
};
use slint_fw::WorkerThread;
use std::{cell::RefCell, time::Duration};
use struckout_proto::{
    MasterProjectorPacket, ProjectorMasterPacket, ReadPacketError, WritePacketError,
    master_projector_packet, projector_master_packet, read_packet, write_packet,
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::timeout,
};
use tracing::{debug, info};

#[derive(Debug)]
pub struct ProjectorTransport {
    msg_tx: mpsc::Sender<(Command, oneshot::Sender<Response>)>,
    score_rx: RefCell<Option<mpsc::Receiver<Result<u32, ScoreReceivedError>>>>,
    status_rx: watch::Receiver<ProjectorTransportStatus>,
}

impl ProjectorTransport {
    pub fn new<L>(worker: &WorkerThread) -> Self
    where
        L: TcpListenerTrait + 'static,
    {
        let (cmd_tx, cmd_rx) = mpsc::channel(MSG_CHANNEL_BUF);
        let (score_tx, score_rx) = mpsc::channel(SCORE_CHANNEL_BUF);
        let (status_tx, status_rx) = watch::channel(ProjectorTransportStatus::default());
        worker.spawn(Self::worker::<L>(status_tx, cmd_rx, score_tx));

        Self {
            msg_tx: cmd_tx,
            score_rx: RefCell::new(Some(score_rx)),
            status_rx,
        }
    }

    /// Loops over `cmd_tx` until connected. Once connected, loops over `cmd_tx` and data from the client.
    ///
    /// Must be called inside tokio runtime.
    async fn worker<L: TcpListenerTrait>(
        status_tx: watch::Sender<ProjectorTransportStatus>,
        mut cmd_rx: mpsc::Receiver<(Command, oneshot::Sender<Response>)>,
        score_tx: mpsc::Sender<Result<u32, ScoreReceivedError>>,
    ) {
        let mut inner = ProjectorTransportInner::<L>::new(status_tx);

        loop {
            let (cmd, res_tx): (Command, oneshot::Sender<Response>) = cmd_rx.recv().await.unwrap();
            match cmd {
                Command::Connect => {
                    let res = inner.connect().await;

                    if res.is_err() {
                        res_tx.send(Response::Connect(res)).unwrap();
                        return;
                    }
                    res_tx.send(Response::Connect(res)).unwrap();
                    loop {
                        tokio::select! {
                            cmd = cmd_rx.recv() => {
                                let (cmd, res_tx) = cmd.unwrap();
                                match cmd {
                                    Command::Bind | Command::Connect => {
                                        todo!("these command can't be handled while connected (return Err instead of panic)");
                                    }
                                    Command::StartGame(difficulty) => {
                                        let res =inner.start_game(difficulty).await;
                                        res_tx.send(Response::StartGame(res)).unwrap();
                                    }
                                }
                            }
                            res = inner.recv_score() => {
                                score_tx.send(res).await.unwrap();
                            }
                        }
                    }
                }
                Command::Bind => {
                    let res = inner.bind().await;
                    res_tx.send(Response::Bind(res)).unwrap();
                }
                Command::StartGame(_) => {
                    todo!(
                        "these command can't be handled when not connected (return Err instead of panic)"
                    );
                }
            }
        }
    }

    pub async fn bind(&self) -> Result<(), BindError> {
        let (res_tx, res_rx) = oneshot::channel();
        self.msg_tx.blocking_send((Command::Bind, res_tx)).unwrap();

        let Response::Bind(res) = res_rx.await.unwrap() else {
            panic!(
                "Command::{} should return Response::{}",
                stringify!(Bind),
                stringify!(Bind),
            );
        };
        res
    }

    pub async fn connect(&self) -> Result<(), ConnectError> {
        let (res_tx, res_rx) = oneshot::channel();
        self.msg_tx
            .blocking_send((Command::Connect, res_tx))
            .unwrap();

        let Response::Connect(res) = res_rx.await.unwrap() else {
            panic!(
                "Command::{} should return Response::{}",
                stringify!(Connect),
                stringify!(Connect),
            );
        };
        res
    }

    pub async fn start_game(
        &self,
        difficulty: impl Into<struckout_proto::Difficulty>,
    ) -> Result<(), StartGameError> {
        let (res_tx, res_rx) = oneshot::channel();
        self.msg_tx
            .blocking_send((Command::StartGame(difficulty.into()), res_tx))
            .unwrap();
        let Response::StartGame(res) = res_rx.await.unwrap() else {
            panic!("Command::StartGame should return Response::StartGame");
        };
        res
    }

    pub fn take_rx(&self) -> Option<mpsc::Receiver<Result<u32, ScoreReceivedError>>> {
        self.score_rx.borrow_mut().take()
    }

    pub fn status(&self) -> watch::Receiver<ProjectorTransportStatus> {
        self.status_rx.clone()
    }
}

struct ProjectorTransportInner<L: TcpListenerTrait> {
    status_tx: watch::Sender<ProjectorTransportStatus>,
    state: InternalStatus<L>,
}

impl<L> ProjectorTransportInner<L>
where
    L: TcpListenerTrait,
{
    fn new(status_tx: watch::Sender<ProjectorTransportStatus>) -> Self {
        Self {
            status_tx,
            state: InternalStatus::default(),
        }
    }

    /// Binds port for TCP. Returns immediately without status change if it's already bound.
    async fn bind(&mut self) -> Result<(), BindError> {
        let InternalStatus::NotBound = self.state else {
            return Err(BindError::AlreadyBound);
        };
        debug!("binding port");
        let listener = L::bind(PROJECTOR_PORT).await?;
        self.state = InternalStatus::Bound(listener);
        self.status_tx
            .send(ProjectorTransportStatus::Bound)
            .unwrap();
        Ok(())
    }

    async fn connect(&mut self) -> Result<(), ConnectError> {
        let listener = match &self.state {
            InternalStatus::NotBound => {
                return Err(ConnectError::PortNotBound);
            }
            InternalStatus::Bound(listener) => listener,
            InternalStatus::Connected {
                listener: _,
                stream: _,
            } => {
                return Err(ConnectError::AlreadyConnected);
            }
            InternalStatus::Temp => {
                panic!("dummy should be used temporalily");
            }
        };

        self.status_tx
            .send(ProjectorTransportStatus::WaitingForConnection)
            .unwrap();
        let (stream, addr) = match timeout(Duration::from_mins(10), listener.accept()).await {
            Ok(Ok(it)) => it,
            Ok(Err(e)) => {
                self.status_tx
                    .send(ProjectorTransportStatus::Bound)
                    .unwrap();
                return Err(e.into());
            }
            Err(e) => {
                self.status_tx
                    .send(ProjectorTransportStatus::Bound)
                    .unwrap();
                return Err(e.into());
            }
        };

        // Temporary replace with dummy state in order to move `listener`.
        let InternalStatus::Bound(listener) =
            std::mem::replace(&mut self.state, InternalStatus::Temp)
        else {
            panic!("we've checked `self.state` is `InternalState::Bound` at the first match");
        };
        self.state = InternalStatus::Connected { listener, stream };
        self.status_tx
            .send(ProjectorTransportStatus::Connected)
            .unwrap();

        info!(?addr, "accepted TCP connection with projector");
        Ok(())
    }

    /// Set `state` to [`InternalStatus::Bound`] if the error in `res` indicates broken connection, and returns mapped error.
    async fn recv_score(&mut self) -> Result<u32, ScoreReceivedError> {
        let InternalStatus::Connected {
            listener: _,
            stream,
        } = &mut self.state
        else {
            todo!()
        };
        let res = read_packet::<ProjectorMasterPacket, _>(&mut stream.split().0).await;
        match res {
            Ok(ProjectorMasterPacket { payload: None }) => Err(ScoreReceivedError::EmptyPacket),
            Ok(ProjectorMasterPacket {
                payload: Some(projector_master_packet::Payload::Score(s)),
            }) => Ok(s),
            Err(ReadPacketError::ReadFailed(e)) => {
                let InternalStatus::Connected {
                    listener,
                    stream: _,
                } = std::mem::replace(&mut self.state, InternalStatus::Temp)
                else {
                    panic!("`state` is `InternalStatus::Connected` while loop_recv() is running")
                };
                self.state = InternalStatus::Bound(listener);
                self.status_tx
                    .send(ProjectorTransportStatus::Bound)
                    .unwrap();
                Err(ScoreReceivedError::ReadFailed(e))
            }
            Err(ReadPacketError::DecodeFailed(e)) => Err(ScoreReceivedError::DecodeFailed(e)),
        }
    }

    async fn start_game(
        &mut self,
        difficulty: impl Into<struckout_proto::Difficulty>,
    ) -> Result<(), StartGameError> {
        let InternalStatus::Connected {
            listener: _,
            stream,
        } = &mut self.state
        else {
            return Err(StartGameError::NotConnected);
        };
        let difficulty = {
            let it: struckout_proto::Difficulty = difficulty.into();
            it.into()
        };
        let packet = MasterProjectorPacket {
            payload: Some(master_projector_packet::Payload::StartGame(
                struckout_proto::StartGame { difficulty },
            )),
        };
        if let Err(e) = write_packet(packet, &mut stream.split().1).await {
            match e {
                WritePacketError::EncodeFailed(e) => {
                    panic!("encode failed {:?}", e)
                }
                // TODO: set state to `Bound` if the error suggests unrecoverable connection (e.g. UnexpectedEof)
                WritePacketError::WriteFailed(e) => {
                    return Err(StartGameError::Tcp(e));
                }
            }
        };
        Ok(())
    }
}

#[derive(derive_more::Debug, Default)]
/// Internal state used in [`ProjectorTransportInner`].
enum InternalStatus<L: TcpListenerTrait> {
    #[default]
    NotBound,
    Bound(#[debug(skip)] L),
    Connected {
        #[debug(skip)]
        listener: L,
        #[debug(skip)]
        stream: L::Stream,
    },
    Temp,
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::net::{SocketAddr, ToSocketAddrs};

    use tokio::net::tcp;
    use tokio_stream::{StreamExt, wrappers::WatchStream};

    use crate::data::projector::TcpStreamTrait;

    use super::*;
    struct FakeTcpListener {}

    impl TcpListenerTrait for FakeTcpListener {
        type Stream = FakeTcpStream;

        async fn bind<A: tokio::net::ToSocketAddrs + Send>(
            addr: A,
        ) -> Result<Self, std::io::Error> {
            Ok(Self {})
        }

        async fn accept(&self) -> Result<(Self::Stream, SocketAddr), std::io::Error> {
            tokio::time::sleep(Duration::from_mins(1)).await;
            Ok((
                FakeTcpStream {},
                "127.0.0.1:80".to_socket_addrs().unwrap().next().unwrap(),
            ))
        }
    }

    struct FakeTcpStream {}

    impl TcpStreamTrait for FakeTcpStream {
        type Reader<'a> = tokio::io::Empty;
        type Writer<'a> = tokio::io::Empty;

        fn split(&mut self) -> (Self::Reader<'_>, Self::Writer<'_>) {
            (tokio::io::empty(), tokio::io::empty())
        }
    }

    #[tokio::test]
    async fn inner_bind_returns_err_when_already_bound() {
        let (status_tx, status_rx) = watch::channel(ProjectorTransportStatus::default());
        let mut inner = ProjectorTransportInner::<FakeTcpListener>::new(status_tx);

        assert!(inner.bind().await.is_ok());
        assert_matches!(inner.bind().await, Err(BindError::AlreadyBound));
    }

    #[tokio::test]
    async fn inner_bind_changes_state_to_bound() {
        let (status_tx, mut status_rx) = watch::channel(ProjectorTransportStatus::default());
        let mut inner = ProjectorTransportInner::<FakeTcpListener>::new(status_tx);

        assert_matches!(*status_rx.borrow(), ProjectorTransportStatus::NotBound);
        inner.bind().await.unwrap();
        assert_matches!(
            *status_rx.borrow_and_update(),
            ProjectorTransportStatus::Bound
        );
    }

    #[tokio::test(start_paused = true)]
    async fn inner_connect_transites_states() {
        let (status_tx, status_rx) = watch::channel(ProjectorTransportStatus::default());
        let mut inner = ProjectorTransportInner::<FakeTcpListener>::new(status_tx);

        inner.bind().await.unwrap();

        let mut status = WatchStream::new(status_rx);
        assert_matches!(status.next().await, Some(ProjectorTransportStatus::Bound));

        tokio::spawn(async move {
            inner.connect().await.unwrap();
        });
        assert_matches!(
            status.next().await,
            Some(ProjectorTransportStatus::WaitingForConnection)
        );
        assert_matches!(
            status.next().await,
            Some(ProjectorTransportStatus::Connected)
        );
    }
}
