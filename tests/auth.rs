use std::error::Error;

use common::{generate_random_string, nok, ok, TestConnection};
use serde_json::{json, Value};

mod common;

#[tokio::test]
async fn register() -> Result<(), Box<dyn Error>> {
    let first_random = generate_random_string(5);

    let mut con = TestConnection::new("3000").await?;
    con.send_message(&json!({
        "method": "register",
        "username": format!("@{}", first_random),
        "name": "I am gay",
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    drop(con);
    let mut con = TestConnection::new("3000").await?;

    ok(response)?;

    con.send_message(&json!({
        "method": "register",
        "username": format!("@{}", first_random),
        "name": "I am gay",
        "password": "asd"
    })).await?;

    let response = con.receive_response().await?;
    println!("{}", response);
    nok(response)?;
    drop(con);
    let mut con = TestConnection::new("3000").await?;

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
    let mut con = TestConnection::new("3000").await?;

    let username = generate_random_string(8);
    con.send_message(&json!({
        "method": "register",
        "username": format!("@{}", username),
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
    let mut con = TestConnection::new("3000").await?;
    let username = generate_random_string(8);

    con.send_message(&json!({
        "method": "register",
        "username": format!("@{}", username),
        "name": "I am gay",
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    ok(response)?;
    drop(con);

    let mut con = TestConnection::new("3000").await?;
    con.send_message(&json!({
        "method": "login",
        "username": format!("@{}", username),
        "password": "asd"
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    let val = serde_json::from_str::<Value>(&response)?;
    let session_id_1 = val.get("session_id").unwrap().as_str().unwrap();
    ok(response)?;
    drop(con);

    let mut con = TestConnection::new("3000").await?;
    con.send_message(&json!({
        "method": "login",
        "username": format!("@{}", username),
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
    let mut con = TestConnection::new("3000").await?;
    let username = generate_random_string(8);

    con.send_message(&json!({
        "method": "register",
        "username": format!("@{}", username),
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

    let mut con = TestConnection::new("3000").await?;
    con.send_message(&json!({
        "method": "auth",
        "session_id": session_id,
        "user_id": user_id
    })).await?;
    let response = con.receive_response().await?;
    println!("{}", response);
    ok(response)?;
    drop(con);

    let mut con = TestConnection::new("3000").await?;
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
