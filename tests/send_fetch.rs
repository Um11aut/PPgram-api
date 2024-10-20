use std::error::Error;

use common::{gen_random_username, nok, ok, TestConnection};
use log::info;
use serde_json::{json, Value};

mod common;

#[tokio::test]
async fn send_message() -> Result<(), Box<dyn Error>> {
    let mut c = TestConnection::new().await?;

    c.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": gen_random_username(),
        "password": "pwd"
    })).await?;
    let r = c.receive_response().await?;
    ok(r.clone())?;

    let val = serde_json::from_str::<Value>(&r)?;
    let user_id = val.get("user_id").unwrap().as_i64().unwrap();
    drop(c);

    let mut c = TestConnection::new().await?;
    c.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": gen_random_username(),
        "password": "pwd"
    })).await?;
    ok(c.receive_response().await?)?;

    c.send_message(&json!({
        "method": "send_message",
        "to": user_id,
        "has_reply": false,
        "reply_to": 0,
        "content": {
            "text": "Test"
        }
    })).await?;
    ok(c.receive_response().await?)?;

    drop(c);

    let mut c = TestConnection::new().await?;
    c.send_message(&json!({
        "method": "login",
        "username": gen_random_username(),
        "password": "pwd"
    })).await?;
    ok(c.receive_response().await?)?;
    c.send_message(&json!({
        "method": "fetch",
        "what": "chats"
    })).await?;
    ok(c.receive_response().await?)?;

    
    Ok(())
}
