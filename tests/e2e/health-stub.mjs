import { createServer } from "node:http";

const browserSession = {
  browser_session_id: "ses_018f0e0e7f7b70008000000000000000",
  workspace_id: "wsp_018f0e0e7f7b70008000000000000000",
  selected_profile_grant_id: "grt_018f0e0e7f7b70008000000000000000",
  is_current: true,
  created_at: "2026-08-31T12:00:00Z",
  last_seen_at: "2026-08-31T12:01:00Z",
  idle_expires_at: "2099-08-31T12:31:00Z",
  absolute_expires_at: "2099-08-31T20:00:00Z",
  rotation_generation: 1,
};

const accessProjection = {
  generated_at: "2026-08-31T12:01:00Z",
  subject: {
    auth_subject_id: "sub_018f0e0e7f7b70008000000000000000",
    lifecycle: "active",
    created_at: "2026-08-31T12:00:00Z",
    updated_at: "2026-08-31T12:01:00Z",
  },
  membership: {
    membership_id: "mem_018f0e0e7f7b70008000000000000000",
    workspace_id: browserSession.workspace_id,
    lifecycle: "active",
    role: "administrator",
    created_at: "2026-08-31T12:00:00Z",
    updated_at: "2026-08-31T12:01:00Z",
  },
  current_session: browserSession,
  sessions: [browserSession],
  sessions_truncated: false,
  profile_grants: [
    {
      profile_grant_id: browserSession.selected_profile_grant_id,
      profile_id: "prf_018f0e0e7f7b70008000000000000000",
      owner_client_id: "cli_018f0e0e7f7b70008000000000000000",
      selected: true,
    },
  ],
  profile_grants_truncated: false,
  session_policy: {
    idle_timeout_seconds: 1_800,
    browser_lifetime_seconds: 28_800,
    remembered_browser_lifetime_seconds: 2_592_000,
    last_seen_write_interval_seconds: 60,
  },
  authentication: {
    method: "trail_base_password",
    verified_at: "2026-08-31T12:00:00Z",
    activation_generation: 1,
    recent_authentication: { state: "unavailable", expires_at: null },
  },
  trailbase: {
    state: "active",
    blocker: null,
    trailbase_instance_id: "tbi_018f0e0e7f7b70008000000000000000",
    generation: 1,
    session_generation_current: true,
    updated_at: "2026-08-31T12:00:00Z",
  },
  first_run_steps: [
    { key: "account_confirmed", state: "verified" },
    { key: "strong_sign_in", state: "needs_attention" },
    { key: "recovery", state: "unavailable" },
    { key: "devices_and_clients", state: "unavailable" },
    { key: "external_identity", state: "unavailable" },
  ],
  evidence: [],
  evidence_truncated: false,
};

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
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405, { "content-type": "application/problem+json" });
    response.end(JSON.stringify({ title: "Method not allowed", status: 405 }));
    return;
  }

  let status = 200;
  let payload;
  if (url.pathname === "/api/v1/health") {
    payload = { status: "healthy", version: "0.1.0-test" };
  } else if (url.pathname === "/api/access/v1/projection") {
    payload = accessProjection;
  } else if (url.pathname === "/api/v1/profile/nuvio-collections") {
    payload = [];
  } else if (url.pathname === "/api/v1/profile/record-tracking-dispositions") {
    payload = [];
  } else if (
    url.pathname === "/api/v1/records" ||
    url.pathname.startsWith("/api/v1/records/")
  ) {
    payload = [];
  } else if (
    url.pathname === "/api/v1/reviews" ||
    url.pathname.startsWith("/api/v1/reviews/")
  ) {
    payload = [];
  } else if (url.pathname === "/api/v1/integrations") {
    payload = { integrations: [] };
  } else {
    status = 404;
    payload = { title: "Not found", status };
  }
  response.writeHead(status, {
    "content-type":
      status === 200 ? "application/json" : "application/problem+json",
  });
  response.end(request.method === "HEAD" ? undefined : JSON.stringify(payload));
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
