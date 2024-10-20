use std::error::Error;

use common::{gen_random_username, nok, ok, TestConnection};
use log::info;
use serde_json::{json, Value};

mod common;

#[tokio::test]
async fn register() -> Result<(), Box<dyn Error>> {
    let mut con = TestConnection::new().await?;
    con.send_message(&json!({
        "method": "register",
        "username": "@fsdfsd",
        "name": "I am gay",
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    drop(con);
    let mut con = TestConnection::new().await?;

    ok(response)?;

    con.send_message(&json!({
        "method": "register",
        "username": "@fsdfsd",
        "name": "I am gay",
        "password": "asd"
    })).await?;

    let response = con.receive_response().await?;
    println!("{}", response);
    nok(response)?;
    drop(con);
    let mut con = TestConnection::new().await?;

    con.send_message(&json!({
        "method": "register",
        "username": "@123]]",
        "name": "I am gay",
        "password": "asd"
    })).await?;

    let response = con.receive_response().await?;
    println!("{}", response);
    nok(response)?;
    drop(con);
    let mut con = TestConnection::new().await?; 
    con.send_message(&json!({
        "method": "register",
        "username": "@fdf",
        "name": "I am gay",
        "password": "asd"
    })).await?;

    let response = con.receive_response().await?;
    println!("{}", response);
    ok(response)?;

    Ok(())
}

#[tokio::test]
async fn login() -> Result<(), Box<dyn Error>> {
    let mut con = TestConnection::new().await?;

    con.send_message(&json!({
        "method": "register",
        "username": "@asdadassdasd",
        "name": "I am gay",
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    ok(response)?;
    drop(con);

    let mut con = TestConnection::new().await?;
    con.send_message(&json!({
        "method": "login",
        "username": "@asdadassdasd",
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    let val = serde_json::from_str::<Value>(&response)?;
    let session_id_1 = val.get("session_id").unwrap().as_str().unwrap();
    ok(response)?;
    drop(con);

    let mut con = TestConnection::new().await?;
    con.send_message(&json!({
        "method": "login",
        "username": "@asdadassdasd",
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    let val = serde_json::from_str::<Value>(&response)?;
    let session_id_2 = val.get("session_id").unwrap().as_str().unwrap();
    println!("{}", response);
    ok(response)?;
    drop(con);

    assert!(session_id_1 != session_id_2);

    Ok(())
}

#[tokio::test]
async fn auth() -> Result<(), Box<dyn Error>> {
    let mut con = TestConnection::new().await?;

    con.send_message(&json!({
        "method": "register",
        "username": gen_random_username(),
        "name": "I am gay",
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    ok(response.clone())?;
    let val = serde_json::from_str::<Value>(&response)?;
    let session_id = val.get("session_id").unwrap().as_str().unwrap();
    let user_id = val.get("user_id").unwrap().as_i64().unwrap();

    drop(con);

    let mut con = TestConnection::new().await?;
    con.send_message(&json!({
        "method": "auth",
        "session_id": session_id,
        "user_id": user_id
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    ok(response)?;
    drop(con);

    let mut con = TestConnection::new().await?;
    con.send_message(&json!({
        "method": "auth",
        "session_id": session_id,
        "user_id": user_id
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    ok(response)?;
    drop(con);

    Ok(())
}