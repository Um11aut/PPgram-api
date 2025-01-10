use std::sync::Arc;

use log::{debug, info};
use serde::Serialize;
use serde_json::Value;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::{mpsc, Mutex, RwLock};

use crate::db::bucket::{DatabaseBucket, DatabaseBuilder};
use crate::db::internal::error::PPError;
use crate::server::connection::TCPConnection;
use crate::server::message::builder::MessageBuilder;
use crate::server::message::methods::{auth, bind, check, edit, fetch, join, new, send};
use crate::server::message::types::response::events::IsTypingEvent;
use crate::server::message::types::user::UserId;
use crate::server::message::Handler;
use crate::server::server::Sessions;
use crate::server::session::Session;

pub type SessionArcRwLock = Arc<RwLock<Session>>;

const IS_TYPING_SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(1000);

const MAX_JSON_MSG_SIZE: u32 = 4096 /* 4kb */;

/// used by the channels to send is_typing event
/// `Vec<UserId>` is the users, to which this event will be sent
pub type TypingEventMsg = (IsTypingEvent, Vec<UserId>);

/// Message handler struct that has everything it needs to have to be able
/// to handle any JSON message type
///
/// TODO: Refactor to ensure SRP
pub struct JsonHandler {
    builder: Option<MessageBuilder>,
    is_message_first: bool,
    pub session: SessionArcRwLock,
    pub sessions: Sessions,
    /// Output TCP connection on which all the responses/messages are sent
    pub output_connection: Arc<TCPConnection>,
    bucket: DatabaseBucket,
    /// Mpsc Sender on receiver task for is_typing event
    /// TODO: Maybe better 'is_typing' event sending?
    is_typing_tx: mpsc::Sender<TypingEventMsg>,
}

#[async_trait::async_trait]
impl Handler for JsonHandler {
    async fn handle_segmented_frame(&mut self, buffer: &[u8]) {
        if self.is_message_first {
            self.builder = MessageBuilder::parse(buffer);
            if let Some(builder) = &self.builder {
                if builder.size() == 0 {
                    self.send_error("none", "Message size cannot be 0!".into())
                        .await;
                    self.builder = None;
                    self.is_message_first = true;
                    return;
                }

                // if message size exceeds the maximum size do not handle it.
                if builder.size() > MAX_JSON_MSG_SIZE {
                    self.send_error(
                        "none",
                        format!("Message size cannot be {}!", builder.size()).into(),
                    )
                    .await;
                    self.builder = None;
                    self.is_message_first = true;
                    return;
                }

                #[cfg(debug_assertions)]
                if builder.size() < 1024 {
                    debug!(
                        "Got the message! \n Message size: {} \n Message Content: {}",
                        builder.size(),
                        String::from_utf8_lossy(builder.content_bytes())
                    );
                } else {
                    debug!("Got the message! \n Message size: {}", builder.size());
                }
            }

            if let Some(ref message) = self.builder {
                if !message.ready() {
                    self.is_message_first = false;
                    return;
                }
            }
        }

        let mut do_handle = false;
        if let Some(ref mut message) = self.builder {
            if !message.ready() && !self.is_message_first {
                message.extend(buffer);
            }

            if message.ready() {
                do_handle = true;
            }
        }

        self.is_message_first = false;

        if do_handle {
            if self.builder.is_some() {
                self.try_handle_json_message().await;
            }

            if let Some(ref mut message) = self.builder {
                message.clear();
            }
            self.builder = None;
            self.is_message_first = true;
        }
    }
}

