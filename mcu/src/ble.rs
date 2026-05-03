use embassy_futures::join::*;
use embassy_futures::select::*;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use trouble_host::prelude::*;

use crate::fmt::{info, warning};
use crate::{FRAME_CHANNEL_CAP, Frame};

use crate::Server;

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
pub async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    loop {
        if let Err(e) = runner.run().await {
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

pub async fn advertise_and_handle_gatt<'values, 'server, C: Controller>(
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
    tx: Sender<'static, CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP>,
) {
    // Track connections in slots. Slot value is taken when being handled.
    let mut conn_slot_0: Option<GattConnection<'_, '_, DefaultPacketPool>> = None;
    let mut conn_slot_1: Option<GattConnection<'_, '_, DefaultPacketPool>> = None;
    let frame_handle = server.service.frame.handle;

    loop {
        // Race: accept new connection OR handle existing connections
        let result = select(
            async {
                // Try to accept a new connection
                match advertise("Struckout", peripheral, &server).await {
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
                    warning!("[adv] both slots full, rejecting connection");
                }
            }
            embassy_futures::select::Either::Second(_) => {
                // Handler finished, will loop and try again
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
    conn: GattConnection<'_, '_, P>,
    frame_handle: u16,
    tx: Sender<'static, CriticalSectionRawMutex, Frame, FRAME_CHANNEL_CAP>,
) {
    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                match &event {
                    GattEvent::Read(_event) => {}
                    GattEvent::Write(event) => {
                        if event.handle() == frame_handle {
                            let val: [u8; 8] = match event.data().try_into() {
                                Ok(val) => val,
                                Err(e) => {
                                    warning!(
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
    if result
        .as_ref()
        .is_err_and(|e| matches!(e, Error::ConnectionLimitReached))
    {
        warning!("[adv] reached to connection capacity");
    }
    let conn = result?;
    info!("[adv] connection established");

    Ok(conn)
}
