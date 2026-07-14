use parking_lot::RwLock;
use std::{cell::RefCell, fmt::Display, rc::Rc, sync::Arc};

use chrono::Duration;

use crate::data::projector::ProjectorConnection;

const TIMELIMIT: RemainingTime = RemainingTime { mins: 1, secs: 30 };

pub struct SessionManager(Arc<RwLock<SessionManagerInner>>);

impl SessionManager {
    pub fn new<PT>(projector_transport: Rc<RefCell<PT>>) -> Self
    where
        PT: ProjectorConnection,
    {
        let inner = Arc::new(RwLock::new(SessionManagerInner::new()));

        slint::spawn_local({
            let mut score_rx = projector_transport.borrow_mut().take_rx().unwrap();
            let inner = Arc::clone(&inner);
            async move {
                let score = score_rx
                    .recv()
                    .await
                    .unwrap()
                    .expect("todo: receiving score from projector failed");
                let mut guard = inner.write();
                {
                    let Some(session) = &mut guard.session else {
                        panic!("recieved score from projector, but session not started");
                    };
                    session.cur_score += score;
                }
                if let Some(sub) = &guard.subscriber {
                    // session always exists (checked above)
                    sub.on_score_changed(guard.session.as_ref().unwrap().cur_score);
                }
            }
        })
        .unwrap();

        Self(inner)
    }

    pub fn subscribe(&mut self, sub: impl SessionSubscriber + 'static) {
        self.0.write().subscriber = Some(Box::new(sub));
    }
}

pub trait SessionSubscriber {
    fn on_score_changed(&self, score: u32);

    fn on_remaining_time_changed(&self, remaining_time: &RemainingTime);
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
    session: Option<Session>,
    subscriber: Option<Box<dyn SessionSubscriber + 'static>>,
}

impl SessionManagerInner {
    fn new() -> Self {
        Self {
            session: None,
            subscriber: None,
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
