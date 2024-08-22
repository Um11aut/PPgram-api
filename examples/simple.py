import socket

from examples import send_message
from examples.api import listen_for_messages

# Example usage
if __name__ == "__main__":
    # Define the server address and port
    server_address = ('127.0.0.1', 8080)

    # Create a TCP/IP socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    try:
        # Connect the socket to the server
        sock.connect(server_address)

        # Define the login message
        login_message = {
            "method": "register",
            "username": "@pepuk",
            "name": "Pepuk Alpha",
            "password_hash": "asd"
        }

        # Send the login message
        login_response = send_message(sock, login_message)
        print('Response:', login_response)

        # Now listen for incoming messages indefinitely
        listen_for_messages(sock)

    except Exception as e:
        print(f"An error occurred during connection or messaging: {e}")
    finally:
        # Close the socket when done or in case of an error
        sock.close()
