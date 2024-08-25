use std::sync::Arc;

use log::{error, info};
use serde::Serialize;
use serde_json::Value;
use tokio::{io::AsyncWriteExt, net::{tcp::{OwnedReadHalf, OwnedWriteHalf}, TcpStream}, sync::{mpsc, Mutex}};

use super::message::builder::MessageBuilder;

#[derive(Debug)]
pub struct Connection {
    sender: Mutex<mpsc::Sender<Value>>,
    writer: Arc<Mutex<OwnedWriteHalf>>,
    reader: Arc<Mutex<OwnedReadHalf>>,
}

impl Connection {
    pub fn new(socket: TcpStream) -> Self {
        let (reader, writer) = {
            let (r, w) = socket.into_split();

            (Arc::new(Mutex::new(r)), Arc::new(Mutex::new(w)))
        };

        let (sender, receiver) = mpsc::channel::<Value>(10);

        tokio::spawn(Self::receiver_handler(Arc::clone(&writer), receiver));
        
        Self {
            sender: Mutex::new(sender),
            writer,
            reader
        }
    }

    pub async fn send(&self, value: impl Serialize) {
        self.sender.lock().await.send(serde_json::to_value(&value).unwrap()).await.unwrap();
    } 

    pub async fn write(&self, buf: &[u8]) {
        let mut writer = self.writer.lock().await;
        writer.write_all(buf).await.unwrap();
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.reader)
    }

    async fn receiver_handler(writer: Arc<Mutex<OwnedWriteHalf>>, mut receiver: mpsc::Receiver<Value>) {
        let writer = Arc::clone(&writer);

        while let Some(message) = receiver.recv().await {
            let mut writer = writer.lock().await;
            if let Err(e) = writer.write_all(&MessageBuilder::build_from(serde_json::to_string(&message).unwrap()).packed()).await {
                error!("Failed to send message: {}", e);
            }
        }
    }
}