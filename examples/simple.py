import socket
import json
import struct

def send_message(sock: socket.socket, message: dict) -> str:
    """
    Send a message over an open socket and receive a response.

    Parameters:
    - sock: An open and connected socket object.
    - message: A dictionary to be sent as a JSON message.

    Returns:
    - response: The server's response as a string.
    """

    # Convert the dictionary to a JSON string
    message_json = json.dumps(message)
    message_bytes = message_json.encode('utf-8')

    # Calculate the length of the JSON message
    message_length = len(message_bytes)

    # Convert the length to a 4-byte integer in network byte order
    message_length_bytes = struct.pack('!I', message_length)

    # Combine the length and the message
    message_to_send = message_length_bytes + message_bytes

    # Send the length-prefixed message
    sock.sendall(message_to_send)

    # First, read the first 4 bytes to get the length of the incoming message
    length_bytes = sock.recv(4)

    # Convert the length bytes to an integer
    message_length = struct.unpack('!I', length_bytes)[0]

    # Now read the actual message based on the length
    response_bytes = sock.recv(message_length)
    response = response_bytes.decode('utf-8')

    return response

def listen_for_messages(sock: socket.socket):
    """
    Continuously listen for incoming messages from the server.

    Parameters:
    - sock: An open and connected socket object.
    """
    try:
        while True:
            # First, read the first 4 bytes to get the length of the incoming message
            length_bytes = sock.recv(4)
            if not length_bytes:
                break  # No more data

            # Convert the length bytes to an integer
            message_length = struct.unpack('!I', length_bytes)[0]
            print("Expected message length:", message_length)

            # Initialize a buffer for the full message
            response_bytes = b''

            # Ensure the full message is received
            while len(response_bytes) < message_length:
                chunk = sock.recv(message_length)
                if not chunk:
                    break  # Connection closed or error
                response_bytes += chunk

            print("Actual message length received:", len(response_bytes))

            try:
                # Attempt to decode the message as UTF-8
                response = response_bytes.decode('utf-8')
                print('Received:', response)
            except UnicodeDecodeError as e:
                print(f"Decoding error: {e}")
                # Handle or log binary data appropriately
                # For example, save to a file, or process differently

    except Exception as e:
        print(f"An error occurred: {e}")
    finally:
        # Ensure the socket is closed in case of any errors
        sock.close()

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
            "method": "login",
            "username": "@pepuk",
            "name": "Pepuk Alpha",
            "user_id": -100878589,
            "password_hash": "asd"
        }

        # Send the login message
        login_response = send_message(sock, login_message)
        print('Login Response:', login_response)

        # Now listen for incoming messages indefinitely
        listen_for_messages(sock)

    except Exception as e:
        print(f"An error occurred during connection or messaging: {e}")
    finally:
        # Close the socket when done or in case of an error
        sock.close()
