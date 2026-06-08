use std::net::TcpListener;
use std::io::{Read, Write};

fn main() {
    println!("Hello, world!");
    let listener = TcpListener::bind("127.0.0.1:5000")
        .expect("Failed to open the Listener");
    println!("Successfully opened the Listener"); 

    for stream in listener.incoming()
    {
        match stream {
        Ok(stream) => {
            stream.write_all("Hello\n".as_bytes()).expect("Failed to write to stream");
            stream.flush().expect("Failed to flush stream");
            println!("New connection established");
        }
        Err(e) => {
            println!("Failed to establish a connection: {}", e);
        }
    }
}
