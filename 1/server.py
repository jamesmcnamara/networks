from socket import socket, AF_INET, SOCK_STREAM


sock = socket(AF_INET, SOCK_STREAM)
sock.bind(("127.0.0.1", 5000))

print("listening on 5000...")
sock.listen(1)
a = 0
conn, addr = sock.accept()
while 1:
    try:
        print("receiving")
        data = conn.recv(1024)
        if data:
            print("received \"{}\" of type {}".format(data, type(data)))
        if a == 1000:
            conn.sendall(b"cs3700fall2015 BYE 24601\n")
        conn.sendall(b"cs3700fall2015 STATUS 5 + 6\n")
        a += 1
        print("done")
    except KeyboardInterrupt:
        sock.close()
        break

