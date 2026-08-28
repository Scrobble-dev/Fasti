# Network configuration

Fasti keeps the listener, client URL, and public URL separate. This prevents a
reverse-proxy address from changing the daemon bind address.

| Variable                         | Owner                    | Purpose                                                                                                                           |
| -------------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `FASTI_LISTEN`                   | `fastid`                 | Bind address as `IP:PORT`. Default: `127.0.0.1:8420`.                                                                             |
| `FASTI_PORT`                     | launcher                 | Native or container host port. Default: `8420`.                                                                                   |
| `FASTI_PORT_FALLBACK`            | `fastid` and launcher    | `fail` stops on a collision. Explicit `auto` selects an OS-assigned loopback port. Default: `fail`.                               |
| `FASTI_DATA_ROOT`                | `fastid`                 | Required private directory for every durable router. It is never inferred.                                                        |
| `FASTI_API_URL`                  | launcher or app build    | Origin used by a client or health probe. Do not include credentials, a path, a query, or a fragment.                              |
| `FASTI_PUBLIC_URL`               | launcher or app settings | External origin shown to people. It does not bind a socket or configure a proxy.                                                  |
| `FASTI_REMOTE_TRUSTED_PROXY`     | `fastid`                 | Exact `true` opts a non-loopback durable listener into the authenticated router behind trusted TLS termination. Default: `false`. |
| `FASTI_DEVELOPMENT_TEST_ACCOUNT` | `fastid`                 | Exact `true` seeds the one-time `testadmin` account on a loopback durable listener. Default: `false`. Remote listeners reject it. |
| `GOOGLE_BOOKS_API_KEY`           | Desktop provider adapter | Optional process-managed Google Books key. It overrides the app credential store and is sent only in `X-Goog-Api-Key`.            |
| `TMDB_API_READ_ACCESS_TOKEN`     | Desktop provider adapter | Optional process-managed TMDB API Read Access Token. It overrides the app credential store and is sent only as a bearer header.   |
| `FASTI_CONTAINER_RUNTIME`        | launcher                 | `podman` or `docker`. Default: `podman`.                                                                                          |
| `FASTI_EXTERNAL_BIND_IP`         | `fastid` and launcher    | Explicit outer bind IP for a wildcard container listener. Only a loopback IP inside a detected container boundary is accepted.   |
| `FASTI_BOUND_ADDR_FILE`          | supervisor               | Optional file where `fastid` atomically publishes its actual bind address.                                                        |

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
configured host port to it on `127.0.0.1` and sets
`FASTI_EXTERNAL_BIND_IP=127.0.0.1`. That explicit assertion lets `fastid`
mount the durable API through the wildcard container socket. A wildcard
listener without the assertion stays health-only. A native process cannot
replay the assertion because `fastid` also requires the container isolation
boundary. A non-loopback assertion is rejected. Do not set this variable when
the outer published address is public.
The launcher runs the container process as the invoking non-root user so its
worktree-owned data directory remains writable. Podman uses `keep-id` for the
rootless user namespace. The image default remains the non-root `fasti` user.
The memory ceiling remains 192 MiB with no extra swap.

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

This variable does not create DNS, TLS, or a reverse proxy. A development
Remote instance can mount authenticated durable routes only when all of these
conditions are true:

```bash
FASTI_LISTEN=0.0.0.0:8420 \
FASTI_DATA_ROOT=/path/to/private/fasti-data \
FASTI_REMOTE_TRUSTED_PROXY=true \
FASTI_PUBLIC_URL=https://fasti.internal \
cargo run --locked -p fastid
```

The trusted proxy must terminate HTTPS and must not expose the cleartext
upstream directly. The remote router omits node initialization and first-client
enrollment. It accepts browser sessions or existing scoped bearer credentials
for durable data routes. Remote session cookies are `Secure`, `HttpOnly`, and
`SameSite=Strict`; the readable strict CSRF cookie is required as an exact
`X-Fasti-CSRF` proof for browser mutations.

Development accounts are disabled by default in every build. For an isolated
loopback-only data root, an operator can opt in explicitly:

```bash
FASTI_LISTEN=127.0.0.1:8420 \
FASTI_DATA_ROOT=/path/to/private/fasti-data \
FASTI_DEVELOPMENT_TEST_ACCOUNT=true \
cargo run --locked -p fastid
```

This creates `testadmin` with password `testadmin` once and marks it as a test
account. A remote listener rejects the setting. Rename and deletion do not
clear the durable seed marker, so startup never recreates the account. Fasti
also rejects browser-account or credential changes that would leave no active
administrator that can manage browser users. This is not a supported production
provisioning or recovery path.

`.internal` needs working name resolution and a certificate whose subject
includes that host. Fasti uses the platform trust store. It does not issue a
root certificate, install a certificate authority, or bypass certificate
validation. Add the issuing root CA through the operating system or managed
device policy when a private CA is required.

For a managed Tauri or APK build, project the app-owned values into the Rust
build. The trusted host reads the same names at run time first, then falls back
to the compiled values:

```bash
FASTI_API_URL=https://fasti.internal \
FASTI_PUBLIC_URL=https://fasti.internal \
cargo build --release --manifest-path apps/desktop/src-tauri/Cargo.toml
```

The browser Workbench development server deliberately uses its configured Vite
proxy. Listener and collision variables belong to `fastid`, not to the browser,
embedded Tauri, or APK kernel.

