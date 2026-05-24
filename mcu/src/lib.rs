#![no_std]

mod ble;
mod fmt;
mod types;

use bt_hci::param::ConnHandle;
use embassy_executor::Spawner;
use embassy_futures::join::*;
#[allow(unused_imports)]
use embassy_futures::select::*;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use heapless::{Deque, Vec, index_map::FnvIndexMap};
use trouble_host::{HostResources, prelude::*};

use crate::{
    ble::{CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, Server, advertise_and_handle_gatt, ble_background},
    fmt::todo,
    types::{Frame, FrameId},
};
use crate::{
    fmt::{info, warning},
    types::CameraLocation,
};

/// frameデータをgattタスクから送るチャンネルのcapacity.
const FRAME_CHANNEL_CAP: usize = 5;

/// camera_locをgattタスクから送るチャンネルのcapacity.
const CAMERA_LOC_CHANNEL_CAP: usize = 2;

/// collectタスクで保持するframeの最大数。
const FRAME_BUF_CAP: usize = 8;

/// カメラの数
const CAMERA_NUM: usize = 2; // TODO: CONNECTIONS_MAXと揃えるべき？

struct State {
    frames: Deque<(FrameId, Vec<Frame, CAMERA_NUM>), FRAME_BUF_CAP>,
    camera_loc: FnvIndexMap<ConnHandle, CameraLocation, CAMERA_NUM>,
}

impl State {
    fn new() -> Self {
        Self {
            frames: Deque::new(),
            camera_loc: FnvIndexMap::new(),
        }
    }
}

pub async fn run<C>(controller: C, _s: Spawner)
where
    C: trouble_host::Controller,
{
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {:?}", address);

    let mut resources: HostResources<_, DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();

    let stack: Stack<'_, C, DefaultPacketPool> = trouble_host::new(controller, &mut resources)
        .set_random_address(address)
        .build();
    let runner = stack.runner();
    let mut peripheral = stack.peripheral();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "Struckout",
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();

    let state = State::new();

    static FRAME_CHANNEL: Channel<CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP> =
        Channel::new();
    let frame_tx = FRAME_CHANNEL.sender();
    let frame_rx = FRAME_CHANNEL.receiver();

    static CAMERA_LOC_CHANNEL: Channel<
        CriticalSectionRawMutex,
        (ConnHandle, CameraLocation),
        CAMERA_LOC_CHANNEL_CAP,
    > = Channel::new();
    let camera_loc_tx = CAMERA_LOC_CHANNEL.sender();
    let camera_loc_rx = CAMERA_LOC_CHANNEL.receiver();

    // Extract frame handle once (used for all handlers)

    join3(
        ble_background(runner),
        advertise_and_handle_gatt(&mut peripheral, &server, frame_tx, camera_loc_tx),
        collect_frames(state, frame_rx, camera_loc_rx),
    )
    .await;
}

async fn collect_frames(
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
