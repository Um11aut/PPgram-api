use std::sync::Arc;

use log::{debug, error};
use serde::Serialize;
use serde_json::Value;
use tokio::{io::AsyncWriteExt, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream}, sync::{mpsc, Mutex}};


use super::message::builder::MessageBuilder;

#[derive(Debug)]
pub struct TCPConnection {
    sender: Mutex<mpsc::Sender<Value>>,
    writer: Arc<Mutex<OwnedWriteHalf>>,
    reader: Arc<Mutex<OwnedReadHalf>>,
}

impl TCPConnection {
    pub fn new(socket: TcpStream) -> Self {
        let (reader, writer) = {
            let (r, w) = socket.into_split();

            (Arc::new(Mutex::new(r)), Arc::new(Mutex::new(w)))
        };

        let (sender, receiver) = mpsc::channel::<Value>(10);

        tokio::spawn(Self::launch_receiver_handler(Arc::clone(&writer), receiver));
        
        Self {
            sender: Mutex::new(sender),
            writer,
            reader
        }
    }

    /// Send to receiver
    pub async fn mpsc_send(&self, value: impl Serialize) {
        self.sender.lock().await.send(serde_json::to_value(&value).unwrap()).await.unwrap();
    } 

    /// Writes the data to the buffer
    pub async fn write(&self, buf: &[u8]) {
        if buf.len() < 1000 {
            debug!("Sending response!\n {}", String::from_utf8_lossy(&buf[4..]));
        }
        let mut writer = self.writer.lock().await;
        if let Err(err) = writer.write_all(buf).await {
            error!("Failed to write to the buffer: {}", err);
        }
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.reader)
    }

    async fn launch_receiver_handler(writer: Arc<Mutex<OwnedWriteHalf>>, mut receiver: mpsc::Receiver<Value>) {
        let writer = Arc::clone(&writer);

        while let Some(message) = receiver.recv().await {
            let mut writer = writer.lock().await;
            if let Err(e) = writer.write_all(&MessageBuilder::build_from_str(serde_json::to_string(&message).unwrap()).packed()).await {
                error!("Failed to send message: {}", e);
            }
        }
    }

//     async fn attempt_reconnect(writer: &Arc<Mutex<OwnedWriteHalf>>, attempts: u32) -> PPResult<()> {
//         let backoff = Duration::from_secs(5);
//     
//         warn!("Waiting for reconnect: {}", backoff);
//         sleep(backoff).await;
// 
// 
//     }
}