impl JsonHandler {
    async fn typing_recv_task(sessions: Sessions, mut rx: mpsc::Receiver<TypingEventMsg>) {
        let mut last_chat_id: i32;

        let delay_fut = tokio::time::sleep(IS_TYPING_SLEEP_DURATION);
        tokio::pin!(delay_fut);

        'receiver_loop: while let Some((mut msg, mut users)) = rx.recv().await {
            last_chat_id = msg.chat_id;

            delay_fut
                .as_mut()
                .reset(tokio::time::Instant::now() + IS_TYPING_SLEEP_DURATION);

            for user in users.iter() {
                if let Some(receiver_session) = sessions.get(&user.as_i32_unchecked()) {
                    let mut target_connection = receiver_session.write().await;

                    target_connection.mpsc_send(msg.clone(), 0).await;
                }
            }

            // If new message is received before the Sleeper finishes, reset sleeper
            'inner_loop: loop {
                tokio::select! {
                    _ = &mut delay_fut => {
                        msg.is_typing = false;

                        for user in users.iter() {
                            if let Some(receiver_session) = sessions.get(&user.as_i32_unchecked()) {
                                let mut target_connection = receiver_session.write().await;

                                target_connection.mpsc_send(msg.clone(), 0).await;
                            }
                        }
                        break 'inner_loop;
                    },
                    // reset timer
                    Some((new_msg, new_users)) = rx.recv() => {
                        msg = new_msg;
                        users = new_users;

                        if msg.chat_id != last_chat_id || !msg.is_typing {
                            msg.is_typing = false;

                            for user in users.iter() {
                                if let Some(receiver_session) = sessions.get(&user.as_i32_unchecked()) {
                                    let mut target_connection = receiver_session.write().await;

                                    target_connection.mpsc_send(msg.clone(), 0).await;
                                }
                            }
                            break 'inner_loop;
                        } else {
                            delay_fut
                                .as_mut()
                                .reset(tokio::time::Instant::now() + IS_TYPING_SLEEP_DURATION);
                        }
                    },
                    else => {
                        break 'receiver_loop;
                    }
                }
            }
        }
    }

    pub async fn new(
        session: Arc<RwLock<Session>>,
        sessions: Sessions,
        bucket: DatabaseBucket,
    ) -> Self {
        let output_connection = {
            let session_locked = session.read().await;
            // Assume the last connection is the output connection
            // TODO: User must decide on which connection he wants the output
            Arc::clone(session_locked.connections().first().unwrap())
        };

        let (tx, rx) = mpsc::channel::<TypingEventMsg>(3);
        tokio::spawn({
            let sessions = Arc::clone(&sessions);
            async move {
                Self::typing_recv_task(sessions, rx).await;
            }
        });

        JsonHandler {
            builder: None,
            session: Arc::clone(&session),
            sessions,
            output_connection,
            is_message_first: true,
            bucket,
            is_typing_tx: tx,
        }
    }

    /// Later, we need to retreive the content of the `self.builder`
    pub fn utf8_content_unchecked(&mut self) -> &String {
        self.builder.as_mut().unwrap().content_utf8().unwrap()
    }

    pub async fn send_is_typing(&self, event: IsTypingEvent, users: Vec<UserId>) {
        let res = self.is_typing_tx.send((event, users)).await;

        if cfg!(debug_assertions) {
            res.expect("to be able to send to the queue");
        }
    }

    /// Sends standartized json error:
    ///
    /// ok: false,
    /// method as `str`,
    /// err as `str`
    pub async fn send_error(&self, method: &str, err: PPError) {
        err.safe_send(method, &self.output_connection).await;
    }

    /// Sending message with tokio::spawn.
    /// Necessary for large objects, so the read operations won't be stopped
    pub fn send_raw_detached(&self, data: Arc<[u8]>) {
        tokio::spawn({
            let connection = Arc::clone(&self.output_connection);
            let data = Arc::clone(&data);
            info!("Sending media back: {}", data.len());
            async move {
                connection
                    .write(&MessageBuilder::build_from_slice(&data).packed())
                    .await
            }
        });
    }

    /// Instead of json, sends raw buffer directly to the connection
    pub async fn send_raw(&self, data: &[u8]) {
        self.output_connection
            .write(&MessageBuilder::build_from_slice(data).packed())
            .await;
    }

    pub fn reader(&self) -> Arc<Mutex<OwnedReadHalf>> {
        Arc::clone(&self.output_connection.reader())
    }

    pub async fn send_message<T: ?Sized + Serialize>(&self, message: &T) {
        self.output_connection
            .write(
                &MessageBuilder::build_from_str(serde_json::to_string(&message).unwrap()).packed(),
            )
            .await;
    }

    /// Sends message to other user, meaning connection(e.g. new chat, new message, or any other event that must be handled in realtime)
    ///
    /// If user isn't connected to the server, nothing happens
    /// WARN!! DO NOT USE IN FOR LOOP!
    pub fn send_event_to_con_detached(
        &self,
        to: i32,
        msg: impl Serialize + std::fmt::Debug + Send + 'static,
    ) {
        tokio::spawn({
            let connections = Arc::clone(&self.sessions);
            async move {
                if let Some(receiver_session) = connections.get(&to) {
                    let mut target_connection = receiver_session.write().await;

                    target_connection.mpsc_send(msg, 0).await;
                }
            }
        });
    }

    /// same as `send_event_to_con_detached`, but multiple
    pub fn send_events_to_connections<I, M>(&self, recv_msgs: I)
    where
        I: IntoIterator<Item = (i32, M)> + Send + 'static,
        M: Serialize + std::fmt::Debug + Send + 'static,
        <I as std::iter::IntoIterator>::IntoIter: std::marker::Send,
    {
        tokio::spawn({
            let connections = Arc::clone(&self.sessions);
            async move {
                for (to, msg) in recv_msgs {
                    if let Some(receiver_session) = connections.get(&to) {
                        let mut target_connection = receiver_session.write().await;

                        target_connection.mpsc_send(msg, 0).await;
                    }
                }
            }
        });
    }

    // Function to get any database by just passing the type
    pub fn get_db<T: From<DatabaseBuilder>>(&self) -> T {
        DatabaseBuilder::from(self.bucket.clone()).into()
    }

    async fn try_handle_json_message(&mut self) {
        let message = self.builder.as_mut().unwrap().content_utf8();
        if message.is_none() {
            self.send_error("none", "Invalid utf8 sequence!".into())
                .await;
            return;
        }
        let message = message.unwrap();

        match serde_json::from_str::<Value>(message) {
            Ok(value) => match value.get("method").and_then(Value::as_str) {
                Some(method) => match method {
                    "login" | "auth" | "register" => auth::handle(self, method).await,
                    "send_message" => send::handle(self, method).await,
                    "edit" | "delete" => edit::handle(self, method).await,
                    "fetch" => fetch::handle(self, method).await,
                    "check" => check::handle(self, method).await,
                    "bind" => bind::handle(self, method).await,
                    "new" => new::handle(self, method).await,
                    "join" => join::handle(self, method).await,
                    _ => {
                        self.send_error(method, "Unknown method given!".into())
                            .await
                    }
                },
                None => {
                    self.send_error(
                        "none",
                        "Failed to get the method from the json message!".into(),
                    )
                    .await
                }
            },
            Err(err) => {
                self.send_error("none", err.to_string().into()).await;
            }
        }
    }
}

impl Drop for JsonHandler {
    fn drop(&mut self) {
        // Basically drops the reference count
        self.bucket.decrement_rc();

        tokio::spawn({
            let connections: Sessions = Arc::clone(&self.sessions);
            let session = Arc::clone(&self.session);
            let connection = Arc::clone(&self.output_connection);

            async move {
                // Try to find this connection in a global hashmap, delete if authenticated
                let mut session = session.write().await;

                session.remove_connection(connection);
                if let Some((user_id, _)) = session.get_credentials() {
                    if session.connections().is_empty() {
                        connections.remove(&user_id.as_i32().unwrap());
                    }
                }
            }
        });
    }
}
