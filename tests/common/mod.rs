use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Serialize;
use serde_json::{json, Value};
use std::{error::Error, io, path::Path};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct TestConnection {
    stream: TcpStream,
}

impl TestConnection {
    pub async fn new(port: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
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

    pub async fn upload_file(&mut self, file_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let payload = json!({
            "method": "upload_file",
            "name": "test.jpg",
            "is_media": false,
            "compress": false
        });
        self.send_message(&payload).await?;

        let files_binary = tokio::fs::read(&file_path).await?;

        let bytes = u64::to_be_bytes(files_binary.len() as u64);
        self.stream.write_all(&bytes).await?;
        self.stream.write_all(&files_binary).await?;

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

pub fn generate_random_string(length: usize) -> String {
    let charset: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut rng = thread_rng();

    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset[idx] as char
        })
        .collect()
}

pub fn nok(resp: String) -> Result<(), Box<dyn Error>> {
    let res = serde_json::from_str::<Value>(&resp)?;
    let ok = res.get("ok").unwrap();
    assert!(ok.as_bool().unwrap() == false);

    Ok(())
}
