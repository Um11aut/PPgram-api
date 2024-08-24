import json
import socket
import struct
import time

def time_it(func):
    def wrapper(*args, **kwargs):
        start_time = time.time()  # Record the start time
        result = func(*args, **kwargs)  # Call the actual function
        end_time = time.time()  # Record the end time
        elapsed_time = end_time - start_time  # Calculate the elapsed time
        print(f"Function '{func.__name__}' took {elapsed_time:.6f} seconds to complete.")
        return result
    return wrapper

@time_it
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
    if len(length_bytes) < 4:
        raise ValueError("Received incomplete message length")

    # Convert the length bytes to an integer
    expected_size = struct.unpack('!I', length_bytes)[0]

    # Initialize a buffer to accumulate the incoming data
    chunks = b''

    # Now read the actual message based on the length
    while len(chunks) < expected_size:
        response_bytes = sock.recv(expected_size - len(chunks))
        if not response_bytes:
            raise ConnectionError("Connection lost during message reception")
        chunks += response_bytes

    response = chunks.decode('utf-8')
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
            response_bytes = sock.recv(message_length)

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
