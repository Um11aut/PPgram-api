use serde::{Deserialize, Serialize};


// Define a struct to represent text messages
#[derive(Debug, Serialize, Deserialize)]
pub struct TextMessage {
    pub text: String,
}