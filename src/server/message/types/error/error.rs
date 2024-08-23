use std::{borrow::Cow, sync::Arc};

use serde_json::json;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf, sync::Mutex};

use crate::server::message::builder::MessageBuilder;

pub struct PPErrorSender {
    builder: Option<MessageBuilder>,
    error: String,
}

impl PPErrorSender 
{
    pub async fn send<T: Into<Cow<'static, str>>>(method: &str, what: T, writer: Arc<Mutex<OwnedWriteHalf>>) {
        let what: String = what.into().to_string();

        let error = json!({
            "ok": false,
            "method": method,
            "error": what
        });

        let builder = MessageBuilder::build_from(serde_json::to_string(&error).unwrap());

        let mut writer = writer.lock().await;
        writer.write_all(&builder.packed()).await.unwrap();
    }
}