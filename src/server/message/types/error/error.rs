use std::{borrow::Cow, sync::Arc};

use serde_json::json;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::server::message::builder::Message;

pub struct PPgramError {
    builder: Option<Message>,
    error: String,
}

impl PPgramError 
{
    pub async fn send<T: Into<Cow<'static, str>>>(method: &str, what: T, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let what: String = what.into().to_string();

        let error = json!({
            "ok": false,
            "method": method,
            "error": what
        });

        let builder = Message::build_from(serde_json::to_string(&error).unwrap());

        let mut writer = writer.lock().await;
        writer.write_all(builder.packed().as_bytes()).await.unwrap();
    }
}