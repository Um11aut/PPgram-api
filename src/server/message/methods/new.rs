
use crate::{
    db::{
        chat::{chats::{ChatsDB, InvitationHash}, messages::MessagesDB},
        internal::error::{PPError, PPResult},
        user::UsersDB,
    },
    server::message::{
            handlers::json_handler::JsonHandler, methods::macros, types::{
                chat::{Chat, ChatDetails, ChatDetailsResponse},
                request::{
                    extract_what_field, new::{NewGroupRequest, NewInvitationLinkRequest}
                },
                response::new::{NewGroupResponse, NewInvitationLinkResponse},
                user::UserId,
            }
        },
};

/// Returns latest chat message id if sucessful
async fn handle_new_group(msg: NewGroupRequest, handler: &JsonHandler) -> PPResult<(Chat, ChatDetails)> {
    let self_user_id: UserId = {
        handler
            .session
            .read()
            .await
            .get_credentials_unchecked()
            .0
            .to_owned()
    };

    let chats_db = handler.get_db::<ChatsDB>();
    let group =
        chats_db.create_group(
                &self_user_id,
            vec![self_user_id.clone()],
            ChatDetails {
                name: msg.name,
                chat_id: Default::default(),
                is_group: true,
                tag: msg.username,
                photo: msg.avatar_hash,
            },
        )
        .await?;
    handler.get_db::<UsersDB>().add_chat(&self_user_id, group.0.chat_id(), group.0.chat_id()).await?;

    Ok(group)
}

async fn handle_new_invitation_link(
    msg: NewInvitationLinkRequest,
    handler: &JsonHandler,
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
            .get_associated_chat_id(&self_user_id, msg.chat_id)
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

async fn on_new(handler: &mut JsonHandler) -> PPResult<()> {
    let content = handler.utf8_content_unchecked();
    let what_field = extract_what_field(content)?;

    match what_field.as_str() {
        "group" => match serde_json::from_str::<NewGroupRequest>(&content) {
            Ok(msg) => {
                let (_, chat_details) = handle_new_group(msg, handler).await?;
                handler
                    .send_message(&NewGroupResponse {
                        ok: true,
                        method: "new_group".into(),
                        chat: ChatDetailsResponse{
                            details: chat_details,
                            unread_count: 0
                        },
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

pub async fn handle(handler: &mut JsonHandler, method: &str) {
    macros::require_auth!(handler, method);

    if let Err(err) = on_new(handler).await {
        handler.send_error(method, err).await
    }
}
