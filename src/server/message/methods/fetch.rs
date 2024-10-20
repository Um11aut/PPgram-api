use serde::Serialize;
use serde_json::{json, Value};

use crate::db::chat::chats::ChatsDB;
use crate::db::chat::messages::MessagesDB;
use crate::db::internal::error::{PPError, PPResult};
use crate::db::user::UsersDB;
use crate::fs::media::get_media;
use crate::server::message::methods::auth_macros;
use crate::server::message::types::chat::ChatDetails;
use crate::server::message::types::message::Message;
use crate::server::message::types::request::{extract_what_field, fetch::*};
use crate::server::message::types::response::fetch::{FetchChatsResponse, FetchMessagesResponse, FetchSelfResponseMessage, FetchUserResponse, FetchUsersResponse};
use crate::server::message::{
        handlers::tcp_handler::TCPHandler,
        types::user::{User, UserId},
    };

async fn handle_fetch_chats(handler: &TCPHandler) -> PPResult<Vec<ChatDetails>> {
    let users_db: UsersDB = handler.get_db();
    let chats_db: ChatsDB = handler.get_db();

    let (user_id, _) = handler.session.read().await.get_credentials_unchecked();
    let chat_ids = users_db.fetch_chats(&user_id).await?;
    let mut chats_details: Vec<ChatDetails> = vec![];
    for (chat_id, associated_chat_id) in chat_ids {
        let chat = chats_db.fetch_chat(associated_chat_id).await.unwrap();
        if let Some(chat) = chat {
            let details = chat.details(&user_id).await?;
            if let Some(mut details) = details {
                if !chat.is_group() {
                    // Fake the chat id by the user id
                    details.chat_id = chat_id;
                }
                chats_details.push(details);
            }
        }
    }
    Ok(chats_details)
}

/// Fetches Users by the given search query
async fn on_users(handler: &mut TCPHandler) -> PPResult<FetchUsersResponse> {
    let content = handler.utf8_content_unchecked();
    let msg = serde_json::from_str::<FetchUsersRequest>(&content)?;
    let query = msg.query;

    let users_db: UsersDB = handler.get_db();
    let search_result = users_db.fetch_users_by_search_query(query).await?;

    Ok(FetchUsersResponse{
        ok: true,
        method: "fetch_users".into(),
        users: search_result
    })
}

async fn fetch_user(
    identifier: &UserId,
    db: UsersDB
) -> PPResult<User> {
    match db.fetch_user(identifier).await {
        Ok(details) => return details.ok_or("User wasn't found!".into()),
        Err(err) => {Err(err)}
    }
}

async fn handle_fetch_self(handler: &TCPHandler) -> PPResult<User> {
    // Can unwrap because we have checked the creds earlier
    let (user_id, _) = handler.session.read().await.get_credentials_unchecked();
    fetch_user(&user_id, handler.get_db()).await
}

async fn handle_fetch_messages(handler: &TCPHandler, msg: FetchMessagesRequest) -> PPResult<Vec<Message>> {
    // Groups have negative id
    let maybe_chat_id = if msg.chat_id.is_positive() {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        handler.get_db::<UsersDB>().get_associated_chat_id(&user_id, msg.chat_id).await
    } else {match handler.get_db::<ChatsDB>().chat_exists(msg.chat_id).await {
        Ok(res) => if res {Ok(Some(msg.chat_id))} else {Err("No group found by the given chat id!".into())},
        Err(err) => Err(err)
    }}?;

    match maybe_chat_id {
        Some(target_chat_id) => {
            let mut msgs = handler.get_db::<MessagesDB>().fetch_messages(target_chat_id, msg.range[0]..msg.range[1]).await?;

            msgs.iter_mut().for_each(|message| message.chat_id = msg.chat_id);
            Ok(msgs)
        }
        None => Err("failed to retrieve chat_id!".into())
    }
}

