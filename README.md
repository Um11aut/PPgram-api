## PPgram-api
### Building
Using docker:
```bash
docker-compose up --build
docker exec -ti rust-app /bin/bash
$ cargo run --release
```
Wait until the database is fully started.

The server is accessible via tcp by the adress `127.0.0.1:8080`. Example of sending the message:
```py
import socket
import json
import struct

def send_message():
    # Define the message as a dictionary
    message = {
        "method": "register",
        "username": "@pepuk",
        "name": "Pepuk Pidar",
        "password_hash": "asd"
    }

    # Convert the dictionary to a JSON string
    message_json = json.dumps(message)
    message_bytes = message_json.encode('utf-8')

    # Calculate the length of the JSON message
    message_length = len(message_bytes)

    # Convert the length to a 4-byte integer in network byte order
    message_length_bytes = struct.pack('!I', message_length)

    # Combine the length and the message
    message_to_send = message_length_bytes + message_bytes

    # Define the server address and port
    server_address = ('127.0.0.1', 8080)

    # Create a TCP/IP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    try:
        # Connect the socket to the server
        sock.connect(server_address)

        # Send the length-prefixed message
        sock.sendall(message_to_send)

        # Loop to receive the response from the server indefinitely
        while True:
            # First, read the first 4 bytes to get the length of the incoming message
            length_bytes = sock.recv(4)
            if not length_bytes:
                break

            # Convert the length bytes to an integer
            message_length = struct.unpack('!I', length_bytes)[0]

            # Now read the actual message based on the length
            response_bytes = sock.recv(message_length)
            response = response_bytes.decode('utf-8')

            print('Received:', response)
    finally:
        # Close the socket
        sock.close()

if __name__ == "__main__":
    send_message()
```