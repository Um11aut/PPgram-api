use log::debug;

use crate::{
    db::{
        chat::{chats::ChatsDB, drafts::DraftsDB, hashes::HashesDB, messages::MessagesDB},
        internal::error::PPResult,
        user::UsersDB,
    },
    server::message::{
        handlers::json_handler::JsonHandler,
        types::{
            edit::EditedMessageBuilder,
            request::{
                delete::{DeleteAllMessagesRequest, DeleteChatRequest, DeleteMessagesRequest},
                edit::{EditDraftRequest, EditMessageRequest, EditSelfRequest, MarkAsReadRequest},
                extract_what_field,
            },
            response::{
                delete::{DeleteAllMessagesResponse, DeleteChatResponse, DeleteMessagesResponse},
                edit::{EditDraftResponse, EditMessageResponse, MarkAsReadResponse},
                events::{
                    DeleteAllMessagesEvent, DeleteMessageEvent, EditMessageEvent, EditSelfEvent,
                    IsTypingEvent, MarkAsReadEvent,
                },
            },
            user::{User, UserId},
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

    if !messages_db
        .message_exists(real_chat_id, msg.message_id)
        .await?
    {
        return Err("Message with the given message_id wasn't found!".into());
    }

    let private_chat_id = msg.chat_id;
    let msg_id = msg.message_id;

    let hashes_db: HashesDB = handler.get_db();

    if let Some(hashes) = msg.sha256_hashes.as_ref() {
        for hash in hashes.iter() {
            if !hashes_db.hash_exists(hash).await? {
                return Err(format!("Provided SHA256 Hash: {} doesn't exist!", hash).into());
            }
        }
    }

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
        let (chat, _) = chats_db
            .fetch_chat(&self_user_id, real_chat_id)
            .await?
            .unwrap();

        let ev = EditMessageEvent {
            event: "edit_message".into(),
            new_message: edited_msg,
        };

        let receivers: Vec<_> = chat
            .participants()
            .iter()
            .filter(|el| el.user_id() != self_user_id.as_i32_unchecked())
            .map(|u| (u.user_id(), ev.clone()))
            .collect();
        handler.send_events_to_connections(receivers);
    }

    Ok(())
}

async fn handle_mark_as_read(handler: &mut JsonHandler, msg: &MarkAsReadRequest) -> PPResult<()> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };

    let messages_db: MessagesDB = handler.get_db();
    let users_db: UsersDB = handler.get_db();
    let chats_db: ChatsDB = handler.get_db();

    let chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Given ChatId doesn't exist!")?;

    messages_db.mark_as_read(chat_id, &msg.message_ids).await?;

    let ev = MarkAsReadEvent {
        event: "mark_as_read".into(),
        chat_id: self_user_id.as_i32_unchecked(),
        message_ids: msg.message_ids.clone(),
    };

    if msg.chat_id.is_positive() {
        handler.send_event_to_con_detached(msg.chat_id, ev);
    } else {
        let mut ev = ev;
        let (group, _) = chats_db
            .fetch_chat(&self_user_id, chat_id)
            .await?
            .expect("chat to exist");
        ev.chat_id = msg.chat_id;

        let receivers: Vec<_> = group
            .participants()
            .iter()
            .filter(|u| u.user_id() != self_user_id.as_i32_unchecked())
            .map(|u| (u.user_id(), ev.clone()))
            .collect();

        handler.send_events_to_connections(receivers);
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
    let chats_db: ChatsDB = handler.get_db();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Chat with the given chat_id doesn't exist!")?;

    let event_chat_id = if msg.chat_id.is_positive() {
        self_user_id.as_i32_unchecked()
    } else {
        real_chat_id
    };

    let users = if msg.chat_id.is_positive() {
        vec![UserId::UserId(msg.chat_id)]
    } else {
        let (group, _) = chats_db
            .fetch_chat(&self_user_id, real_chat_id)
            .await?
            .expect("chat to exist");
        group
            .participants()
            .iter()
            .filter(|&u| u.user_id() != self_user_id.as_i32_unchecked())
            .map(|user| UserId::UserId(user.user_id()))
            .collect()
    };

    let ev = IsTypingEvent {
        event: "is_typing".into(),
        is_typing: true,
        chat_id: event_chat_id,
        user_id: self_user_id.as_i32_unchecked(),
    };

    handler.send_is_typing(ev, users).await;

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

    if let Some(&profile_color) = msg.profile_color.as_ref() {
        users_db
            .update_profile_color(&self_user_id, profile_color)
            .await?;
    }

    if let Some(username) = msg.username.as_ref() {
        users_db.update_username(&self_user_id, username).await?;
    }

    if let Some(hash) = msg.photo.as_ref() {
        let hashes_db: HashesDB = handler.get_db();
        if !hashes_db.hash_exists(hash).await? {
            return Err(format!("Provided SHA256 Hash: {} doesn't exist!", hash).into());
        }
        users_db.update_name(&self_user_id, hash).await?;
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
                        self_profile.profile_color(),
                    ),
                },
            );
        }
    }

    Ok(())
}

