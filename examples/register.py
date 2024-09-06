import socket
from api import send_message, listen_for_messages

if __name__ == "__main__":
    # Define the server address and port
    server_address = ('127.0.0.1', 8080)

    # Create a TCP/IP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    try:
        # Connect the socket to the server
        sock.connect(server_address)

        # Define the messages as dictionaries
        register_message = {
            "method": "login",
            "username": "@pavlo",
            "name": "Pavlo Alpha",
            "password_hash": "huzpidaras"
        }

        resp = send_message(sock, register_message)
        print('Response: ' + resp)

        fetch = {
            "method": "fetch",
            "what": "media",
            "media_hash": "ad175a0bf3ae35303b54157bf176139d63b1b3e3003cc624c0e9e30f3b762ff3",
        }
        print(len(send_message(sock, fetch)))

    finally:
        # Close the socket
        sock.close()
