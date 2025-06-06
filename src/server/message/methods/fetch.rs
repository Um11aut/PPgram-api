use serde_json::Value;

use crate::db::chat::chats::ChatsDB;
use crate::db::chat::drafts::DraftsDB;
use crate::db::chat::hashes::HashesDB;
use crate::db::chat::messages::MessagesDB;
use crate::db::internal::error::{PPError, PPResult};
use crate::db::user::UsersDB;
use crate::fs::media::MediaType;
use crate::server::message::methods::macros;
use crate::server::message::types::chat::ChatDetailsResponse;
use crate::server::message::types::message::Message;
use crate::server::message::types::request::{extract_what_field, fetch::*};
use crate::server::message::types::response::fetch::{
    FetchChatInfoResponse, FetchChatsResponse, FetchMessagesResponse, FetchSelfResponse,
    FetchUserResponse, FetchUsersResponse,
};
use crate::server::message::{
    handlers::json_handler::JsonHandler,
    types::user::{User, UserId},
};

async fn handle_fetch_chats(handler: &JsonHandler) -> PPResult<Vec<ChatDetailsResponse>> {
    let self_user_id = {
        let session = handler.session.read().await;
        session.get_credentials_unchecked().0.to_owned()
    };

    let users_db: UsersDB = handler.get_db();
    let chats_db: ChatsDB = handler.get_db();
    let messages_db: MessagesDB = handler.get_db();
    let drafts_db: DraftsDB = handler.get_db();

    let chat_ids = users_db.fetch_chats(&self_user_id).await?;

    let mut chats_details: Vec<ChatDetailsResponse> = vec![];
    for (chat_id, associated_chat_id) in chat_ids {
        let chat = chats_db
            .fetch_chat(&self_user_id, associated_chat_id)
            .await
            .unwrap();

        if let Some((chat, mut details)) = chat {
            if !chat.is_group() {
                // Fake the chat id with the user id
                details.chat_id = chat_id;
            }
            chats_details.push(ChatDetailsResponse {
                details,
                unread_count: messages_db
                    .fetch_unread_count(associated_chat_id)
                    .await?
                    .ok_or(PPError::from("Failed to fetch chat!"))?,
                draft: drafts_db
                    .fetch_draft(&self_user_id, associated_chat_id)
                    .await?
                    .unwrap_or("".into()),
            });
        }
    }
    Ok(chats_details)
}

/// Fetches Users by the given search query
async fn on_users(handler: &mut JsonHandler) -> PPResult<FetchUsersResponse> {
    let self_user_id = {
        let session = handler.session.read().await;
        session.get_credentials_unchecked().0.as_i32_unchecked()
    };

    let content = handler.utf8_content_unchecked();
    let msg = serde_json::from_str::<FetchUsersRequest>(content)?;
    let query = msg.query;

    let users_db: UsersDB = handler.get_db();
    let mut search_result = users_db.fetch_users_by_search_query(query).await?;
    // filter yourself
    search_result.retain(|x| x.user_id() != self_user_id);

    Ok(FetchUsersResponse {
        ok: true,
        method: "fetch_users".into(),
        users: search_result,
    })
}

async fn fetch_user(identifier: &UserId, db: UsersDB) -> PPResult<User> {
    db.fetch_user(identifier).await?.ok_or("User wasn't found!".into())
}

async fn handle_fetch_self(handler: &JsonHandler) -> PPResult<User> {
    // Can unwrap because we have checked the creds earlier
    let (user_id, _) = handler.session.read().await.get_credentials_unchecked();
    fetch_user(&user_id, handler.get_db()).await
}

async fn handle_fetch_messages(
    handler: &JsonHandler,
    msg: FetchMessagesRequest,
) -> PPResult<Vec<Message>> {
    // Groups have negative id
    let maybe_chat_id = if msg.chat_id.is_positive() {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        handler
            .get_db::<UsersDB>()
            .get_associated_chat_id(&user_id, msg.chat_id)
            .await
    } else if handler.get_db::<ChatsDB>().chat_exists(msg.chat_id).await? {
        Ok(Some(msg.chat_id))
    } else {
        Err("No group found by the given chat id!".into())
    }?;

    match maybe_chat_id {
        Some(target_chat_id) => {
            let mut msgs = handler
                .get_db::<MessagesDB>()
                .fetch_messages(target_chat_id, msg.range[0]..msg.range[1])
                .await?;

            msgs.iter_mut()
                .for_each(|message| message.chat_id = msg.chat_id);
            Ok(msgs)
        }
        None => Err("failed to retrieve chat_id!".into()),
    }
}

