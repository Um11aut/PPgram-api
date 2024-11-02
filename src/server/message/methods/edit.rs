use log::debug;
use serde_json::Value;

use crate::{db::{chat::messages::MessagesDB, internal::error::PPResult, user::UsersDB}, server::message::{
            handlers::json_handler::TCPHandler, types::{
                edit::EditedMessageBuilder, request::{
                    delete::DeleteMessageRequest, edit::{EditMessageRequest, EditSelfRequest}, extract_what_field
                }, response::{delete::DeleteMessageResponse, edit::EditMessageResponse, events::{DeleteMessageEvent, EditMessageEvent}}
            }
        }};

use super::auth_macros;

async fn handle_edit_message(handler: &mut TCPHandler, msg: EditMessageRequest) -> PPResult<()> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };

    let users_db: UsersDB = handler.get_db();
    let messages_db: MessagesDB = handler.get_db();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Chat with the given chat_id doesn't exist!")?;
    if messages_db
        .message_exists(real_chat_id, msg.message_id)
        .await?
    {
        let to_user_id = msg.chat_id.clone();
        let msg_id = msg.message_id.clone();

        let builder = EditedMessageBuilder::from(msg);
        let existing_message = messages_db
            .fetch_messages(real_chat_id, msg_id..0)
            .await?
            .remove(0);
        debug!("Existing Message: {:?}", existing_message);

        let edited_msg = builder.get_edited_message(existing_message);
        messages_db
            .edit_message(msg_id, real_chat_id, edited_msg.clone())
            .await?;
        debug!("Edited Message: {:?}", edited_msg);
        handler.send_msg_to_connection_detached(to_user_id, EditMessageEvent{
            event: "edit_message".into(),
            new_message: edited_msg
        });
    } else {
        return Err("Message with the given message_id wasn't found!".into());
    }

    Ok(())
}

// TODO: Add self editing(do not travers all subscribtions)
async fn handle_edit_self(handler: &mut TCPHandler, msg: &EditSelfRequest) -> PPResult<()> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };

    let users_db: UsersDB = handler.get_db();

    todo!()
}

async fn on_edit(handler: &mut TCPHandler, content: &String) -> PPResult<EditMessageResponse> {
    let what_field = extract_what_field(&content)?;

    match what_field.as_str() {
        "message" => {
            let msg: EditMessageRequest = serde_json::from_str(&content)?;
            handle_edit_message(handler, msg).await?;
            Ok(EditMessageResponse {
                ok: true,
                method: "edit_message".into(),
            })
        }
        "self" => {
            let msg: EditSelfRequest = serde_json::from_str(&content)?;
            handle_edit_self(handler, &msg).await?;
            Ok(EditMessageResponse {
                ok: true,
                method: "edit_self".into(),
            })
        }
        _ => Err("Unknown what field! Known what fields for edit: 'message', 'self'".into()),
    }
}

async fn on_delete(handler: &mut TCPHandler, content: &String) -> PPResult<DeleteMessageResponse> {
    let msg: DeleteMessageRequest = serde_json::from_str(&content)?;

    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };
    
    let users_db: UsersDB = handler.get_db();
    let messages_db: MessagesDB = handler.get_db();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Chat with the given chat_id doesn't exist!")?;
    if messages_db
        .message_exists(real_chat_id, msg.message_id)
        .await?
    {
        messages_db.delete_message(real_chat_id, msg.message_id).await?;
        handler.send_msg_to_connection_detached(msg.chat_id, DeleteMessageEvent{
            event: "delete_message".into(),
            chat_id: msg.chat_id,
            message_id: msg.message_id
        });

        Ok(DeleteMessageResponse {
            ok: true,
            method: "delete_message".into()
        })
    } else {
        Err("Message with the given message_id wasn't found!".into())
    }
}

async fn handle_messages(handler: &mut TCPHandler, method: &str) -> PPResult<Value> {
    let content = handler.utf8_content_unchecked().to_owned();
    match method {
        "edit" => Ok(serde_json::to_value(on_edit(handler, &content).await?).unwrap()),
        "delete" => Ok(serde_json::to_value(on_delete(handler, &content).await?).unwrap()),
        _ => Err("Unknown method".into()),
    }
}

pub async fn handle(handler: &mut TCPHandler, method: &str) {
    auth_macros::require_auth!(handler, method);

    match handle_messages(handler, method).await {
        Ok(val) => {
            handler.send_message(&val).await;
        }
        Err(err) => {
            handler.send_error(method, err).await;
        }
    }
}
