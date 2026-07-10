use tokio::sync::mpsc;

const WORKER_CHANNEL_BUF: usize = 8;

/// A thread to run async codes.
pub struct WorkerThread {
    tx: mpsc::Sender<Box<dyn Future<Output = ()> + Send + Unpin + 'static>>,
}

// TODO: 停止できるようにする
impl WorkerThread {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(WORKER_CHANNEL_BUF);
        let _join = std::thread::spawn(move || Self::run(rx));
        Self { tx }
    }

    #[tokio::main]
    async fn run(mut rx: mpsc::Receiver<Box<dyn Future<Output = ()> + Send + Unpin + 'static>>) {
        loop {
            let fut = rx.recv().await.unwrap();
            let _join = tokio::spawn(fut);
        }
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + Unpin + 'static,
    {
        self.tx.blocking_send(Box::new(fut)).unwrap();
    }
}
