use super::message::{handlers::{files_handler::FilesHandler, json_handler::{self, JsonHandler}}, Handler};

pub struct PendingConnection {
    pub connection_id: String,
    pub handler: Box<dyn Handler>,
}
unsafe impl Send for PendingConnection {}
unsafe impl Sync for PendingConnection {}