## Edit desktop settings

The trusted Tauri host exposes its service URL, public URL, and outbound policy
in **Settings → Advanced Network Access**. Saved values use the app
configuration directory. Environment values take precedence over app-build
values, which take precedence over saved values and defaults. Managed values
remain visible and read-only.

The service URL owns the app's target host and port. With an IPv4 loopback
service, clients can use `127.0.0.1` or `localhost`. IPv6 `[::1]` remains a
separate choice. **Test service URL** resolves once, rejects unsafe plain HTTP,
ignores system proxies, disables redirects, pins the resolved addresses, and
validates the generated health response. Host lookups have a five-second
deadline. Only one system lookup can run at a time, so a stalled resolver cannot
accumulate provider or health-check work.

The Tauri host embeds the local kernel. It does not start or rebind `fastid`.
The daemon and container launcher therefore remain the only owners of
`FASTI_LISTEN`, `FASTI_PORT`, `FASTI_PORT_FALLBACK`, and bound-address
publication. Settings does not present an unconsumed listener control.

Desktop builds still require an explicit, non-empty `FASTI_DATA_ROOT`. Android
uses an explicit `FASTI_DATA_ROOT` when the launch environment supplies one;
otherwise it uses the app's sandbox data directory. The explicit override wins.
Before SQLite opens, Android retains the physical data-root directory and takes
the same exclusive `fasti.lock` used by the daemon. The Android lock uses
`openat` with no symbolic-link following so it works on kernels that do not
provide Linux B3's stronger `openat2` restore primitives. Android does not
activate B3 restore or startup recovery. Those paths remain Linux-only until
they have separate platform evidence. Fixed kernel directories and evidence
prefixes are opened relative to retained directory descriptors without
following symbolic links. This Android path is implemented in source. It has
not passed an Android NDK build or device test in this worktree, so Android
package support is not yet verified.

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

The Google Books adapter requires `GOOGLE_BOOKS_API_KEY` or a credential saved
by the trusted Tauri host. The TMDB adapter uses the corresponding
`TMDB_API_READ_ACCESS_TOKEN` or stored credential. Environment credentials take
precedence and are read-only in Settings. The host loads a credential only
after outbound access is authorized. It sends Google Books keys in the
sensitive `X-Goog-Api-Key` header and TMDB tokens in the sensitive
`Authorization: Bearer` header.

Search returns at most ten neutral candidates. Selecting a candidate does not
trust the earlier search body: the host performs a bounded `metadata.read` for
the exact provider ID. If that exact response contains artwork, the host then
authorizes a separate `metadata.artwork` request to `image.tmdb.org`,
`books.google.com`, or `books.googleusercontent.com`. It resolves and
authorizes every address, pins a proxy-free and redirect-free client, sends no
provider credential, and accepts only JPEG, PNG, or WebP content up to 2 MB and
4096 pixels per dimension. A failure occurs before the metadata write.

Validated artwork is written atomically to an owner-only app cache. The cache
keeps at most 128 content-addressed files. `list_records` exposes a local path
only when the file is regular, bounded, and still has safe image dimensions.
The Desktop adapter converts that path through Tauri's asset protocol, whose
runtime scope contains only the artwork cache directory. Remote provider URLs
remain provenance claims and never reach a Desktop image element.

After network work succeeds, the host passes validated identifiers and field
claims to the local application port. Creating a provider record commits the
Record, identifier, and claims atomically. Refresh attaches or verifies the
selected identifier and appends claims without rewriting earlier provider
evidence. Provider failure leaves existing identity and Chronicle state
unchanged. The browser does not receive credentials or execute provider
requests.

TMDB's [API FAQ](https://developer.themoviedb.org/docs/faq) requires attribution
for applications that use its API. The Workbench
includes the approved TMDB mark and required non-endorsement notice in Provider
credits. Commercial deployments need the appropriate TMDB licence; configuring
a token does not grant one. Poster URLs are stored as provider claims. The
Desktop CSP still rejects remote images; it permits only the narrowly scoped
local Tauri asset protocol in addition to self-contained image sources.

TMDB people and adult results are excluded before candidates reach the UI.

An app-managed provider credential is scoped to the identity of the opened
physical Fasti data root. Fasti derives that identity from the retained root
descriptor's device and inode plus an owner-only random nonce persisted in
`fasti.lock`, not from the configured path. The nonce is written and synced
before SQLite opens. Renaming or reopening a root keeps the credential account.
Replacing that path with a different root uses a different account, including
if the filesystem later reuses an inode.
All profiles on that local node share it. Another data root under the same
operating-system account uses a different credential-store account, and Fasti
does not read, replace, delete, or use the other root's entry. Environment
credentials remain process-managed. Fasti does not import or delete an older
unscoped credential because that could move a secret across node boundaries.
Profile-private provider credentials remain a later capability that requires a
real authenticated profile context.

## Contract disposition

Provider credentials, search, exact-item fetch, and metadata mutations use
local Tauri IPC. They do not add a public Fasti HTTP route, event, domain
entity, or linked-data term. The existing public Record read response gains
additive resolved metadata fields and external identifiers, so production
OpenAPI and the generated TypeScript SDK change. AsyncAPI and JSON-LD remain
unchanged. The container exposure assertion changes only which existing
production router is composed; it adds no route or payload. The Tauri command
types and this document own the local mutation contract until a public
capability is authorized.
