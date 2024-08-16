## PPgram-api
### Building
Using docker:
```bash
docker-compose up --build
docker exec -ti rust-app /bin/bash
$ cargo run --release
```
Wait until the database is fully started.

The server is accessible via tcp by the adress `127.0.0.1:8080`. 

## Usage
You can find examples for using API in `examples` folder. There is also a [desktop client](https://github.com/pepukcoder/PPgram-desktop) for windows.
In the `examples` folder are some basic examples for authentication, sending messages, checking if username exists etc.