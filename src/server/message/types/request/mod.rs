use serde::{Deserialize, Serialize};

use crate::db::internal::error::PPResult;

pub mod auth;
pub mod send;
pub mod bind;
pub mod check;
pub mod fetch;
pub mod edit;
pub mod delete;
pub mod new;
pub mod join;

#[derive(Serialize, Deserialize)]
struct BaseWhatRequest {
    pub what: String
}
pub fn extract_what_field(message: &String) -> PPResult<String> {
    let o = serde_json::from_str::<BaseWhatRequest>(&message)?;

    Ok(o.what)
}