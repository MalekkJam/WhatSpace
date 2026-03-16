use std::io::{Read, Write};
use std::net::TcpStream;

/// Connects to a peer node and sends a message
pub fn send_message(address: &str, message: &str) {
    // Try to connect to the peer
    match TcpStream::connect(address) {
        Ok(mut stream) => {
            println!("[Client] Connected to {}", address);
            stream.write_all(message.as_bytes())
                .expect("Failed to send message");
            println!("[Client] Message sent: {}", message);

            // Read the response from the server
            let mut buffer = [0; 1024];
            stream.read(&mut buffer)
                .expect("Failed to read response");
            let response = String::from_utf8_lossy(&buffer);
            println!("[Client] Server response: {}", response);
        }
        Err(e) => {
            println!("[Client] Could not connect to {}: {}", address, e);
        }
    }
}