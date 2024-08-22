import socket

from examples import send_message

# Example usage
if __name__ == "__main__":
    # Define the server address and port
    server_address = ('127.0.0.1', 8080)

    # Create a TCP/IP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    try:
        # Connect the socket to the server
        sock.connect(server_address)

        # Define the messages as dictionaries
        login_message = {
            "method": "register",
            "username": "@pavlo",
            "name": "Pepuk Alpha",
            "password_hash": "asd"
        }

        fetch = {
            "method": "fetch",
            "what": "chats"
        }
        print(send_message(sock, fetch))

    finally:
        # Close the socket
        sock.close()
