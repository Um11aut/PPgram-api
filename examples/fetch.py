import socket
import time

from api import listen_for_messages, send_message

# Example usage
if __name__ == "__main__":
    # Define the server address and port
    server_address = ('127.0.0.1', 8080)

    # Create a TCP/IP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    try:
        # Connect the socket to the server
        sock.connect(server_address)
        bind = {
            "method": "bind",
            "session_id": "Fm8rUecSkI7ueI9C8pKun2OrPplrk8",
            "user_id": -497022138
        }
        print(send_message(sock, bind))

        fetch = {
            "method": "fetch",
            "what": "chats"
        }
        print(send_message(sock, fetch))

        fetch = {
            "method": "fetch",
            "what": "messages",
            "chat_id": -2079655369,
            "range": [-1, -300_000]
        }
        print(len(send_message(sock, fetch)))

        listen_for_messages(sock)

    finally:
        # Close the socket
        sock.close()
