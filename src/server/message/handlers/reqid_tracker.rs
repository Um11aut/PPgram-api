use crate::db::internal::error::{PPError, PPResult};

use super::json_handler::JsonHandler;

pub fn handle_request_id(handler: &mut JsonHandler) -> PPResult<()> {
    let msg = handler.utf8_content_unchecked();
    let root = serde_json::from_str::<serde_json::Value>(msg).unwrap();
    let req_id = root.get("req_id").unwrap().as_i64();
    if let Some(request_id) = req_id {
        handler.set_latest_req_id(request_id);
        return Ok(());
    }

    Err(PPError::from("Provided req_id must be of type i64!"))
}
