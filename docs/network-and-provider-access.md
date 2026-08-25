# Network and provider access

This guide covers the review implementation. It is not a supported production release.

## Configuration map

| Setting                | Owner                             | Default                 | Purpose                                                                                                                |
| ---------------------- | --------------------------------- | ----------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `FASTI_LISTEN`         | `fastid`                          | `127.0.0.1:8420`        | Node bind socket. Use an explicit `IP:PORT`.                                                                           |
| `FASTI_API_URL`        | web proxy, desktop, Android build | `http://127.0.0.1:8420` | Managed daemon origin. Browser requests use their own origin and the web proxy; native hosts use this origin directly. |
| `FASTI_WEB_PORT`       | Vite                              | `5173`                  | Local web development port.                                                                                            |
| `FASTI_PORT`           | Compose or Podman                 | `8420`                  | Host port published to container port `8420`.                                                                          |
| `GOOGLE_BOOKS_API_KEY` | trusted desktop or Android host   | unset                   | Optional managed provider key. It overrides the system credential store.                                               |

The node bind socket and the client origin are separate settings. Changing one does not rewrite the other.

## Native node

Use an explicit private data directory for durable setup:

```bash
FASTI_DATA_ROOT=/path/to/private/fasti-data \
FASTI_LISTEN=127.0.0.1:8420 \
cargo run --locked -p fastid
```

Set a different port in both the listener and client origin when required:

```bash
FASTI_LISTEN=127.0.0.1:18420 \
FASTI_API_URL=http://127.0.0.1:18420 \
FASTI_WEB_PORT=15173 \
bash scripts/dev.sh
```

## Docker and Podman

Compose keeps the container port fixed at `8420`. `FASTI_PORT` selects the host port:

```bash
FASTI_PORT=18420 podman compose up --build
curl --fail http://127.0.0.1:18420/api/v1/health
```

The same variable works with Docker Compose. Direct Podman development binds the published port to host loopback.

## Settings and loopback names

Settings accepts an absolute HTTP or HTTPS origin. It rejects credentials, paths, queries, fragments, and unsupported schemes. It shows the effective host, port, configuration source, certificate-trust mode, and connection-test result.

`localhost`, `127.0.0.1`, and `[::1]` are equivalent only when the node and client share a network namespace. In a container, `localhost` means that container. On Android, it means the Android device. Use a reachable LAN address, DNS name, emulator gateway, or reverse proxy when the node runs elsewhere.

The connection test performs a real bounded request to `/api/v1/health`. It does not disable certificate validation.

## `.internal` and private CAs

Fasti preserves valid `.internal` names. DNS or the local hosts file must resolve the name.

TLS belongs at the deployment edge. A minimal Caddy configuration is:

```caddyfile
fasti.internal {
    reverse_proxy 127.0.0.1:8420
    tls internal
}
```

Trust the Caddy root CA through the operating-system certificate store on each client. Fasti uses normal platform trust and has no insecure bypass. See the [Caddy local HTTPS guide](https://caddyserver.com/docs/automatic-https#local-https).

Android debug builds also trust user-installed CAs. Release builds trust system CAs by default. To ship one private CA with a release build:

1. Put the public CA certificate at `apps/desktop/src-tauri/gen/android/app/src/main/res/raw/fasti_ca.crt`.
2. Add `<certificates src="@raw/fasti_ca" />` under `trust-anchors` in `network_security_config.xml`.
3. Rebuild the APK. Never add the CA private key to the repository or application.

Android network security configuration is documented by [Android Developers](https://developer.android.com/privacy-and-security/security-config).

## Desktop and Android builds

The trusted Tauri host performs configurable health and provider requests. The webview does not receive provider secret bytes.

Build the desktop shell after the web assets:

```bash
FASTI_API_URL=https://fasti.internal pnpm --filter @fasti/desktop build
```

Initialize and build Android with JDK 21, Android SDK 36, NDK `27.0.12077973`, and the four Rust Android targets installed:

```bash
pnpm --filter @fasti/desktop android:init --ci
FASTI_API_URL=https://fasti.internal \
pnpm --filter @fasti/desktop android:build --target aarch64 --apk --ci
```

If `FASTI_API_URL` is not managed at build time, the installed app can save another origin in Settings. The saved origin is not a listener configuration.

## Google Books search

Google Books is the only live metadata provider in this review slice. Discover submits a bounded search through the trusted Tauri host. Results are neutral candidates and do not mutate local records.

The API key is optional. Desktop and Android store an app-entered key through their platform credential store. `GOOGLE_BOOKS_API_KEY` supplies a read-only managed key. Settings shows only whether a key exists and its source.

The trusted host sends the key in `X-Goog-Api-Key`, not in the request URL.

The provider declaration permits one capability, one HTTPS host, `GET`, and public network addresses. Operator policy can deny the provider, `metadata.search`, `www.googleapis.com`, or public networks. An allow cannot widen the declaration. A deny wins. DNS answers are classified before the request, redirects are disabled, the timeout is 15 seconds, the response limit is 2 MB, and at most 10 candidates are returned.

The ordinary browser build cannot manage provider secrets or run provider search. An authenticated daemon route does not exist yet.

## API and schema surfaces

Connection testing, credential status, credential updates, and provider search are Tauri IPC commands. They are not HTTP routes or event channels.

- OpenAPI: not applicable until an authenticated daemon HTTP route is activated.
- AsyncAPI: not applicable because this slice adds no message or event transport.
- JSON-LD: not applicable because provider candidates are transient UI data, not linked-data entities.
- Provider manifest: `contracts/addons/manifests/google-books.provider.yaml` records the adapter maximum and provenance.

## Deployment boundaries

PikaPods expects one HTTPS application port and supports custom domains, automatic certificates, and outbound firewall rules. Fasti still needs its remote-access and release gates before publication there. See the [PikaPods app requirements](https://docs.pikapods.com/faq/apps), [custom domains](https://docs.pikapods.com/manage/custom-domains), and [firewall controls](https://docs.pikapods.com/manage/firewall).

Cloudflare Deploy buttons deploy Worker projects from public Git repositories. The native Fasti daemon is not a Worker project. A future one-click path needs a separate Worker or Tunnel adapter. See [Cloudflare Deploy buttons](https://developers.cloudflare.com/workers/platform/deploy-buttons/).
