use std::{error::Error, fmt::format, io};
use serde_json::Value;
use tokio::{net::TcpStream, io::{AsyncReadExt, AsyncWriteExt}};
use serde::Serialize;

pub struct TestConnection {
    stream: TcpStream,
}

impl TestConnection {
    pub async fn new() -> io::Result<Self> {
        let stream = TcpStream::connect("127.0.0.1:3000").await?;
        Ok(Self { stream })
    }

    pub async fn send_message<T: Serialize>(&mut self, message: &T) -> io::Result<()> {
        let msg = serde_json::to_string(&message)?;

        let len = (msg.len() as u32).to_be_bytes();

        let mut output_vec: Vec<u8> = Vec::with_capacity(len.len() + msg.len());
        output_vec.extend_from_slice(&len);
        output_vec.extend_from_slice(msg.as_bytes());

        self.stream.write_all(&output_vec).await?;
        Ok(())
    }

    pub async fn receive_response(&mut self) -> Result<String, Box<dyn Error>> {
        let mut size_buffer = [0; 4]; // Buffer to read message size
        self.stream.read_exact(&mut size_buffer).await?;
        
        let expected_size = u32::from_be_bytes(size_buffer) as usize;
        assert!(expected_size < 1_000_000_000);

        let mut response = vec![0; expected_size];
        self.stream.read_exact(&mut response).await?;

        Ok(String::from_utf8(response)?)
    }
}


pub fn ok(resp: String) -> Result<(), Box<dyn Error>> {
    let res = serde_json::from_str::<Value>(&resp)?;
    let ok = res.get("ok").unwrap();
    assert!(ok.as_bool().unwrap() == true);

    Ok(())
}

pub fn nok(resp: String) -> Result<(), Box<dyn Error>> {
    let res = serde_json::from_str::<Value>(&resp)?;
    let ok = res.get("ok").unwrap();
    assert!(ok.as_bool().unwrap() == false);

    Ok(())
}

use rand::{distributions::Alphanumeric, seq::SliceRandom, Rng};
use rand::distributions::Uniform;

pub fn gen_random_username() -> String {
    // Define the character set: lowercase, uppercase, and underscore
    let allowed_chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_".chars().collect();
    let mut rng = rand::thread_rng();
    let username: String = (0..10)
        .map(|_| *allowed_chars.choose(&mut rng).unwrap()) // Randomly select from the allowed characters
        .collect();
    format!("@{}", username)
}