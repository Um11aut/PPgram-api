import socket
import simple

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
            "username": "@pepuk",
            "name": "Pepuk Alpha",
            "password_hash": "asd"
        }

        resp = simple.send_message(sock, register_message)
        print('Registering response: ' + resp)

        send_message_dict = {
            "method": "send_message",
            "to": -497022138,
            "has_reply": False,
            "reply_to": 0,
            "content": {
                "text": "Gay"
            }
        }

        while True:
            resp = simple.send_message(sock, send_message_dict)
            print('Sending Message Response: ' + resp)



    finally:
        # Close the socket
        sock.close()
