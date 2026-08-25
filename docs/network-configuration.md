# Network configuration

Fasti keeps the listener, client URL, and public URL separate. This prevents a
reverse-proxy address from changing the daemon bind address.

| Variable | Owner | Purpose |
| --- | --- | --- |
| `FASTI_LISTEN` | `fastid` | Bind address as `IP:PORT`. Default: `127.0.0.1:8420`. |
| `FASTI_PORT` | launcher | Native or container host port. Default: `8420`. |
| `FASTI_PORT_FALLBACK` | `fastid` and launcher | `fail` stops on a collision. Explicit `auto` selects an OS-assigned loopback port. Default: `fail`. |
| `FASTI_API_URL` | launcher or app build | Origin used by a client or health probe. Do not include credentials, a path, a query, or a fragment. |
| `FASTI_PUBLIC_URL` | launcher or app settings | External origin shown to people. It does not bind a socket or configure a proxy. |
| `FASTI_CONTAINER_RUNTIME` | launcher | `podman` or `docker`. Default: `podman`. |
| `FASTI_BOUND_ADDR_FILE` | supervisor | Optional file where `fastid` atomically publishes its actual bind address. |

Non-loopback client and public URLs must use HTTPS. `localhost` and
`127.0.0.1` are interchangeable when local name resolution uses IPv4. `[::1]`
is a separate IPv6 listener choice.

## Use a custom port

Native:

```bash
FASTI_PORT=19420 ./scripts/dev.sh
```

Podman or Docker:

```bash
FASTI_PORT=19420 ./scripts/dev.sh --podman
FASTI_PORT=19420 ./scripts/dev.sh --docker
```

The container always listens on port `8420` internally. The launcher maps the
configured host port to it. The memory ceiling remains 192 MiB with no extra
swap.

## Recover from a port collision

Set `FASTI_PORT_FALLBACK=auto` to recover from a loopback collision. Fasti asks
the operating system for an available port on the same loopback address. It
publishes and reports the actual address before the health probe. Direct daemon
starts fail by default so a client cannot remain on an occupied old port.

Fasti never moves a wildcard or public listener. It also stops when automatic
fallback would make an explicit `FASTI_API_URL` or `FASTI_PUBLIC_URL` false.
Use fail-closed behavior when a proxy has a fixed upstream port:

```bash
FASTI_PORT_FALLBACK=fail FASTI_PORT=8420 ./scripts/dev.sh
```

## Use a portless public URL

A URL omits its port only when the public entry point uses the default HTTP or
HTTPS port. Set the external address separately:

```bash
FASTI_PUBLIC_URL=https://fasti.internal ./scripts/dev.sh
```

This variable does not create DNS, TLS, or a reverse proxy. Fasti does not yet
publish a remote-exposure recipe. A reverse proxy can erase the daemon's
loopback trust boundary, so public routing remains blocked until authenticated
inbound access and its threat-model gate are active.

`.internal` needs working name resolution and a certificate whose subject
includes that host. Fasti uses the platform trust store. It does not issue a
root certificate, install a certificate authority, or bypass certificate
validation. Add the issuing root CA through the operating system or managed
device policy when a private CA is required.

For a managed browser, Tauri, or APK build, project the same values into the
app at build time:

```bash
VITE_FASTI_API_URL=https://fasti.internal \
VITE_FASTI_PUBLIC_URL=https://fasti.internal \
VITE_FASTI_PORT_FALLBACK=fail \
pnpm build
```

## Provider network policy

Provider manifests declare their maximum hosts, capabilities, and network
classes. Operator allow lists can only narrow that declaration. A deny always
wins. The application policy rejects empty or mixed DNS results and unsafe
addresses before an adapter connects.

Adapters must ignore system proxies, disable redirects, authorize every
resolved address, and connect to one authorized address without a second DNS
lookup. Provider credentials must use headers or a platform credential store.
They must not enter URLs, arguments, logs, browser storage, screenshots,
fixtures, or proof bundles.

The Google Books review manifest accepts `GOOGLE_BOOKS_API_KEY` and sends it as
`X-Goog-Api-Key`. It remains a contract until the provider runtime body is
authorized. The browser does not execute provider requests.

## Contract disposition

These values configure startup and client transport. They do not add a public
Fasti capability, HTTP route, event, domain entity, or linked-data term.
OpenAPI, AsyncAPI, JSON Schema, and JSON-LD therefore remain unchanged.
