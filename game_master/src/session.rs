use derive_getters::Getters;
use parking_lot::RwLock;
use slint::{Timer, TimerMode};
use std::{cell::RefCell, fmt::Display, rc::Rc, sync::Arc, time::Duration};
use tokio::sync::mpsc;

use crate::{data::projector::ProjectorConnection, ui};

const TIMELIMIT: RemainingTime = RemainingTime { mins: 1, secs: 30 };

#[derive(Debug)]
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

    /// `cb` is called when session ends.
    pub fn start_session(&mut self, difficulty: ui::Difficulity, cb: impl FnOnce() + 'static) {
        {
            let mut guard = self.0.write();
            guard.session = Some(Session::new(difficulty));
        }
        let timer = Timer::default();
        let (tx, mut rx) = mpsc::channel(1);
        timer.start(TimerMode::Repeated, Duration::from_secs(1), {
            let this = Arc::clone(&self.0);
            let tx = tx.clone();
            move || {
                {
                    let mut guard = this.write();
                    let session = guard.session.as_mut().expect("session should be started");
                    let is_end = session.remaining_time.count_down();
                    if is_end {
                        tx.blocking_send(()).unwrap();
                    }
                }
                let guard = this.read();
                let session = guard.session.as_ref().unwrap(); // checked above
                if let Some(sub) = &guard.subscriber {
                    sub.on_remaining_time_changed(&session.remaining_time);
                }
            }
        });
        slint::spawn_local(async move {
            rx.recv().await.unwrap();
            timer.stop();
            cb();
        })
        .unwrap();
    }

    /// Returns clone of current [`Session`].
    ///
    /// Clone occures due to internal [`RwLock`] borrowing.
    pub fn session(&self) -> Option<Session> {
        let guard = self.0.read();
        guard.session.as_ref().cloned()
    }

    pub fn subscribe<T>(&mut self, sub: Rc<T>)
    where
        T: SessionSubscriber + 'static,
    {
        self.0.write().subscriber = Some(sub);
    }
}

pub trait SessionSubscriber {
    fn on_score_changed(&self, score: u32);

    fn on_remaining_time_changed(&self, remaining_time: &RemainingTime);
}

#[derive(Debug, Clone)]
pub struct RemainingTime {
    pub mins: usize,
    pub secs: usize,
}

impl Display for RemainingTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.mins, self.secs)
    }
}

impl RemainingTime {
    /// Returns true if count became 00:00.
    fn count_down(&mut self) -> bool {
        if self.secs != 0 {
            self.secs -= 1;
            false
        } else {
            if self.mins == 0 {
                true
            } else {
                self.mins -= 1;
                false
            }
        }
    }
}

struct SessionManagerInner {
    session: Option<Session>,
    subscriber: Option<Rc<dyn SessionSubscriber + 'static>>,
}

impl SessionManagerInner {
    fn new() -> Self {
        Self {
            session: None,
            subscriber: None,
        }
    }
}

impl std::fmt::Debug for SessionManagerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManagerInner")
            .field("session", &self.session)
            .field(
                "subscriber",
                if self.subscriber.is_some() {
                    &"Some( .. )"
                } else {
                    &"None"
                },
            )
            .finish()
    }
}

#[derive(Debug, Clone, Getters)]
pub struct Session {
    cur_score: u32,
    difficulty: ui::Difficulity,
    remaining_time: RemainingTime,
}

impl Session {
    fn new(difficulty: ui::Difficulity) -> Self {
        Self {
            cur_score: 0,
            difficulty,
            remaining_time: TIMELIMIT,
        }
    }
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
