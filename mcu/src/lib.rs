#![no_std]

mod fmt;

use bt_hci::param::ConnHandle;
use embassy_executor::Spawner;
use embassy_futures::join::join3;
use embassy_futures::select::select;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use heapless::{Vec, index_map::FnvIndexMap};
use trouble_host::{HostResources, prelude::*};

/// Max number of connections
const CONNECTIONS_MAX: usize = 2;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

/// frameデータをgattタスクから送るチャンネルのcapacity.
const FRAME_CHANNEL_CAP: usize = 5;

/// collectタスクで保持するframeの最大数。
const FRAME_BUF_CAP: usize = 8;

/// カメラの数
const CAMERA_NUM: usize = 2;

/// GATT Server definition
#[gatt_server(connections_max=CONNECTIONS_MAX)]
struct Server {
    service: Service,
}

/// Gatt service definition
#[gatt_service(uuid = "d575b50d-cfd8-4747-b6cd-1aa0ffce1108")]
struct Service {
    /// f32をx,yの順にバイト列化したもの(little-endian)
    /// | x (4byte) | y (4byte) |
    #[characteristic(uuid = "a4b3a793-ff34-47a0-847b-32b54cba0d6f", write)]
    camera_loc: [u8; 8],

    /// | frame_id (4byte) | x (4byte) | x (4byte) | (little-endian)
    #[characteristic(uuid = "bda5d9c9-0c9a-4e45-b20b-1fb937e71a7d", write)]
    frame: [u8; 12],
}

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

pub async fn run<C>(controller: C, s: Spawner)
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
    let frame_handle = server.service.frame.handle;

    join3(
        ble_task(runner),
        async move {
            // Track connections in slots. Slot value is taken when being handled.
            let mut conn_slot_0: Option<GattConnection<'_, '_, DefaultPacketPool>> = None;
            let mut conn_slot_1: Option<GattConnection<'_, '_, DefaultPacketPool>> = None;

            loop {
                // Race: accept new connection OR handle existing connections
                let result = select(
                    async {
                        // Try to accept a new connection
                        match advertise("Struckout", &mut peripheral, &server).await {
                            Ok(conn) => Some(conn),
                            Err(e) => {
                                panic!("[adv] error: {:?}", e);
                            }
                        }
                    },
                    async {
                        // Try to handle connections from slots
                        // If slot 0 has a connection, handle it
                        if let Some(conn) = conn_slot_0.take() {
                            gatt_handler(conn, frame_handle, tx.clone()).await;
                            info!("[handler] slot 0 connection closed");
                            return;
                        }

                        // If slot 1 has a connection, handle it
                        if let Some(conn) = conn_slot_1.take() {
                            gatt_handler(conn, frame_handle, tx.clone()).await;
                            info!("[handler] slot 1 connection closed");
                            return;
                        }

                        // If no connections to handle, wait indefinitely
                        core::future::pending::<()>().await;
                    },
                )
                .await;

                // Handle the result of select
                match result {
                    embassy_futures::select::Either::First(Some(conn)) => {
                        // Place new connection in empty slot
                        if conn_slot_0.is_none() {
                            info!("[adv] connection placed in slot 0");
                            conn_slot_0 = Some(conn);
                        } else if conn_slot_1.is_none() {
                            info!("[adv] connection placed in slot 1");
                            conn_slot_1 = Some(conn);
                        } else {
                            warn!("[adv] both slots full, rejecting connection");
                        }
                    }
                    embassy_futures::select::Either::Second(_) => {
                        // Handler finished, will loop and try again
                    }
                    _ => {}
                }
            }
        },
        async move {
            loop {
                let frame = rx.receive().await;
                info!("[collect] received frame: {:?}", frame);
            }
        },
    )
    .await;
}

/// This is a background task that is required to run forever alongside any other BLE tasks.
///
/// ## Alternative
///
/// If you didn't require this to be generic for your application, you could statically spawn this with i.e.
///
/// ```rust,ignore
///
/// #[embassy_executor::task]
/// async fn ble_task(mut runner: Runner<'static, SoftdeviceController<'static>>) {
///     runner.run().await;
/// }
///
/// spawner.must_spawn(ble_task(runner));
/// ```
async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

/// Stream events for a single connection until it closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
/// Called from the advertising loop when a new connection is accepted.
async fn gatt_handler<P: PacketPool>(
    conn: GattConnection<'_, '_, P>,
    frame_handle: u16,
    tx: Sender<'static, CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP>,
) {
    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    GattEvent::Read(event) => {}
                    GattEvent::Write(event) => {
                        if event.handle() == frame_handle {
                            let val: [u8; 8] = match event.data().try_into() {
                                Ok(val) => val,
                                Err(e) => {
                                    warn!(
                                        "[gatt] event data for writing frame was incorrect: {:?}",
                                        e
                                    );
                                    return;
                                }
                            };
                            //server.set(&frame, &val).unwrap();
                            info!("[gatt] set new frame value: {:?}", val);
                        }
                    }
                    _ => (),
                };
                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("[gatt] error while sending response: {:?}", e),
                }
            }
            _ => (),
        };
    };
    info!("[gatt] disconnected: {:?}", reason);
}

async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::IncompleteServiceUuids16(&[[0x0f, 0x18]]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let result = advertiser
        .accept()
        .await?
        .with_attribute_server::<_, _, _, 2>(&server.server);
    if result
        .as_ref()
        .is_err_and(|e| matches!(e, Error::ConnectionLimitReached))
    {
        warn!("[adv] reached to connection capacity");
    }
    let conn = result?;
    info!("[adv] connection established");

    Ok(conn)
}

#[embassy_executor::task]
async fn colect_frame_task(
    mut state: State,
    rx: Receiver<'static, CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP>,
) {
    loop {
        let frame = rx.receive().await;
        let frames = state
            .frames
            .entry(frame.frame_id)
            .or_insert(Vec::new())
            .unwrap();
        if let Err(frame) = frames.push(frame) {
            warn!(
                "[collect] frames was full. frame {:?} was dropped",
                frame.frame_id
            )
        }
    }
}
