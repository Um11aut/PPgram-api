use std::error::Error;

use common::{generate_random_string, ok, TestConnection};
use serde_json::{json, Value};

mod common;

#[tokio::test]
async fn join() -> Result<(), Box<dyn Error>> {
    let mut c = TestConnection::new("3000").await?;

    let rnd = format!("@{}", generate_random_string(5));
    let rnd1 = format!("@{}", generate_random_string(5));

    c.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": rnd,
        "password": "pwd"
    }))
    .await?;
    let r = c.receive_response().await?;
    ok(r.clone())?;

    c.send_message(&json!({
        "method": "new",
        "what": "group",
        "name": "TestGroup"
    }))
    .await?;
    let r = c.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;
    let v: Value = serde_json::from_str(&r)?;
    let chat_id = v
        .get("chat")
        .unwrap()
        .get("chat_id")
        .unwrap()
        .as_i64()
        .unwrap();

    c.send_message(&json!({
        "method": "new",
        "what": "invitation_link",
        "chat_id": chat_id
    }))
    .await?;
    let r = c.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;
    let v: Value = serde_json::from_str(&r)?;
    let link = v.get("link").unwrap().as_str().unwrap();

    let mut m = TestConnection::new("3000").await?;

    m.send_message(&json!({
        "method": "register",
        "name": "a",
        "username": rnd1,
        "password": "pwd"
    }))
    .await?;
    let r = m.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;

    m.send_message(&json!({
        "method": "join",
        "link": link
    }))
    .await?;
    let r = m.receive_response().await?;
    println!("{}", r);
    ok(r.clone())?;

    Ok(())
}
