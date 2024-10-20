use std::error::Error;

use common::{gen_random_username, nok, ok, TestConnection};
use log::info;
use serde_json::{json, Value};

mod common;

#[tokio::test]
async fn join() -> Result<(), Box<dyn Error>> {
    let mut c = TestConnection::new().await?;

    c.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": gen_random_username(),
        "password": "pwd"
    })).await?;
    let r = c.receive_response().await?;
    ok(r.clone())?;

    c.send_message(&json!({
        "method": "new",
        "what": "group",
        "name": "TestGroup"
    })).await?;
    let r = c.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;
    let v: Value = serde_json::from_str(&r)?;
    let chat_id = v.get("chat").unwrap().get("chat_id").unwrap().as_i64().unwrap();

    c.send_message(&json!({
        "method": "new",
        "what": "invitation_link",
        "chat_id": chat_id
    })).await?;
    let r = c.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;
    let v: Value = serde_json::from_str(&r)?;
    let link = v.get("link").unwrap().as_str().unwrap();

    let mut m = TestConnection::new().await?;

    m.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": gen_random_username(),
        "password": "pwd"
    })).await?;
    let r = m.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;

    m.send_message(&json!({
        "method": "join",
        "link": link
    })).await?;
    let r = m.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;

    m.send_message(&json!({
        "method": "join",
        "link": link
    })).await?;
    let r = m.receive_response().await?;
    println!("{}", r);
    nok(r.clone())?;

    Ok(())
}
