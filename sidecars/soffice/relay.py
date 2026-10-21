"""Dev-only TCP relay: listens on :8080, forwards to $TARGET (host:port). Lets a published port reach a
sidecar that sits on an --internal Docker network (which cannot publish ports itself)."""
import os, socket, threading

host, port = os.environ["TARGET"].split(":")


def pipe(a, b):
    try:
        while d := a.recv(65536):
            b.sendall(d)
    except OSError:
        pass
    finally:
        for s in (a, b):
            try: s.shutdown(socket.SHUT_RDWR)
            except OSError: pass


srv = socket.create_server(("0.0.0.0", 8080))
while True:
    c, _ = srv.accept()
    u = socket.create_connection((host, int(port)))
    threading.Thread(target=pipe, args=(c, u), daemon=True).start()
    threading.Thread(target=pipe, args=(u, c), daemon=True).start()
