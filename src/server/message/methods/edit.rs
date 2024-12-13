use log::debug;
use serde_json::Value;

use crate::{
    db::{
        chat::{chats::ChatsDB, drafts::DraftsDB, messages::MessagesDB},
        internal::error::PPResult,
        user::UsersDB,
    },
    server::message::{
        handlers::json_handler::JsonHandler,
        types::{
            edit::EditedMessageBuilder,
            request::{
                delete::DeleteMessageRequest,
                edit::{EditDraftRequest, EditMessageRequest, EditSelfRequest},
                extract_what_field,
            },
            response::{
                delete::DeleteMessageResponse,
                edit::{EditDraftResponse, EditMessageResponse},
                events::{DeleteMessageEvent, EditMessageEvent, EditSelfEvent},
            },
            user::User,
        },
    },
};

use super::macros;

async fn handle_edit_message(handler: &mut JsonHandler, msg: EditMessageRequest) -> PPResult<()> {
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
        let private_chat_id = msg.chat_id;
        let msg_id = msg.message_id;

        let builder = EditedMessageBuilder::from(msg);
        let existing_message = messages_db
            .fetch_messages(real_chat_id, msg_id..0)
            .await?
            .remove(0);
        if existing_message.from_id != self_user_id.as_i32_unchecked() {
            return Err("You can edit only yours message!".into());
        }
        debug!("Existing Message: {:?}", existing_message);

        let edited_msg = builder.get_edited_message(existing_message);
        messages_db
            .edit_message(msg_id, real_chat_id, edited_msg.clone())
            .await?;
        debug!("Edited Message: {:?}", edited_msg);

        // only negative chat id's are groups
        let is_group = real_chat_id.is_negative();

        if !is_group {
            let mut edited_msg = edited_msg;
            edited_msg.chat_id = self_user_id.as_i32_unchecked();

            handler.send_event_to_con_detached(
                private_chat_id,
                EditMessageEvent {
                    event: "edit_message".into(),
                    new_message: edited_msg,
                },
            );
        } else {
            let chats_db: ChatsDB = handler.get_db();
            let (chat, _) = chats_db.fetch_chat(&self_user_id, real_chat_id).await?.unwrap();

            for participant in chat
                .participants()
                .iter()
                .filter(|el| el.user_id() == self_user_id.as_i32_unchecked())
            {
                handler.send_event_to_con_detached(
                    participant.user_id(),
                    EditMessageEvent {
                        event: "edit_message".into(),
                        new_message: edited_msg.clone(),
                    },
                );
            }
        }
    } else {
        return Err("Message with the given message_id wasn't found!".into());
    }

    Ok(())
}

async fn handle_edit_draft(handler: &mut JsonHandler, msg: &EditDraftRequest) -> PPResult<()> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };

    let drafts_db: DraftsDB = handler.get_db();
    let users_db: UsersDB = handler.get_db();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Chat with the given chat_id doesn't exist!")?;

    drafts_db
        .update_draft(&self_user_id, real_chat_id, msg.draft.as_str())
        .await?;

    Ok(())
}

/// Edits self user profile
async fn handle_edit_self(handler: &mut JsonHandler, msg: &EditSelfRequest) -> PPResult<()> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };

    let users_db: UsersDB = handler.get_db();

    if let Some(name) = msg.name.as_ref() {
        users_db.update_name(&self_user_id, name).await?;
    }

    if let Some(username) = msg.username.as_ref() {
        users_db.update_username(&self_user_id, username).await?;
    }

    if let Some(photo) = msg.photo.as_ref() {
        users_db.update_name(&self_user_id, photo).await?;
    }

    if let Some(password) = msg.password.as_ref() {
        users_db.update_password(&self_user_id, password).await?;
    }

    let chats = users_db.fetch_chats(&self_user_id).await?;
    let self_profile = users_db.fetch_user(&self_user_id).await?.unwrap();

    for pub_chat_id in chats.keys() {
        let is_group = pub_chat_id.is_negative();

        if is_group {
            // if chat is group, we do nothing because of the performance cost of traversing every
            // participant
            continue;
        } else {
            handler.send_event_to_con_detached(
                *pub_chat_id,
                EditSelfEvent {
                    event: "edit_self".into(),
                    new_profile: User::construct(
                        self_profile.name().to_string(),
                        self_profile.user_id(),
                        self_profile.username().to_string(),
                        self_profile.photo_cloned(),
                    ),
                },
            );
        }
    }

    Ok(())
}

