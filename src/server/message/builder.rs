use std::{borrow::Cow, sync::Arc};

use log::info;

// The default message contains the size of it (u32 4 bytes)
// and the content(the rest of it)
#[derive(Clone)]
pub struct MessageBuilder {
    size: u32,
    content: String
}

impl MessageBuilder {
    pub fn build_from<T: Into<Cow<'static, str>>>(message: T) -> Self {
        let message: Cow<'static, str> = message.into();
        let message = message.into_owned();

        let size = message.len() as u32;

        Self {
            size,
            content: message,
        }
    }

    pub fn parse(message: &[u8]) -> Option<Self> {
        if message.len() < 4 {
            return None;
        }

        let message = message.to_vec();
        let size_bytes = &message[..4];
        let size = u32::from_be_bytes([size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]]);

        let content: Vec<u8>;
        if size < message.len() as u32 {
            content = (&message[4..size as usize + 4]).to_vec();
        } else {
            content = (&message[4..]).to_vec();
        }

        if let Ok(content) = String::from_utf8(content) {
            return Some(
                Self {
                    size,
                    content
                }
            );
        }
        None
    }

    pub fn extend(&mut self, buffer: &[u8]) 
    {
        unsafe { self.content.as_mut_vec().extend_from_slice(buffer) };
    }

    pub fn ready(&self) -> bool {
        self.content.len() >= self.size as usize
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.size = 0;
    }

    pub fn content(&self) -> &String {
        &self.content
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn packed(&self) -> Vec<u8> {
        let size_bytes = self.size.to_be_bytes();

        let full_len = self.content.len() + self.size as usize;
        let mut full_message: Vec<u8> = Vec::with_capacity(full_len);
        full_message.extend_from_slice(&size_bytes);
        full_message.extend_from_slice(&self.content.as_bytes());

        full_message
    }
}