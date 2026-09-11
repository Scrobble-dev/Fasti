# Multi-architecture Docker Official Image index digests resolved on 2026-08-22.
# The tags remain readable; the digests make the build inputs immutable.
FROM rust:1.98-alpine@sha256:a10e64dd139b7387337c7fbe8aca31b959b57b2fd4c8ae20a02cf1d6ea424dce AS rust-builder
# The default target relies on unused-stage pruning. This executable feature
# gate rejects Docker's deprecated legacy builder. Docker BuildKit and current
# Podman/Buildah support RUN mounts and skip unrelated stages.
RUN --mount=type=tmpfs,target=/tmp/fasti-modern-builder true
RUN apk add --no-cache musl-dev
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY apps/fastid/ ./apps/fastid/
COPY xtask/ ./xtask/
COPY contracts/ ./contracts/
COPY third_party/trailbase/release.json ./third_party/trailbase/release.json

RUN cargo build --locked --release --bin fastid --bin fasti

FROM alpine:3.24@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b AS runtime
ARG FASTI_SOURCE_COMMIT=""
ARG FASTI_SOURCE_TREE=""
ARG FASTI_CONTRACT_REF=""
LABEL org.opencontainers.image.revision="${FASTI_SOURCE_COMMIT}" \
      dev.scrobble.fasti.source.tree="${FASTI_SOURCE_TREE}" \
      dev.scrobble.fasti.contracts="${FASTI_CONTRACT_REF}"
RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -S fasti \
    && adduser -S fasti -G fasti

COPY --from=rust-builder /app/target/release/fastid /usr/local/bin/fastid
COPY --from=rust-builder /app/target/release/fasti /usr/local/bin/fasti

ENV FASTI_LISTEN=0.0.0.0:8420
EXPOSE 8420
USER fasti:fasti

HEALTHCHECK --interval=10s --timeout=3s --start-period=3s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8420/api/v1/health || exit 1

CMD ["/usr/local/bin/fastid"]

# ---------------------------------------------------------------------------
# Optional "local" target: fastid plus the pre-built web UI, so one container
# is the whole product with no separate reverse proxy. Build it explicitly:
#   docker build --target local --tag fasti:local .
# apps/web is B4 review-only (see docs/dev-loop.md) -- this target exists so
# anyone can run it easily, it does not change that status.
# ---------------------------------------------------------------------------
# Docker Official Image index digest resolved on 2026-08-29. Dependabot tracks
# the Docker ecosystem and can update the readable tag and digest together.
FROM node:26-alpine@sha256:2d984a15c9b54fd0aeb608b8e0d0d83529eb34d2966db27a1fb4f1edc3d298a3 AS web-builder
RUN corepack enable
WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.base.json ./
COPY patches/ ./patches/
COPY packages/tokens/package.json packages/tokens/package.json
COPY packages/sdk/package.json packages/sdk/package.json
COPY packages/ui/package.json packages/ui/package.json
COPY apps/web/package.json apps/web/package.json
RUN pnpm install --frozen-lockfile
COPY brand/ ./brand/
COPY packages/tokens/ packages/tokens/
COPY packages/sdk/ packages/sdk/
COPY packages/ui/ packages/ui/
COPY apps/web/ apps/web/
RUN pnpm --filter @fasti/tokens --filter @fasti/sdk --filter @fasti/ui --filter @fasti/web run build

FROM runtime AS local
COPY --from=web-builder --chown=fasti:fasti /app/apps/web/dist /srv/fasti-web
ENV FASTI_STATIC_DIR=/srv/fasti-web

# Keep the daemon-and-CLI image as the default output. A supported modern
# builder skips the unrelated web stages and never includes their files.
FROM runtime AS default
