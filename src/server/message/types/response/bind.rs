use std::borrow::Cow;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct BindReponse<'a> {
    pub ok: bool,
    pub method: Cow<'a, str>,
}