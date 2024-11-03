## PPgram-api
### Building
Using docker:
```bash
docker-compose up --build
docker exec -ti rust-app /bin/bash
$ cargo run --release
```
Wait until the database is fully started.

The server is accessible via TCP for basic JSON messages by the adress `0.0.0.0:3000`. And via TCP for files `0.0.0.0:8080` 

## Basics
### Protocol
PPgram-api doesn't use any extern TCP Protocol. The messages transmitting is very simple: you just put 4 bytes as bytes representation of big-endian integer on the start as the length of the upcoming message(not the full message, but the content you want to send).

Data alignment for JSON Messages:

| Byte index   | 0  | 1  | 2  | 3  | ... |
|---------|----|----|----|----|----------|
| Value | 0  | 0  | 0  | 255| [content with length of 255] |

Data alignment for File Messages:

| Byte index   | 0  | 1  | 2  | 3  | ... |
|---------|----|----|----|----|----------|
| [Metadata size - 4 bytes] | 0  | 0  | 0  | 255| [] |

## Usage
You can find examples for using API in `examples` folder. There is also a [desktop client](https://github.com/pepukcoder/PPgram-desktop) for windows.
In the `examples` folder are some basic examples for authentication, sending messages, checking if username exists etc.