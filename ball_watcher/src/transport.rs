use std::net::SocketAddr;

use prost::Message;

use crate::{
    protobuf::{self, packet::Body::SendFrame},
    types::CameraLocation,
    types::Frame,
};

/// Socket to receive frames from cameras.
pub trait FrameSocket {
    fn bind(addr: SocketAddr) -> std::io::Result<Self>;
    async fn recv_from(&self, buf: &mut [u8]) -> std::io::Result<(usize, SocketAddr)>;
}

pub struct FrameHandler<S> {
    socket: S,
    buf: Vec<u8>,
}

impl<S> FrameHandler<S>
where
    S: FrameSocket,
{
    fn new(addr: SocketAddr) -> std::io::Result<Self> {
        Ok(Self {
            socket: S::bind(addr)?,
            buf: Vec::new(),
        })
    }

    async fn receive(&mut self) -> std::io::Result<()> {
        let (len, addr) = self.socket.recv_from(&mut self.buf).await?;
        let packet = protobuf::Packet::decode(&mut self.buf)?;
        match packet.body.unwrap() {
            SendFrame(send_frame) => todo!(),
        }
        todo!();
        Ok(())
    }
}

/// Used to receive camera location from cameras.
pub trait CameraLocationListener {
    // TODO
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

mod tests {}
