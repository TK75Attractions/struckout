#![no_std]

mod ble;

use bt_hci::param::ConnHandle;
use embassy_executor::Spawner;
use embassy_futures::join::*;
#[allow(unused_imports)]
use embassy_futures::select::*;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use struckout_mcu_core::{
    CAMERA_LOC_CHANNEL_CAP, CameraLocation, FRAME_CHANNEL_CAP, Frame, State, collect_frames,
};
use trouble_host::{HostResources, prelude::*};

use crate::ble::{
    CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, Server, advertise_and_handle_gatt, ble_background,
};
use struckout_mcu_core::info;

#[cfg(not(any(target_arch = "xtensa", target_arch = "riscv32")))]
compile_error!("This crate only supports ESP devices");

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
