use crate::{db::{chat::chats::ChatsDB, internal::error::{PPError, PPResult}, user::UsersDB}, server::message::{handlers::json_handler::TCPHandler, methods::auth_macros, types::{request::join::JoinGroupRequest, response::{events::NewParticipantEvent, join::{JoinGroupResponse, JoinLinkNotFoundResponse}}, user::UserId}}};

enum JoinGroupResult {
    JoinGroupResponse(JoinGroupResponse),
    JoinLinkNotFoundResponse(JoinLinkNotFoundResponse)
}

async fn on_join_group(msg: JoinGroupRequest, handler: &mut TCPHandler) -> PPResult<JoinGroupResult> {
    let self_user_id: UserId = {
        handler
            .session
            .read()
            .await
            .get_credentials_unchecked()
            .0
            .to_owned()
    };

    if !msg.link.starts_with("+") {return Err("Invitation link must start with '+'".into())}
    
    let chats_db: ChatsDB = handler.get_db();
    let users_db: UsersDB = handler.get_db();
    let chat = chats_db.get_chat_by_invitation_hash(msg.link).await?;

    match chat {
        Some(chat) => {
            if users_db.get_associated_chat_id(&self_user_id, chat.chat_id()).await?.is_some() {
                return Err(PPError::from("You have already joined this chat!"))
            }
            users_db.add_chat(&self_user_id, chat.chat_id(), chat.chat_id()).await?;
            chats_db.add_participant(chat.chat_id(), &self_user_id).await?;
            let self_info = users_db.fetch_user(&self_user_id).await?.unwrap();
            
            // Send event to every user in the chat 
            // that new participant joined
            for other in chat.participants() {
                let self_info = self_info.clone();
                let chat_id = chat.chat_id().clone();

                handler.send_msg_to_connection_detached(other.user_id(), NewParticipantEvent{
                    event: "new_participant".into(),
                    chat_id,
                    new_user: self_info
                });
            }

            Ok(JoinGroupResult::JoinGroupResponse(JoinGroupResponse{
                ok: true,
                method: "join_group".into(),
                chat: chat.group_details_unchecked()
            }))
        }
        None => {
            Ok(JoinGroupResult::JoinLinkNotFoundResponse(JoinLinkNotFoundResponse{
                ok: true,
                method: "join_invitation_link".into(),
                code: 404
            }))
        }
    }
}

async fn on_join(handler: &mut TCPHandler) -> PPResult<JoinGroupResult> {
    match serde_json::from_str::<JoinGroupRequest>(&handler.utf8_content_unchecked()) {
        Ok(msg) => {
            Ok(on_join_group(msg, handler).await?)
        },
        Err(err) => Err(PPError::from(err))
    }
}

pub async fn handle(handler: &mut TCPHandler, method: &str) {
    auth_macros::require_auth!(handler, method);

    match on_join(handler).await {
        Ok(msg) => match msg {
            JoinGroupResult::JoinGroupResponse(msg) => handler.send_message(&msg).await,
            JoinGroupResult::JoinLinkNotFoundResponse(msg) => handler.send_message(&msg).await,
        },
        Err(err) => {handler.send_error(method, err.into()).await}
    }
}