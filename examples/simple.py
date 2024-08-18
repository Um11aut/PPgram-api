import socket
import json
import struct

def send_message(sock, message):
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
            "method": "login",
            "username": "@pavlo",
            "name": "Pepuk Alpha",
            "user_id": -100878589,
            "password_hash": "asd"
        }

        send_message_dict = {
            "method": "send_message",
            "to": -1609472930,
            "has_reply": False,
            "reply_to": 0,
            "content": {
                "text": "Pepuk pidar"
            }
        }

        check_message = {
            "method": "check",
            "what": "username",
            "data": "@pepuk"
        }

        # Send the login message
        login_response = send_message(sock, login_message)
        print('Login Response:', login_response)

        fetch = {
            "method": "fetch",
            "what": "chats",
        }

        res = send_message(sock, fetch)
        print('Fetching result', res)

        fetch = {
            "method": "fetch",
            "what": "user",
            "username": "@pavlo"
        }

        res = send_message(sock, fetch)
        print('Fetching result', res)

        fetch = {
            "method": "fetch",
            "what": "self",
        }

        res = send_message(sock, fetch)
        print('Fetching result', res)

        res = send_message(sock, check_message)
        print('Check Response:', res)

        # Send another message
        send_message_response = send_message(sock, send_message_dict)
        print('Send Message Response:', send_message_response)
        send_message_response = send_message(sock, send_message_dict)
        print('Send Message Response:', send_message_response)
        send_message_response = send_message(sock, send_message_dict)
        print('Send Message Response:', send_message_response)
        send_message_response = send_message(sock, send_message_dict)
        print('Send Message Response:', send_message_response)
        send_message_response = send_message(sock, send_message_dict)
        print('Send Message Response:', send_message_response)

    finally:
        # Close the socket
        sock.close()
