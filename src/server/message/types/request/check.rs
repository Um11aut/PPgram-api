use std::borrow::Cow;

use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize)]
pub struct CheckUsernameRequest<'a> {
    pub method: Cow<'a, str>,
    pub what: Cow<'a, str>,
    pub data: Cow<'a, str>
}