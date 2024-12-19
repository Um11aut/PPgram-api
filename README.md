## PPgram-api
A complete robust Server for the chat application.

### Building
## Development environment:
Using docker:
```bash
docker-compose up --build
docker exec -ti rust-app /bin/bash
$ cargo run --release
```
The server is accessible via TCP for basic JSON messages by the adress `0.0.0.0:3000`. And via TCP for files `0.0.0.0:8080`
