import socket
import time

from api import send_message

# Example usage
if __name__ == "__main__":
    # Define the server address and port
    server_address = ('6.tcp.eu.ngrok.io', 16349)

    # Create a TCP/IP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    try:
        # Connect the socket to the server
        sock.connect(server_address)

        # Define the messages as dictionaries
        login_message = {
            "method": "login",
            "username": "@pavlo",
            "name": "Pepuk Alpha",
            "password_hash": "asd"
        }
        print(send_message(sock, login_message))

        fetch = {
            "method": "fetch",
            "what": "chats"
        }
        print(send_message(sock, fetch))

        fetch = {
            "method": "fetch",
            "what": "messages",
            "chat_id": -2079655369,
            "range": [-1, -500]
        }
        print(len(send_message(sock, fetch)))

    finally:
        # Close the socket
        sock.close()