async fn handle_edit(handler: &mut JsonHandler, content: &str) -> PPResult<serde_json::Value> {
    let what_field = extract_what_field(content)?;

    match what_field.as_str() {
        "message" => {
            let msg: EditMessageRequest = serde_json::from_str(content)?;
            handle_edit_message(handler, msg).await?;
            Ok(serde_json::to_value(EditMessageResponse {
                ok: true,
                method: "edit_message".into(),
            })
            .unwrap())
        }
        "self" => {
            let msg: EditSelfRequest = serde_json::from_str(content)?;
            handle_edit_self(handler, &msg).await?;
            Ok(serde_json::to_value(EditMessageResponse {
                ok: true,
                method: "edit_self".into(),
            })
            .unwrap())
        }
        "draft" => {
            let msg: EditDraftRequest = serde_json::from_str(content)?;
            handle_edit_draft(handler, &msg).await?;
            Ok(serde_json::to_value(EditDraftResponse {
                ok: true,
                method: "edit_draft".into(),
            })
            .unwrap())
        }
        "is_unread" => {
            let msg: MarkAsReadRequest = serde_json::from_str(content)?;
            handle_mark_as_read(handler, &msg).await?;
            Ok(serde_json::to_value(MarkAsReadResponse {
                ok: true,
                method: "edit_is_unread".into(),
                chat_id: msg.chat_id,
            })
            .unwrap())
        }
        _ => Err("Unknown what field! Known what fields for edit: 'message', 'self', 'draft', 'is_unread'".into()),
    }
}

async fn on_delete_msgs(
    handler: &mut JsonHandler,
    msg: &DeleteMessagesRequest,
) -> PPResult<DeleteMessagesResponse> {
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

    for &msg_id in msg.message_ids.iter() {
        if !messages_db.message_exists(real_chat_id, msg_id).await? {
            return Err("Message with the given message_id wasn't found!".into());
        }
    }

    let is_group = real_chat_id.is_negative();
    if is_group {
        for &msg_id in msg.message_ids.iter() {
            let message_info = messages_db.fetch_messages(real_chat_id, msg_id..0).await?;
            if message_info[0].from_id != self_user_id.as_i32_unchecked() {
                return Err("You aren't authorized to delete not yours message!".into());
            }
        }
    }

    messages_db
        .delete_messages(real_chat_id, &msg.message_ids)
        .await?;
    if !is_group {
        handler.send_event_to_con_detached(
            msg.chat_id,
            DeleteMessageEvent {
                event: "delete_message".into(),
                chat_id: self_user_id.as_i32_unchecked(),
                message_ids: msg.message_ids.clone(),
            },
        );
    } else {
        let (chat, _) = chats_db
            .fetch_chat(&self_user_id, real_chat_id)
            .await?
            .unwrap();

        #[cfg(debug_assertions)]
        assert!(chat.is_group());

        // filter self
        for participant in chat
            .participants()
            .iter()
            .filter(|el| el.user_id() != self_user_id.as_i32_unchecked())
        {
            // send real chat id for everyone
            handler.send_event_to_con_detached(
                participant.user_id(),
                DeleteMessageEvent {
                    event: "delete_message".into(),
                    chat_id: msg.chat_id,
                    message_ids: msg.message_ids.clone(),
                },
            );
        }
    }

    Ok(DeleteMessagesResponse {
        ok: true,
        method: "delete_message".into(),
        chat_id: msg.chat_id,
        message_ids: msg.message_ids.clone(),
    })
}

