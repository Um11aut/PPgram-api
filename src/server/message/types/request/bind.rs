use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BindRequest<'a> {
    pub method: Cow<'a, str>,
    pub session_id: Cow<'a, str>,
    pub user_id: i32
}