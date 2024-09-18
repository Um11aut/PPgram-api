use log::debug;
use serde_json::Value;

use crate::{
    db::{chat::messages::MESSAGES_DB, internal::error::PPResult, user::USERS_DB},
    server::{
        message::{
            handler::MessageHandler,
            methods::edit,
            types::{
                edit::EditedMessageBuilder,
                request::{
                    edit::{DeleteChatMessage, EditChatMessage, EditSelfMessage},
                    extract_what_field,
                },
                response::edit::EditMessageResponse,
            },
        },
        session::Session,
    },
};

async fn handle_edit_message(handler: &mut MessageHandler, msg: EditChatMessage) -> PPResult<()> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials().unwrap();
        user_id
    };

    let users_db = USERS_DB.get().unwrap();
    let messages_db = MESSAGES_DB.get().unwrap();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, &msg.chat_id.into())
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
        handler.send_msg_to_connection(to_user_id, edited_msg);
    } else {
        return Err("Message with the given message_id wasn't found!".into());
    }

    Ok(())
}

// TODO: Add self editing(do not travers all subscribtions)
async fn handle_edit_self(handler: &mut MessageHandler, msg: &EditSelfMessage) -> PPResult<()> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials().unwrap();
        user_id
    };

    let users_db = USERS_DB.get().unwrap();

    todo!()
}

async fn on_edit(handler: &mut MessageHandler, content: &String) -> PPResult<EditMessageResponse> {
    let what_field = extract_what_field(&content)?;

    match what_field.as_str() {
        "message" => {
            let msg: EditChatMessage = serde_json::from_str(&content)?;
            handle_edit_message(handler, msg).await?;
            Ok(EditMessageResponse {
                ok: true,
                method: "edit_message".into(),
            })
        }
        "self" => {
            let msg: EditSelfMessage = serde_json::from_str(&content)?;
            handle_edit_self(handler, &msg).await?;
            Ok(EditMessageResponse {
                ok: true,
                method: "edit_self".into(),
            })
        }
        _ => Err("Unknown what field! Known what fields for edit: 'message', 'self'".into()),
    }
}

async fn on_delete(handler: &mut MessageHandler, content: &String) -> PPResult<DeleteChatMessage> {
    todo!()
}

async fn handle_messages(handler: &mut MessageHandler, method: &str) -> PPResult<Value> {
    let content = handler.utf8_content_unchecked().to_owned();
    match method {
        "edit" => Ok(serde_json::to_value(on_edit(handler, &content).await?).unwrap()),
        "delete" => Ok(serde_json::to_value(on_delete(handler, &content).await?).unwrap()),
        _ => Err("Unknown method".into()),
    }
}

pub async fn handle(handler: &mut MessageHandler, method: &str) {
    {
        let session = handler.session.read().await;
        if !session.is_authenticated() {
            handler
                .send_error(method, "You aren't authenticated!".into())
                .await;
            return;
        }
    }

    match handle_messages(handler, method).await {
        Ok(val) => {
            handler.send_message(&val).await;
        }
        Err(err) => {
            handler.send_error(method, err).await;
        }
    }
}
