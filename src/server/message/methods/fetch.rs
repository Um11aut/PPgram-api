use serde::Serialize;
use serde_json::{json, Value};

use crate::db::chat::messages::MESSAGES_DB;
use crate::db::internal::error::{PPError, PPResult};
use crate::fs::media::get_media;
use crate::server::message::types::chat::ChatDetails;
use crate::server::message::types::request::fetch::*;
use crate::server::message::types::request::message::DbMesssage;
use crate::server::message::types::response::fetch::{FetchChatsResponseMessage, FetchMessagesResponseValue, FetchSelfResponseMessage, FetchUserResponseMessage};
use crate::{
    db::{
        chat::chats::CHATS_DB,
        user::USERS_DB,
    },
    server::message::{
        handler::MessageHandler,
        types::user::{User, UserId},
    },
};

async fn handle_fetch_chats(handler: &MessageHandler) -> PPResult<Vec<ChatDetails>> {
    let users_db = USERS_DB.get().unwrap();
    let chats_db = CHATS_DB.get().unwrap();

    let (user_id, _) = handler.session.read().await.get_credentials().unwrap();
    let chat_ids = users_db.fetch_chats(&user_id).await?;
    let mut chats_details: Vec<ChatDetails> = vec![];
    for (chat_id, associated_chat_id) in chat_ids {
        let chat = chats_db.fetch_chat(associated_chat_id).await.unwrap();
        if let Some(chat) = chat {
            let details = chat.details(&user_id).await?;
            if let Some(mut details) = details {
                details.chat_id = chat_id;
                chats_details.push(details);
            }
        }
    }
    Ok(chats_details)
}

async fn fetch_user(
    identifier: &UserId,
) -> PPResult<User> {
    match USERS_DB.get().unwrap().fetch_user(identifier).await {
        Ok(details) => return details.ok_or("User wasn't found!".into()),
        Err(err) => {Err(err)}
    }
}

async fn handle_fetch_self(handler: &MessageHandler) -> PPResult<User> {
    // Can unwrap because we have checked the creds earlier
    let (user_id, _) = handler.session.read().await.get_credentials().unwrap();
    fetch_user(&user_id).await
}

async fn handle_fetch_messages(handler: &MessageHandler, msg: FetchMessagesRequestMessage) -> PPResult<Vec<DbMesssage>> {
    let maybe_chat_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials().unwrap();
        USERS_DB.get().unwrap().get_associated_chat_id(&user_id, &msg.chat_id.into()).await
    }?;

    match maybe_chat_id {
        Some(target_chat_id) => {
            let mut msgs = MESSAGES_DB.get().unwrap().fetch_messages(target_chat_id, msg.range[0]..msg.range[1]).await?.unwrap_or(vec![]);

            msgs.iter_mut().for_each(|message| message.chat_id = msg.chat_id);
            Ok(msgs)
        }
        None => Err("failed to retrieve chat_id!".into())
    }
}

fn extract_what_field(message: &String) -> PPResult<String> {
    let what_field = serde_json::from_str::<BaseFetchRequestMessage>(&message)?;

    Ok(what_field.what)
}

async fn on_chats(handler: &MessageHandler) -> PPResult<FetchChatsResponseMessage> {
    let chats = handle_fetch_chats(&handler).await?;
    Ok(FetchChatsResponseMessage {
        ok: true,
        method: "fetch_chats".into(),
        chats
    })
}

async fn on_self(handler: &MessageHandler) -> PPResult<FetchSelfResponseMessage> {
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

async fn on_user(content: &String) -> PPResult<FetchUserResponseMessage> {
    let msg: FetchUserRequestMessage = serde_json::from_str(&content)?;
    
    let user_id: UserId = match msg.username {
        Some(username) => username.as_str().into(),
        None => match msg.user_id{
            Some(user_id) => user_id.into(),
            None => return Err("Neither 'user_id' nor 'username' were correctly provided".into())
        }
    };

    let self_info = fetch_user(&user_id).await?;
    Ok(FetchUserResponseMessage {
        ok: true,
        method: "fetch_user".into(),
        name: self_info.name().into(),
        user_id: self_info.user_id(),
        username: self_info.username().into(),
        photo: self_info.photo_moved()
    })
}

async fn on_messages(handler: &mut MessageHandler) -> PPResult<FetchMessagesResponseValue> {
    let content = handler.utf8_content_unchecked();
    let msg = serde_json::from_str::<FetchMessagesRequestMessage>(&content)?;
    let fetched_msgs = handle_fetch_messages(&handler, msg).await?;

    Ok(FetchMessagesResponseValue{
        ok: true,
        method: "fetch_messages".into(),
        messages: fetched_msgs
    })
} 

/// Directly sends raw media
async fn on_media(handler: &mut MessageHandler) -> PPResult<()> {
    let content = handler.utf8_content_unchecked();
    let msg = serde_json::from_str::<FetchMediaRequestMessage>(&content)?;

    let maybe_media = get_media(&msg.media_hash).await?;
    handler.send_raw(&maybe_media).await;

    Ok(())
} 

/// Needs to be wrapped in option because media directly sends the message avoiding json for the performance purpose
async fn handle_json_message(handler: &mut MessageHandler) -> PPResult<Option<Value>> {
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
        "user" => match on_user(&content).await.map(|v| serde_json::to_value(v).unwrap()) {
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
        _ => return Err(PPError::from("Unknown 'what' field provided!"))
    }
}

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    {
        let session = handler.session.read().await;
        if !session.is_authenticated() {
            handler.send_error(method, "You aren't authenticated!".into()).await;
            return;
        }
    }

    match handle_json_message(handler).await {
        Ok(message) => if let Some(msg) = message {handler.send_message(&msg).await},
        Err(err) => {handler.send_error("fetch", err).await;}
    };
}