async fn on_delete_all_messages(
    handler: &mut JsonHandler,
    msg: &DeleteAllMessagesRequest,
) -> PPResult<DeleteAllMessagesResponse> {
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

    messages_db.delete_all_messages(real_chat_id).await?;

    let event = DeleteAllMessagesEvent {
        event: "delete_all_messages".into(),
        chat_id: self_user_id.as_i32_unchecked(),
    };

    if msg.chat_id.is_positive() {
        handler.send_event_to_con_detached(msg.chat_id, event);
    } else {
        let mut event = event;
        let (group, _) = chats_db
            .fetch_chat(&self_user_id, real_chat_id)
            .await?
            .ok_or("Group wasn't found. WTF?")?;
        event.chat_id = msg.chat_id;

        let receivers: Vec<_> = group
            .participants()
            .iter()
            .filter(|u| u.user_id() != self_user_id.as_i32_unchecked())
            .map(|u| (u.user_id(), event.clone()))
            .collect();
        handler.send_events_to_connections(receivers);
    }

    Ok(DeleteAllMessagesResponse {
        ok: true,
        method: "delete_all_messages".into(),
        chat_id: msg.chat_id,
    })
}

async fn on_delete_chat(
    handler: &mut JsonHandler,
    msg: &DeleteChatRequest,
) -> PPResult<DeleteChatResponse> {
    let self_user_id = {
        let session = handler.session.read().await;
        let (user_id, _) = session.get_credentials_unchecked();
        user_id
    };

    let users_db: UsersDB = handler.get_db();
    let chats_db: ChatsDB = handler.get_db();

    let real_chat_id = users_db
        .get_associated_chat_id(&self_user_id, msg.chat_id)
        .await?
        .ok_or("Chat with the given chat_id doesn't exist!")?;

    chats_db.delete_chat(real_chat_id).await?;
    users_db
        .remove_associated_chat(&self_user_id, msg.chat_id)
        .await?;
    users_db
        .remove_associated_chat(&msg.chat_id.into(), self_user_id.as_i32_unchecked())
        .await?;

    Ok(DeleteChatResponse {
        ok: true,
        method: "delete_chat".into(),
        chat_id: msg.chat_id,
    })
}

async fn handle_delete(handler: &mut JsonHandler, content: &str) -> PPResult<serde_json::Value> {
    let what = extract_what_field(content)?;

    match what.as_str() {
        "all_messages" => Ok(serde_json::to_value(
            on_delete_all_messages(handler, &serde_json::from_str(content)?).await?,
        )
        .unwrap()),
        "chat" => Ok(serde_json::to_value(
            on_delete_chat(handler, &serde_json::from_str(content)?).await?,
        )
        .unwrap()),
        "messages" => Ok(serde_json::to_value(
            on_delete_msgs(handler, &serde_json::from_str(content)?).await?,
        )
        .unwrap()),
        _ => Err("Unknown what field provided!".into()),
    }
}

async fn handle_messages(handler: &mut JsonHandler, method: &str) -> PPResult<serde_json::Value> {
    let content = handler.utf8_content_unchecked().to_owned();
    match method {
        "edit" => Ok(handle_edit(handler, &content).await?),
        "delete" => Ok(handle_delete(handler, &content).await?),
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
