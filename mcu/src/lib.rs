#![no_std]

mod ble;
mod fmt;

use bt_hci::param::ConnHandle;
use embassy_executor::Spawner;
use embassy_futures::join::*;
#[allow(unused_imports)]
use embassy_futures::select::*;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver},
};
use heapless::{Vec, index_map::FnvIndexMap};
use trouble_host::{HostResources, prelude::*};

use crate::ble::{
    CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, Server, advertise_and_handle_gatt, ble_task,
};
use crate::fmt::{info, warning};

/// frameデータをgattタスクから送るチャンネルのcapacity.
const FRAME_CHANNEL_CAP: usize = 5;

/// collectタスクで保持するframeの最大数。
const FRAME_BUF_CAP: usize = 8;

/// カメラの数
const CAMERA_NUM: usize = 2; // TODO: CONNECTIONS_MAXと揃えるべき？

struct State {
    frames: FnvIndexMap<FrameId, Vec<Frame, CAMERA_NUM>, FRAME_BUF_CAP>,
}

impl State {
    fn new() -> Self {
        Self {
            frames: FnvIndexMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FrameId(u32);

#[cfg(feature = "defmt")]
impl defmt::Format for FrameId {
    fn format(&self, fmt: defmt::Formatter) {
        self.0.format(fmt);
    }
}

/// フレームのデータ。
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct Frame {
    conn_id: ConnHandle,
    frame_id: FrameId,
    x: f32,
    y: f32,
}

impl Frame {
    fn from_bytes(data: [u8; 12], conn_id: ConnHandle) -> Self {
        let frame_id = data[..4].try_into().unwrap();
        let frame_id = FrameId(u32::from_le_bytes(frame_id));
        let x = data[4..8].try_into().unwrap();
        let x = f32::from_le_bytes(x);
        let y = data[8..].try_into().unwrap();
        let y = f32::from_le_bytes(y);
        Self {
            conn_id,
            frame_id,
            x,
            y,
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
    static FRAME_CHANNEL: Channel<CriticalSectionRawMutex, Frame, 5> = Channel::new();
    let tx = FRAME_CHANNEL.sender();
    let rx = FRAME_CHANNEL.receiver();

    // Extract frame handle once (used for all handlers)

    join3(
        ble_task(runner),
        advertise_and_handle_gatt(&mut peripheral, &server, tx),
        collect_frames(state, rx),
    )
    .await;
}

async fn collect_frames(
    mut state: State,
    rx: Receiver<'static, CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP>,
) {
    loop {
        let frame = rx.receive().await;
        let frame_id = frame.frame_id;

        let Ok(frames) = state.frames.entry(frame_id).or_default() else {
            warning!(
                "[collect] frames was full. frame {:?} was dropped",
                frame.frame_id
            )
        }
    }
}
