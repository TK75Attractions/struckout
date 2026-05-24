use bt_hci::param::ConnHandle;
#[allow(unused_imports)]
use embassy_futures::join::*;
use embassy_futures::select::*;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use trouble_host::prelude::*;

use crate::fmt::{info, warning};
use crate::{CAMERA_LOC_CHANNEL_CAP, CameraLocation, FRAME_CHANNEL_CAP, Frame};

/// Max number of connections
pub const CONNECTIONS_MAX: usize = 2;

/// Max number of L2CAP channels.
pub const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att

type FrameSender = Sender<'static, CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP>;
type CamaraLocSender =
    Sender<'static, CriticalSectionRawMutex, (ConnHandle, CameraLocation), CAMERA_LOC_CHANNEL_CAP>;

/// GATT Server definition
#[gatt_server(connections_max=CONNECTIONS_MAX)]
pub struct Server {
    service: Service,
}

/// Gatt service definition
#[gatt_service(uuid = "d575b50d-cfd8-4747-b6cd-1aa0ffce1108")]
struct Service {
    /// f32をx,yの順にバイト列化したもの(little-endian)
    /// | x (4byte) | y (4byte) | z (4byte)
    #[characteristic(uuid = "a4b3a793-ff34-47a0-847b-32b54cba0d6f", write_without_response)]
    camera_loc: [u8; 12],

    /// | frame_id (4byte) | x (4byte) | y (4byte) | z (4byte)
    #[characteristic(uuid = "bda5d9c9-0c9a-4e45-b20b-1fb937e71a7d", write_without_response)]
    frame: [u8; 16],
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
pub async fn ble_background<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

pub async fn advertise_and_handle_gatt<'values, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'_ Server<'values>,
    frame_tx: FrameSender,
    camera_loc_tx: CamaraLocSender,
) {
    // Track connections in slots. Slot value is taken when being handled.
    let mut conn_slot_0: Option<GattConnection<'_, '_, DefaultPacketPool>> = None;
    let mut conn_slot_1: Option<GattConnection<'_, '_, DefaultPacketPool>> = None;
    let frame_handle = server.service.frame.handle;
    let camera_handle = server.service.camera_loc.handle;

    loop {
        // Race: accept new connection OR handle existing connections
        let result = select3(
            async {
                if conn_slot_0.is_none() || conn_slot_1.is_none() {
                    // Try to accept a new connection
                    match advertise("Struckout", peripheral, server).await {
                        Ok(conn) => Some(conn),
                        Err(e) => {
                            panic!("[adv] error: {:?}", e);
                        }
                    }
                } else {
                    info!("[adv] slots are full, stop advertising");
                    core::future::pending::<Option<GattConnection<DefaultPacketPool>>>().await;
                    None
                }
            },
            async {
                // Try to handle connections from slots
                // If slot 0 has a connection, handle it
                if let Some(conn) = &conn_slot_0 {
                    info!("[handler] waiting for event on slot 0");
                    gatt_handler(conn, frame_handle, camera_handle, frame_tx, camera_loc_tx).await;
                    info!("[handler] slot 0 connection closed");
                    return;
                }
                core::future::pending::<()>().await;
            },
            async {
                // If slot 1 has a connection, handle it
                if let Some(conn) = &conn_slot_1 {
                    info!("[handler] waiting for event on slot 1");
                    gatt_handler(conn, frame_handle, camera_handle, frame_tx, camera_loc_tx).await;
                    info!("[handler] slot 1 connection closed");
                    return;
                }
                core::future::pending::<()>().await;
            },
        )
        .await;

        // Handle the result of select
        match result {
            embassy_futures::select::Either3::First(Some(conn)) => {
                // Place new connection in empty slot
                if conn_slot_0.is_none() {
                    conn_slot_0 = Some(conn);
                    info!("[adv] connection placed in slot 0");
                } else if conn_slot_1.is_none() {
                    conn_slot_1 = Some(conn);
                    info!("[adv] connection placed in slot 1");
                } else {
                    unreachable!("advertising should be stopped when all slots are full");
                }
            }
            embassy_futures::select::Either3::Second(_) => {
                info!("[adv] connection on slot 0 closed");
            }
            embassy_futures::select::Either3::Third(_) => {
                info!("[adv] connection on slot 1 closed");
            }
            _ => {}
        }
    }
}

/// Stream events for a single connection until it closes.
///
/// This function will handle the GATT events and process them.
/// This is how we interact with read and write requests.
/// Called from the advertising loop when a new connection is accepted.
async fn gatt_handler<P: PacketPool>(
    conn: &GattConnection<'_, '_, P>,
    frame_handle: u16,
    camera_handle: u16,
    frame_tx: FrameSender,
    camera_loc_tx: CamaraLocSender,
) {
    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    GattEvent::Read(_event) => {}
                    GattEvent::Write(event) => {
                        if event.handle() == frame_handle {
                            let data = event.data();
                            let data: [u8; 16] = match data.try_into() {
                                Ok(val) => val,
                                Err(_) => {
                                    warning!(
                                        "[gatt] event data for writing frame was incorrect: {:?}",
                                        data
                                    );
                                    return;
                                }
                            };
                            let frame = Frame::from_bytes(data, conn.raw().handle());
                            frame_tx.send(frame).await;
                        }
                        if event.handle() == camera_handle {
                            let data = event.data();
                            let data = match event.data().try_into() {
                                Ok(v) => v,
                                Err(_) => {
                                    warning!(
                                        "[gatt] event data for camera location was incorrect: {:?}",
                                        data
                                    );
                                    return;
                                }
                            };
                            let loc = CameraLocation::from_bytes(data);
                            camera_loc_tx.send((conn.raw().handle(), loc)).await;
                        }
                    }
                    _ => (),
                };
                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => warning!("[gatt] error while sending response: {:?}", e),
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

    let conn = result?;
    info!("[adv] connection established");

    Ok(conn)
}