async fn on_chats(handler: &TCPHandler) -> PPResult<FetchChatsResponse> {
    let chats = handle_fetch_chats(&handler).await?;
    Ok(FetchChatsResponse {
        ok: true,
        method: "fetch_chats".into(),
        chats
    })
}

async fn on_self(handler: &TCPHandler) -> PPResult<FetchSelfResponseMessage> {
    let self_info = handle_fetch_self(&handler).await?;
    Ok(FetchSelfResponseMessage {
        ok: true,
        method: "fetch_self".into(),
        name: self_info.name().into(),
        user_id: self_info.user_id(),
        username: self_info.username().into(),
        photo: self_info.photo_moved()
    })
}

async fn on_user(handler: &mut TCPHandler) -> PPResult<FetchUserResponse> {
    let content = handler.utf8_content_unchecked();
    let msg: FetchUserRequest = serde_json::from_str(&content)?;
    
    let user_id: UserId = match msg.username {
        Some(username) => username.as_str().into(),
        None => match msg.user_id{
            Some(user_id) => user_id.into(),
            None => return Err("Neither 'user_id' nor 'username' were correctly provided".into())
        }
    };

    let self_info = fetch_user(&user_id, handler.get_db()).await?;
    Ok(FetchUserResponse {
        ok: true,
        method: "fetch_user".into(),
        name: self_info.name().into(),
        user_id: self_info.user_id(),
        username: self_info.username().into(),
        photo: self_info.photo_moved()
    })
}

async fn on_messages(handler: &mut TCPHandler) -> PPResult<FetchMessagesResponse> {
    let content = handler.utf8_content_unchecked();
    let msg = serde_json::from_str::<FetchMessagesRequest>(&content)?;
    let fetched_msgs = handle_fetch_messages(&handler, msg).await?;

    Ok(FetchMessagesResponse{
        ok: true,
        method: "fetch_messages".into(),
        messages: fetched_msgs
    })
} 

/// Directly sends raw media
async fn on_media(handler: &mut TCPHandler) -> PPResult<()> {
    let content = handler.utf8_content_unchecked();
    let msg = serde_json::from_str::<FetchMediaRequest>(&content)?;

    let maybe_media = get_media(&msg.media_hash).await?;
    handler.send_raw(&maybe_media).await;

    Ok(())
} 

/// Needs to be wrapped in option because media directly sends the message avoiding json for the performance purpose
async fn handle_json_message(handler: &mut TCPHandler) -> PPResult<Option<Value>> {
    let content = handler.utf8_content_unchecked();
    let what = extract_what_field(&content)?;

    match what.as_str() {
        "chats" => match on_chats(&handler).await.map(|v| serde_json::to_value(v).unwrap()) {
            Ok(v) => Ok(Some(v)),
            Err(err) => Err(err)
        },
        "self" => match on_self(&handler).await.map(|v| serde_json::to_value(v).unwrap()){
            Ok(v) => Ok(Some(v)),
            Err(err) => Err(err)
        },
        "user" => match on_user(handler).await.map(|v| serde_json::to_value(v).unwrap()) {
            Ok(v) => Ok(Some(v)),
            Err(err) => Err(err)
        }
        "messages" => match on_messages(handler).await.map(|v| serde_json::to_value(v).unwrap()) {
            Ok(v) => Ok(Some(v)),
            Err(err) => Err(err)
        }
        "media" => match on_media(handler).await {
            Ok(()) => Ok(None),
            Err(err) => Err(err)
        }
        "users" => match on_users(handler).await.map(|v| serde_json::to_value(v).unwrap()) {
            Ok(v) => Ok(Some(v)),
            Err(err) => Err(err)
        }
        _ => return Err(PPError::from("Unknown 'what' field provided!"))
    }
}

pub async fn handle(handler: &mut TCPHandler, method: &str) {
    auth_macros::require_auth!(handler, method);

    match handle_json_message(handler).await {
        Ok(message) => if let Some(msg) = message {handler.send_message(&msg).await},
        Err(err) => {handler.send_error("fetch", err).await;}
    };
}
