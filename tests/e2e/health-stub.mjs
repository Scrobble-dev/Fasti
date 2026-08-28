import { createServer } from "node:http";

const server = createServer((request, response) => {
  response.setHeader("access-control-allow-origin", "*");
  response.setHeader(
    "access-control-allow-methods",
    "GET, HEAD, POST, PUT, DELETE, OPTIONS",
  );
  response.setHeader("access-control-allow-headers", "*");
  if (request.method === "OPTIONS") {
    response.writeHead(204).end();
    return;
  }
  const url = new URL(request.url, "http://127.0.0.1:18422");
  response.writeHead(200, { "content-type": "application/json" });

  if (url.pathname.startsWith("/api/v1/health")) {
    response.end(JSON.stringify({ status: "healthy", version: "0.1.0-test" }));
  } else if (url.pathname.startsWith("/api/v1/browser/session")) {
    response.end(JSON.stringify({ authenticated: false, session: null }));
  } else if (url.pathname.startsWith("/api/v1/profile/nuvio-collections")) {
    response.end(JSON.stringify([]));
  } else if (
    url.pathname.startsWith("/api/v1/profile/record-tracking-dispositions")
  ) {
    response.end(JSON.stringify([]));
  } else if (url.pathname.startsWith("/api/v1/records")) {
    response.end(JSON.stringify([]));
  } else if (url.pathname.startsWith("/api/v1/reviews")) {
    response.end(JSON.stringify([]));
  } else if (url.pathname.startsWith("/api/v1/integrations")) {
    response.end(JSON.stringify({ integrations: [] }));
  } else {
    response.end(JSON.stringify({ status: "healthy", data: [] }));
  }
});

server.on("clientError", (err, socket) => {
  if (err.code === "ECONNRESET" || !socket.writable) {
    return;
  }
  socket.end("HTTP/1.1 400 Bad Request\r\n\r\n");
});

server.on("error", (err) => {
  console.error("health-stub server error:", err);
});

process.on("uncaughtException", (err) => {
  console.error("health-stub uncaught exception:", err);
});

server.listen(18422, "127.0.0.1");