async fn on_chats(handler: &JsonHandler) -> PPResult<FetchChatsResponse> {
    let chats = handle_fetch_chats(handler).await?;
    Ok(FetchChatsResponse {
        ok: true,
        method: "fetch_chats".into(),
        chats,
    })
}

async fn on_self(handler: &JsonHandler) -> PPResult<FetchSelfResponse> {
    let self_info = handle_fetch_self(handler).await?;
    Ok(FetchSelfResponse {
        ok: true,
        method: "fetch_self".into(),
        name: self_info.name().into(),
        profile_color: self_info.profile_color(),
        user_id: self_info.user_id(),
        username: self_info.username().into(),
        photo: self_info.photo_moved(),
    })
}

async fn on_user(handler: &mut JsonHandler) -> PPResult<FetchUserResponse> {
    let content = handler.utf8_content_unchecked();
    let msg: FetchUserRequest = serde_json::from_str(content)?;

    let user_id: UserId = match msg.username {
        Some(username) => username.as_str().into(),
        None => match msg.user_id {
            Some(user_id) => user_id.into(),
            None => return Err("Neither 'user_id' nor 'username' were correctly provided".into()),
        },
    };

    let self_info = fetch_user(&user_id, handler.get_db()).await?;
    Ok(FetchUserResponse {
        ok: true,
        method: "fetch_user".into(),
        name: self_info.name().into(),
        profile_color: self_info.profile_color(),
        user_id: self_info.user_id(),
        username: self_info.username().into(),
        photo: self_info.photo_moved(),
    })
}

/// The most expensive function ever
async fn on_chat_info(handler: &mut JsonHandler) -> PPResult<FetchChatInfoResponse> {
    let self_user_id = {
        let session = handler.session.read().await;
        session.get_credentials_unchecked().0.to_owned()
    };

    let content = handler.utf8_content_unchecked();
    let msg: FetchChatInfoRequest = serde_json::from_str(content)?;

    let users_db: UsersDB = handler.get_db();
    let messages_db: MessagesDB = handler.get_db();
    let chats_db: ChatsDB = handler.get_db();
    let hashes_db: HashesDB = handler.get_db();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Provided chat_id wasn't found!")?;
    let sha256_hashes = messages_db
        .fetch_all_hashes(real_chat_id)
        .await?
        .ok_or("No hashes by the given chat_id were found")?;

    let mut photo_count = 0;
    let mut video_count = 0;

    let mut document_count = 0;

    for hash in sha256_hashes {
        let hash_info = hashes_db
            .fetch_hash(&hash)
            .await?
            .ok_or("No way that happened")?;

        if hash_info.is_media {
            let media_extension = hash_info
                .file_path
                .to_str()
                .unwrap()
                .rsplit('.')
                .next()
                .ok_or(PPError::from("Internal error."))?;

            let media_type = MediaType::try_from(media_extension)?;
            match media_type {
                MediaType::Video(_) => {
                    video_count += 1;
                }
                MediaType::Photo(_) => {
                    photo_count += 1;
                }
            }
        } else {
            document_count += 1;
        }
    }

    let (chat, _) = chats_db
        .fetch_chat(&self_user_id, real_chat_id)
        .await?
        .ok_or("Failed to find chat")?;

    Ok(FetchChatInfoResponse {
        ok: true,
        method: "fetch_chat_info".into(),
        photo_count,
        video_count,
        document_count,
        participants: chat.participants().iter().map(|u| u.user_id()).collect(),
    })
}

async fn on_messages(handler: &mut JsonHandler) -> PPResult<FetchMessagesResponse> {
    let content = handler.utf8_content_unchecked();
    let msg = serde_json::from_str::<FetchMessagesRequest>(content)?;
    let fetched_msgs = handle_fetch_messages(handler, msg).await?;

    Ok(FetchMessagesResponse {
        ok: true,
        method: "fetch_messages".into(),
        messages: fetched_msgs,
    })
}

async fn handle_json_message(handler: &mut JsonHandler) -> PPResult<Value> {
    let content = handler.utf8_content_unchecked();
    let what = extract_what_field(content)?;

    match what.as_str() {
        "chats" => on_chats(handler)
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        "self" => on_self(handler)
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        "user" => on_user(handler)
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        "messages" => on_messages(handler)
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        "users" => on_users(handler)
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        "chat_info" => on_chat_info(handler)
            .await
            .map(|v| serde_json::to_value(v).unwrap()),
        _ => Err(PPError::from("Unknown 'what' field provided!")),
    }
}

pub async fn handle(handler: &mut JsonHandler, method: &str) {
    macros::require_auth!(handler, method);

    match handle_json_message(handler).await {
        Ok(message) => handler.send_message(&message).await,
        Err(err) => {
            handler.send_error("fetch", err).await;
        }
    };
}
