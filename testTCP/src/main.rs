use std::net::TcpListener;
use std::io::{Read, Write};
use proto::struck_out::dto::NetworkPacket;
use proto::struck_out::dto::network_packet::Payload;
use proto::struck_out::debug::StringMessage;
use prost::Message;

pub mod proto {
    pub mod struck_out {
        pub mod dto {
            include!(concat!(env!("OUT_DIR"), "/struck_out.dto.rs"));
        }

        pub mod debug {
            include!(concat!(env!("OUT_DIR"), "/struck_out.debug.rs"));
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
        }
        Err(e) => {
            println!("Failed to establish a connection: {}", e);
        }
        }
    }
}
