# Network configuration

Fasti keeps the listener, client URL, and public URL separate. This prevents a
reverse-proxy address from changing the daemon bind address.

| Variable | Owner | Purpose |
| --- | --- | --- |
| `FASTI_LISTEN` | `fastid` | Bind address as `IP:PORT`. Default: `127.0.0.1:8420`. |
| `FASTI_PORT` | launcher | Native or container host port. Default: `8420`. |
| `FASTI_PORT_FALLBACK` | `fastid` and launcher | `auto` selects an OS-assigned port after a loopback collision. `fail` stops instead. |
| `FASTI_API_URL` | launcher or app build | Origin used by a client or health probe. Do not include credentials, a path, a query, or a fragment. |
| `FASTI_PUBLIC_URL` | launcher or app settings | External origin shown to people. It does not bind a socket or configure a proxy. |
| `FASTI_CONTAINER_RUNTIME` | launcher | `podman` or `docker`. Default: `podman`. |
| `FASTI_BOUND_ADDR_FILE` | supervisor | Optional file where `fastid` atomically publishes its actual bind address. |

Non-loopback client and public URLs must use HTTPS. `localhost`, `127.0.0.1`,
and `[::1]` are equivalent loopback choices. The app can show all three for the
active port.

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

`FASTI_PORT_FALLBACK=auto` is the default. If a loopback port is occupied,
Fasti asks the operating system for an available port on the same loopback
address. It publishes and reports the actual address before the health probe.

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

This variable does not create DNS, TLS, or a reverse proxy. Configure those at
the network edge. This example keeps the development web server and daemon on
loopback and exposes one HTTPS origin:

```caddyfile
https://fasti.internal {
    tls /etc/fasti/tls/fasti.crt /etc/fasti/tls/fasti.key

    handle /api/* {
        reverse_proxy 127.0.0.1:8420
    }

    handle {
        reverse_proxy 127.0.0.1:5173
    }
}
```

`.internal` needs working name resolution and a certificate whose subject
includes that host. Fasti uses the platform trust store. It does not issue a
root certificate, install a certificate authority, or bypass certificate
validation. Add the issuing root CA through the operating system or managed
device policy when a private CA is required.

## Contract disposition

These values configure startup and client transport. They do not add a public
Fasti capability, HTTP route, event, domain entity, or linked-data term.
OpenAPI, AsyncAPI, JSON Schema, and JSON-LD therefore remain unchanged.
