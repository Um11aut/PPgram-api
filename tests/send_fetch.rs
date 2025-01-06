use std::error::Error;

use common::{generate_random_string, ok, TestConnection};
use serde_json::{json, Value};

mod common;

#[tokio::test]
async fn send_message() -> Result<(), Box<dyn Error>> {
    let mut c = TestConnection::new("3000").await?;

    let receiver = format!("@{}", generate_random_string(10));
    let sender= format!("@{}", generate_random_string(10));

    c.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": receiver,
        "password": "pwd"
    })).await?;
    let r = c.receive_response().await?;
    ok(r.clone())?;

    let val = serde_json::from_str::<Value>(&r)?;
    let user_id = val.get("user_id").unwrap().as_i64().unwrap();
    drop(c);

    let mut c = TestConnection::new("3000").await?;
    c.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": sender,
        "password": "pwd"
    })).await?;
    ok(c.receive_response().await?)?;

    c.send_message(&json!({
        "method": "send_message",
        "to": user_id,
        "reply_to": 0,
        "content": {
            "text": "Test"
        }
    })).await?;
    ok(c.receive_response().await?)?;

    drop(c);

    let mut c = TestConnection::new("3000").await?;
    c.send_message(&json!({
        "method": "login",
        "username": receiver,
        "password": "pwd"
    })).await?;
    ok(c.receive_response().await?)?;
    c.send_message(&json!({
        "method": "fetch",
        "what": "chats"
    })).await?;
    ok(c.receive_response().await?)?;

    c.send_message(&json!({
        "method": "fetch",
        "what": "users",
        "query": "@msg"
    })).await?;
    let resp = c.receive_response().await?;
    println!("{}", resp);
    ok(resp)?;

    Ok(())
}

#[tokio::test]
async fn upload_file() -> Result<(), Box<dyn Error>> {
    let mut c = TestConnection::new("8080").await?;
    c.upload_file("/usr/src/app/Cargo.toml").await?;
    let resp = c.receive_response().await?;
    println!("{}", resp);
    ok(resp)?;

    Ok(())
}

#[tokio::test]
async fn download_file() -> Result<(), Box<dyn Error>> {
    let mut c = TestConnection::new("8080").await?;
    c.upload_file("/usr/src/app/Cargo.toml").await?;
    let resp = c.receive_response().await?;
    println!("{}", resp);
    ok(resp.clone())?;
    let val: Value = serde_json::from_str(resp.as_str())?;
    let hash = val.get("sha256_hash").unwrap().as_str().unwrap();
    c.download_file(hash).await?;

    Ok(())
}

