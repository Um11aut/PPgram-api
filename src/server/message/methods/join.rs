use crate::{db::{chat::chats::ChatsDB, internal::error::PPResult, user::UsersDB}, server::message::{handlers::tcp_handler::TCPHandler, types::{request::join::JoinGroupRequest, response::{events::NewParticipantEvent, join::{JoinGroupResponse, JoinLinkNotFoundResponse}}, user::UserId}}};

async fn on_join_group(msg: JoinGroupRequest, handler: &mut TCPHandler) -> PPResult<()> {
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
            users_db.add_chat(&self_user_id, chat.chat_id(), chat.chat_id()).await?;
            chats_db.add_participant(chat.chat_id(), &self_user_id).await?;
            // Send event that new participant joined
            let self_info = users_db.fetch_user(&self_user_id).await?.unwrap();

            for other in chat.participants() {
                let self_info = self_info.clone();
                let chat_id = chat.chat_id().clone();

                handler.send_msg_to_connection_detached(other.user_id(), NewParticipantEvent{
                    event: "new_participant".into(),
                    chat_id,
                    new_user: self_info
                });
            }

            handler.send_message(&JoinGroupResponse{
                ok: true,
                method: "join_group".into(),
                chat: chat.group_details_unchecked()
            }).await;
        }
        None => {
            handler.send_message(&JoinLinkNotFoundResponse{
                ok: true,
                method: "join_invitation_link".into(),
                code: 404
            }).await;
        }
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

    
}