# Pop HTTP server example

This is a multi-module TCP HTTP/1.1 website built with `Pop.Http` and
`Pop.Standard`. It parses typed requests, serves HTML and CSS, routes API
responses, calls SQLite through a declared foreign symbol, and closes each
caller-owned stream.

Build it with:

```text
pop build --manifestPath examples/httpServer/bubble.toml
pop run --manifestPath examples/httpServer/bubble.toml
```

The server listens on `127.0.0.1:18080`. In another terminal:

```text
curl http://127.0.0.1:18080/
curl http://127.0.0.1:18080/api/info
curl http://127.0.0.1:18080/api/network
curl http://127.0.0.1:18080/api/deadline
curl http://127.0.0.1:18080/api/host
curl http://127.0.0.1:18080/api/file
curl http://127.0.0.1:18080/api/sqlite
curl http://127.0.0.1:18080/api/workers
```

`src/main.pop` owns the TCP listener and parses HTTP. `src/site.pop` owns the
website and API routes, including a normal-browser GET fallback for requests
without `Content-Length`. `src/database.pop` binds SQLite directly, while
`src/workers.pop` contains the bounded structured worker workflow for dashboard
jobs. The routes demonstrate typed HTTP/TCP, process facts, scoped
file access, bounded reads, IPv4 parsing/classification, monotonic deadline
cleanup, and a foreign-library call.

`Pop.Standard` also exposes UDP, Unix sockets, DNS, interface and route
snapshots, deadlines/cancellation, multicast controls, TCP controls, and
Rustls-backed TLS through the same typed API.
