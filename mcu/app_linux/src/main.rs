use std::net::UdpSocket;

use struckout_mcu_core::{FrameSocket, Platform};

fn main() {
    println!("Hello, world!");
}

struct LinuxPlatform{
}

impl Platform for LinuxPlatform{
    type FrameSocket = TokioUdpSocket;
}

struct TokioUdpSocket {
    socket:tokio::net::UdpSocket
}

impl FrameSocket for TokioUdpSocket {
    fn bind(addr: core::net::SocketAddr)->; {
        
    }
}
