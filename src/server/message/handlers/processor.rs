use std::sync::Arc;

use crate::{db::bucket::DatabaseBucket, server::{connection::TCPConnection, server::Sessions}};

use super::tcp_handler::SesssionArcRwLock;

pub struct JsonMessageProcessor
{
    message: Arc<String>,
    pub session: SesssionArcRwLock,
    pub sessions: Sessions,
    // Output TCP connection on which all the responses/messages are sent
    pub output_connection: Arc<TCPConnection>,
    bucket: DatabaseBucket
}