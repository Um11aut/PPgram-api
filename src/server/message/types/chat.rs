pub(crate) struct ResponeChatInfo {
    pub(crate) name: String,
    pub(crate) photo: Vec<u8>,
    pub(crate) username: String,
}

pub(crate) struct ChatInfo {
    pub(crate) chat_id: i32,
    pub(crate) is_group: bool,
    pub(crate) participants: Vec<i32>
}