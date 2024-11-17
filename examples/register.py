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
            "method": "register",
            "username": "@pavlo",
            "name": "Pavlo Alpha",
            "password": "huzpidaras"
        }

        resp = send_message(sock, register_message)
        print('Response: ' + resp)

    finally:
        # Close the socket
        sock.close()
