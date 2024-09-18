use super::{message::Message, request::edit::EditMessageRequest};

pub struct EditedMessageBuilder {
    pub is_unread: Option<bool>,
    pub reply_to: Option<i32>,
    pub content: Option<String>,
    pub media_hashes: Option<Vec<String>>,
    pub media_names: Option<Vec<String>>
}

impl From<EditMessageRequest> for EditedMessageBuilder {
    fn from(value: EditMessageRequest) -> Self {
        Self {
            is_unread: value.is_unread,
            reply_to: value.reply_to,
            content: value.content,
            media_hashes: value.media_hashes,
            media_names: value.media_names
        }
    }
}

impl EditedMessageBuilder {
    pub fn get_edited_message(self, msg: Message) -> Message {
        let unread_changed: bool = if let Some(unread) = self.is_unread {
            if !unread {
                true
            } else {false}
        } else {msg.is_unread};

        let reply_to_changed: Option<i32> = if let Some(reply_to) = self.reply_to {
            Some(reply_to)
        } else {msg.reply_to};

        let content_changed: Option<String> = if let Some(content) = self.content {
            Some(content)
        } else {msg.content};

        let media_hashes_changed = if let Some(media_hashes) = self.media_hashes {
            media_hashes
        } else {msg.media_hashes};
        
        let media_names_changed = if let Some(media_names) = self.media_names {
            media_names
        } else {msg.media_names};

        Message {
            is_unread: unread_changed,
            reply_to: reply_to_changed,
            content: content_changed,
            media_hashes: media_hashes_changed,
            media_names: media_names_changed,
            ..msg
        }
    }
}