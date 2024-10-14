use log::debug;
use serde_json::json;
use tokio::sync::RwLock;

use crate::{
    db::{
        chat::{
            chats::{ChatsDB, InvitationHash},
            messages::MessagesDB,
        },
        internal::error::{PPError, PPResult},
        user::UsersDB,
    },
    server::{
        message::{
            handlers::tcp_handler::TCPHandler,
            types::{
                chat::{Chat, ChatDetails, ChatId},
                request::{
                    extract_what_field,
                    message::{MessageId, MessageRequest},
                    new::{NewGroupRequest, NewInvitationLinkRequest},
                },
                response::{
                    events::{NewChatEvent, NewMessageEvent},
                    new::{NewGroupResponse, NewInvitationLinkResponse},
                    send::SendMessageResponse,
                },
                user::UserId,
            },
        },
        session::Session,
    },
};
use std::sync::Arc;

/// Returns latest chat message id if sucessful
async fn handle_new_group(msg: NewGroupRequest, handler: &TCPHandler) -> PPResult<Chat> {
    let self_user_id: UserId = {
        handler
            .session
            .read()
            .await
            .get_credentials_unchecked()
            .0
            .to_owned()
    };
    let group = handler
        .get_db::<ChatsDB>()
        .create_group(
            vec![self_user_id],
            ChatDetails {
                name: msg.name,
                chat_id: Default::default(),
                is_group: true,
                username: msg.username,
                photo: msg.avatar_hash,
            },
        )
        .await?;

    Ok(group)
}

async fn handle_new_invitation_link(
    msg: NewInvitationLinkRequest,
    handler: &TCPHandler,
) -> PPResult<InvitationHash> {
    let self_user_id: UserId = {
        handler
            .session
            .read()
            .await
            .get_credentials_unchecked()
            .0
            .to_owned()
    };

    let users_db: UsersDB = handler.get_db();
    if msg.chat_id
        != users_db
            .get_associated_chat_id(&self_user_id, &msg.chat_id.into())
            .await?
            .ok_or(PPError::from("No group with the given chat_id was found!"))?
    {
        return Err("Internal and public chat_ids must mach".into());
    }

    let db = handler.get_db::<ChatsDB>();
    if msg.chat_id.is_positive() {
        return Err("The id of provided chat must be a group!".into());
    }
    if !db.chat_exists(msg.chat_id).await? {
        return Err("Provided group doesn't exist!".into());
    }

    db.create_invitation_hash(msg.chat_id).await
}

async fn on_new(handler: &mut TCPHandler) -> PPResult<()> {
    let content = handler.utf8_content_unchecked();
    let what_field = extract_what_field(content)?;

    match what_field.as_str() {
        "group" => match serde_json::from_str::<NewGroupRequest>(&content) {
            Ok(msg) => {
                let chat = handle_new_group(msg, handler).await?;
                handler
                    .send_message(&NewGroupResponse {
                        ok: true,
                        method: "new_group".into(),
                        chat: chat.group_details_unchecked(),
                    })
                    .await;
            }
            Err(err) => return Err(err.into()),
        },
        "invitation_link" => match serde_json::from_str::<NewInvitationLinkRequest>(&content) {
            Ok(msg) => {
                let link = handle_new_invitation_link(msg, handler).await?;
                handler.send_message(&NewInvitationLinkResponse{
                    ok: true,
                    method: "new_invitation_link".into(),
                    link
                }).await;
            }
            Err(err) => return Err(err.into()),
        },
        _ => return Err("Unknown what field!".into()),
    }

    Ok(())
}

pub async fn handle(handler: &mut TCPHandler, method: &str) {
    {
        let session = handler.session.read().await;
        if !session.is_authenticated() {
            handler
                .send_error(method, "You aren't authenticated!".into())
                .await;
            return;
        }
    }

    if let Err(err) = on_new(handler).await {
        handler.send_error(method, err).await
    }
}
