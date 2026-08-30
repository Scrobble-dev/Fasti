# Security boundary

## Current support state

Fasti is not a supported public release. Security controls still apply to local
review, development, documentation, and reports.

## Credentials

Keep bootstrap secrets, initialization proofs, client credentials, provider
keys, session values, and recovery secrets out of:

- URLs and query strings;
- command history and process arguments;
- browser local and session storage;
- logs and telemetry;
- screenshots and fixtures;
- exports and public issue reports.

Use the Authorization header or the documented body field only through a
trusted host that keeps the value in memory or an approved credential store.

## Listener and proxy trust

Bootstrap is local. A remote durable listener exposes only the authenticated
non-bootstrap subset and requires an explicitly trusted HTTPS proxy and public
origin. Do not trust forwarded headers from an undeclared proxy. Do not use a
wildcard listener as proof of safe public access.

## Provider network access

Resolve and authorize every destination address. Pin the allowed host. Disable
ambient proxy use and redirects before credential access. Reject private,
loopback, link-local, or changed destinations according to the provider policy.

## Public reports

Report a suspected vulnerability through the private process in `SECURITY.md`.
Do not open a public issue for an undisclosed security problem.

## Safe next action

- For a typed application failure, use [Problems](/integrate/problems/).
- For a local listener, use [Local review](/operate/local-review/).

Content state: STE-controlled draft.
