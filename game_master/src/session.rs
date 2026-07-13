use std::fmt::Display;

use chrono::Duration;
use tokio::sync::mpsc;

use crate::worker::WorkerThread;

const TIMELIMIT: RemainingTime = RemainingTime { mins: 1, secs: 30 };

pub struct SessionManager {
    session: Option<Session>,
    on_score_change: Option<Box<dyn FnMut(u32)>>,
    on_remaining_time_change: Option<Box<dyn FnMut(&RemainingTime)>>,
}

impl SessionManager {
    pub fn new(worker: &WorkerThread, score_rx: mpsc::Receiver<u32>) -> Self {
        let inner = SessionManagerInner::new(score_rx);
        worker.spawn(async move {
            inner.start().await;
        });
        Self {
            session: None,
            on_score_change: None,
            on_remaining_time_change: None,
        }
    }

    pub fn subscribe_on_score_change<F>(&mut self, f: F)
    where
        F: FnMut(u32),
    {
        self.on_score_change = Some(Box::new(f));
    }

    pub fn subscribe_on_remaining_time_change<F>(&mut self, f: F)
    where
        F: FnMut(&RemainingTime),
    {
        self.on_remaining_time_change = Some(Box::new(f));
    }
}

#[derive(Debug, Clone)]
pub struct RemainingTime {
    mins: usize,
    secs: usize,
}

impl Display for RemainingTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.mins, self.secs)
    }
}

struct SessionManagerInner {
    time_tx: mpsc::Sender<RemainingTime>,
    remaining_time: RemainingTime,
}

impl SessionManagerInner {
    fn new(score_rx: mpsc::Receiver<u32>) -> Self {
        Self { score_rx }
    }

    async fn start(&mut self) {
        loop {
            self.score_rx
        }
    }
}

pub struct Session {
    cur_score: u32,
    remaining_time: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_time_display_shows_correctly() {
        let time = RemainingTime { mins: 1, secs: 20 };
        let got = format!("{}", time);
        assert_eq!(got, "01:20");
    }
}