async fn on_edit(handler: &mut JsonHandler, content: &String) -> PPResult<serde_json::Value> {
    let what_field = extract_what_field(content)?;

    match what_field.as_str() {
        "message" => {
            let msg: EditMessageRequest = serde_json::from_str(content)?;
            handle_edit_message(handler, msg).await?;
            Ok(serde_json::to_value(EditMessageResponse {
                ok: true,
                method: "edit_message".into(),
            }).unwrap())
        }
        "self" => {
            let msg: EditSelfRequest = serde_json::from_str(content)?;
            handle_edit_self(handler, &msg).await?;
            Ok(serde_json::to_value(EditMessageResponse {
                ok: true,
                method: "edit_self".into(),
            }).unwrap())
        }
        "draft" => {
            let msg: EditDraftRequest = serde_json::from_str(content)?;
            handle_edit_draft(handler, &msg).await?;
            Ok(serde_json::to_value(EditDraftResponse {
                ok: true,
                method: "edit_draft".into(),
                chat_id: msg.chat_id,
            }).unwrap())
        }
        _ => Err("Unknown what field! Known what fields for edit: 'message', 'self'".into()),
    }
}

async fn on_delete(handler: &mut JsonHandler, content: &str) -> PPResult<DeleteMessageResponse> {
    let msg: DeleteMessageRequest = serde_json::from_str(content)?;

    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };

    let users_db: UsersDB = handler.get_db();
    let messages_db: MessagesDB = handler.get_db();
    let chats_db: ChatsDB = handler.get_db();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Chat with the given chat_id doesn't exist!")?;
    if messages_db
        .message_exists(real_chat_id, msg.message_id)
        .await?
    {
        let is_group = real_chat_id.is_negative();
        if is_group {
            let message_info = messages_db
                .fetch_messages(real_chat_id, msg.message_id..0)
                .await?;
            if message_info[0].from_id != self_user_id.as_i32_unchecked() {
                return Err("You aren't authorized to delete not yours message!".into());
            }
        }

        messages_db
            .delete_message(real_chat_id, msg.message_id)
            .await?;
        if !is_group {
            handler.send_event_to_con_detached(
                msg.chat_id,
                DeleteMessageEvent {
                    event: "delete_message".into(),
                    chat_id: self_user_id.as_i32_unchecked(),
                    message_id: msg.message_id,
                },
            );
        } else {
            let (chat, _)= chats_db.fetch_chat(&self_user_id, real_chat_id).await?.unwrap();
            // assert!(chat_details.is_group());

            // filter self
            for participant in chat
                .participants()
                .iter()
                .filter(|el| el.user_id() == self_user_id.as_i32_unchecked())
            {
                // send real chat id for everyone
                handler.send_event_to_con_detached(
                    participant.user_id(),
                    DeleteMessageEvent {
                        event: "delete_message".into(),
                        chat_id: msg.chat_id,
                        message_id: msg.message_id,
                    },
                );
            }
        }

        Ok(DeleteMessageResponse {
            ok: true,
            method: "delete_message".into(),
        })
    } else {
        Err("Message with the given message_id wasn't found!".into())
    }
}

async fn handle_messages(handler: &mut JsonHandler, method: &str) -> PPResult<Value> {
    let content = handler.utf8_content_unchecked().to_owned();
    match method {
        "edit" => Ok(serde_json::to_value(on_edit(handler, &content).await?).unwrap()),
        "delete" => Ok(serde_json::to_value(on_delete(handler, &content).await?).unwrap()),
        _ => Err("Unknown method".into()),
    }
}

pub async fn handle(handler: &mut JsonHandler, method: &str) {
    macros::require_auth!(handler, method);

    match handle_messages(handler, method).await {
        Ok(val) => {
            handler.send_message(&val).await;
        }
        Err(err) => {
            handler.send_error(method, err).await;
        }
    }
}
