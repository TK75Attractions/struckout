#![no_std]
mod fmt;
mod types;
pub use types::*;

use bt_hci::param::ConnHandle;
use embassy_futures::select::*;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};

use heapless::{Deque, Vec, index_map::FnvIndexMap};

#[cfg(all(
    not(any(target_os = "linux", target_os = "macos", target_os = "windows")),
    test
))]
core::compile_error!("test cannot be ran on no_std");

/// frameデータをgattタスクから送るチャンネルのcapacity.
pub const FRAME_CHANNEL_CAP: usize = 5;

/// camera_locをgattタスクから送るチャンネルのcapacity.
pub const CAMERA_LOC_CHANNEL_CAP: usize = 2;

/// collectタスクで保持するframeの最大数。
pub const FRAME_BUF_CAP: usize = 8;

/// カメラの数
pub const CAMERA_NUM: usize = 2; // TODO: CONNECTIONS_MAXと揃えるべき？

pub struct State {
    frames: Deque<(FrameId, Vec<Frame, CAMERA_NUM>), FRAME_BUF_CAP>,
    camera_loc: FnvIndexMap<ConnHandle, CameraLocation, CAMERA_NUM>,
}

impl State {
    pub fn new() -> Self {
        Self {
            frames: Deque::new(),
            camera_loc: FnvIndexMap::new(),
        }
    }
}

pub async fn collect_frames(
    mut state: State,
    frame_rx: Receiver<'static, CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP>,
    camera_loc_rx: Receiver<
        'static,
        CriticalSectionRawMutex,
        (ConnHandle, CameraLocation),
        CAMERA_LOC_CHANNEL_CAP,
    >,
) {
    loop {
        let result = select(
            async {
                let frame = frame_rx.receive().await;
                let frame_id = frame.frame_id;
                insert_frame(&mut state.frames, frame_id, frame);
            },
            async {
                let camera_loc = camera_loc_rx.receive().await;
                state.camera_loc.insert(camera_loc.0, camera_loc.1).unwrap();
            },
        );
        /*match result {
            embassy_futures::select::Either::First(_) => {
                todo!()
            }
            embassy_futures::select::Either::Second(_) => {
                todo!()
            }
        };*/
    }
}

fn insert_frame(
    frames: &mut Deque<(FrameId, Vec<Frame, CAMERA_NUM>), FRAME_BUF_CAP>,
    frame_id: FrameId,
    new_frame: Frame,
) {
    let (_, current_frame) = if let Some(ret) = frames.iter_mut().find(|(id, _)| *id == frame_id) {
        ret
    } else {
        if frames.is_full() {
            frames.pop_front();
        };
        frames.push_back((frame_id, Vec::new())).unwrap(); // removed old frame above
        frames.get_mut(frames.len() - 1).unwrap() // just pushed above
    };
    current_frame.push(new_frame).unwrap(); // number of frame sent is constrained by CAMERA_NUM 
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use core::assert_eq;

    #[test]
    fn insert_frame_appends_frame_for_existing_frame() {
        let mut frames = Deque::new();
        let frame_id = FrameId::new(0);
        let mut frame = Vec::new();
        frame.push(Frame {
            conn_id: ConnHandle(0),
            frame_id,
            x: 50.,
            y: 300.,
            z: 200.,
        });
        frames.push_back((frame_id, frame)).unwrap();

        insert_frame(
            &mut frames,
            frame_id,
            Frame {
                conn_id: ConnHandle(1),
                frame_id,
                x: 100.,
                y: 50.,
                z: 500.,
            },
        );
    }

    #[test]
    fn insert_frame_inserts_new_frame() {
        let mut frames = Deque::new();
        let frame_id = FrameId::new(0u32);
        let frame = Frame {
            conn_id: ConnHandle(0),
            frame_id,
            x: 50.,
            y: 300.,
            z: 200.,
        };
        insert_frame(&mut frames, frame.frame_id, frame);
        assert_eq!(1, frames.len());
        assert_eq!(frame_id, frames.pop_front().unwrap().0);
    }

    #[test]
    fn insert_frame_removes_old_frame() {
        let mut frames = Deque::new();
        for i in 0..FRAME_BUF_CAP {
            frames.push_back((FrameId::new(i), Vec::new())).unwrap();
        }

        let frame_id = FrameId::new(FRAME_BUF_CAP as u32);
        let new_frame = Frame {
            conn_id: ConnHandle(0),
            frame_id,
            x: 50.,
            y: 300.,
            z: 200.,
        };
        insert_frame(&mut frames, new_frame.frame_id, new_frame);
        assert_eq!(frames.len(), FRAME_BUF_CAP);
        assert_eq!(frame_id, frames.pop_back().unwrap().0);
    }
}
