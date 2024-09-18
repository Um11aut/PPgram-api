use std::error::Error;

use common::TestConnection;
use serde_json::{json, Value};

mod common;

#[test]
fn auth_all() -> Result<(), Box<dyn Error>> {
    let mut con = TestConnection::new()?;

    con.send_message(&json!({
        "method": "register",
        "username": "@PepukPidaras",
        "name": "I am gay",
        "password_hash": "asd"
    }))?;

    let response = con.receive_response()?;
    println!("{}", response);

    let res = serde_json::from_str::<Value>(&response)?;
    let maybe_method = res.get("ok");
    assert!(maybe_method.is_some());
    let method = maybe_method.unwrap();
    assert!(method.as_str().is_some());
    assert!(method.as_str().unwrap() == "ok");


    Ok(())
}