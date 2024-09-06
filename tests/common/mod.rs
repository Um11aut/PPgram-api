use std::{error::Error, io::{self, Read, Write}, net::TcpStream};

use serde::Serialize;

pub struct TestConnection {
    stream: TcpStream
}

impl TestConnection {
    pub fn new() -> io::Result<Self> {
        let stream = TcpStream::connect("127.0.0.1:8080")?;

        Ok(Self { stream })
    }

    pub fn send_message<T: Serialize>(&mut self, message: &T) -> io::Result<()> {
        let msg = serde_json::to_string(&message)?;

        let len = msg.len().to_be_bytes();
        
        let mut output_vec: Vec<u8> = Vec::with_capacity(msg.len() + 4);
        output_vec.extend_from_slice(&len);
        output_vec.extend_from_slice(&msg.as_bytes());

        self.stream.write_all(&output_vec)?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn receive_response(&mut self) -> Result<String, Box<dyn Error>> {
        let mut buffer = [0; 4]; // Message size
        let n = self.stream.read(&mut buffer)?;
        assert_eq!(n, 4);

        let expected_size = u32::from_be_bytes(buffer) as usize;
        assert!(expected_size < 1_000_000_000);
        
        let mut response: Vec<u8> = vec![]; 

        while response.len() <= expected_size {
            let mut buffer = [0; 65535];
            let n = self.stream.read(&mut buffer)?;
            response.extend_from_slice(&buffer[0..n]);
        }

        Ok(String::from_utf8(response)?)
    }
}