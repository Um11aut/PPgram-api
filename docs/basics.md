# Basics of usage of the PPgram API
PPgram API works over raw TCP without any wrapper protocols.
There are two provided for usage ports:
* 3000 - For JSON Messages
* 8080 - For Files Messages

### JSON Message deliverment
Every small message on the PPgram API is delivered via JSON.
Every JSON Request has one guaranteed field: `method`

Method describes what exactly you want to do: authentification, messaging, creating group, etc.
All of the possible request variants are stored in `Deserialize` structs. You can find them under `src/server/message/types/request`

Okay, now when we defined what we want to send, how to actually send it?
Each time you want to send something, you need to include the size of the JSON Message as byte representation of big-endian x64 integer(4 bytes)

It looks like this:
| Byte index   | 0  | 1  | 2  | 3  | ... |
|---------|----|----|----|----|----------|
| Value | 0  | 0  | 0  | 255 | [JSON Message with length of 255] |

Let's suppose you want to deliver JSON Message to the server, like registering the user.

You take look at `src/server/message/types/request/auth.rs`:
```rs
#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterRequest {
    pub method: String,
    pub name: String,
    pub username: String,
    pub password: String
}
```

Request will look like this:

```json
{
    "method": "register",
    "name": "John Smith",
    "username": "@john_smith",
    "password": "johnisnotdumb"
}
```
Note: It's recommended to use non-space format of JSON to save data traffic.

Now you write to the TCP buffer the length of this message and then the message itself.

### Response
Each response must contain `ok` field. It indicates, if your request was successfully processed.
```json
{
    "ok": true,
    "method": "register",
    "session_id": "...",
    "user_id": "..."
}
```
If request wasn't successfully processed, you get `ok: false`, as well as error description:
```json
{
    "ok": false,
    "method": "register",
    "error": "User with the given credentials already exists!"
}
```
All the possible responses can be found in `src/message/type/response`.

### Events
To be able to get realtime updates, the event system was created. Each `TCPConnection` contains `mpsc`(Multiple Producer, Single Consumer), to be able to send events anywhere from the code!
All the authenticated sessions are stored in according `HashMap`, which is called `Sessions`. When user authenticates, it's user_id and `Arc` session is being added there.
If some user wants to send a message to another, after adding message to the database, API:
* searches for the target user by the provided `user_id`
* If found, sends the event on receiver handler task
* Then, the `Mutex` will wait until the socket is free, and then send the intended message

Events are always being sent on the first user connection

Events are guaranteed to have `event` as Event Identifier.
Example:
```rs
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteMessageEvent {
    pub event: String, // delete_message
    pub chat_id: i32,
    pub message_id: i32,
}
```
All events can be found in `src/message/type/response/events.rs`
