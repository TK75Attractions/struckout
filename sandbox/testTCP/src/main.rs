use std::net::TcpListener;
use std::io::{Read, Write};
use proto::struckout::dto::v1::NetworkPacket;
use proto::struckout::dto::v1::network_packet::Payload;
use proto::struckout::debug::StringMessage;
use prost::Message;

pub mod proto {
    pub mod struckout {
        pub mod dto {
            pub mod v1
            {
                include!(concat!(env!("OUT_DIR"), "/struckout.dto.v1.rs"));
            }
        }

        pub mod debug {
            include!(concat!(env!("OUT_DIR"), "/struckout.debug.rs"));
        }
    }
}

fn main() {
    println!("Hello, world!");
    let listener = TcpListener::bind("127.0.0.1:5000")
        .expect("Failed to open the Listener");
    println!("Successfully opened the Listener"); 

    for stream in listener.incoming()
    {
        match stream {
        Ok(mut stream) => {
            println!("New connection established");
            let payload = Payload::Message(
                StringMessage {
                    message: "Hello".to_string(),
                }
            );
            let packet = NetworkPacket {
                payload: Some(payload),
            };

            let buffer = packet.encode_to_vec();

            let len = buffer.len() as u32;

            stream.write_all(&len.to_le_bytes()).expect("Failed to write to stream");
            stream.write_all(&buffer).expect("Failed to write to stream");
            stream.flush().expect("Failed to flush stream");

            loop {
                std::thread::sleep(
                    std::time::Duration::from_secs(1)
                );
            }
        }
        Err(e) => {
            println!("Failed to establish a connection: {}", e);
        }
        }
    }
}
