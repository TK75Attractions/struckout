use std::pin::Pin;

use tokio::sync::mpsc;

const WORKER_CHANNEL_BUF: usize = 8;

/// A thread to run async codes.
pub struct WorkerThread {
    tx: mpsc::Sender<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
}

// TODO: 停止できるようにする
impl WorkerThread {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(WORKER_CHANNEL_BUF);
        let _join = std::thread::spawn(move || Self::run(rx));
        Self { tx }
    }

    #[tokio::main]
    async fn run(mut rx: mpsc::Receiver<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>) {
        loop {
            let fut = rx.recv().await.unwrap();
            let _join = tokio::spawn(fut);
        }
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tx.blocking_send(Box::pin(fut)).unwrap();
    }
}
