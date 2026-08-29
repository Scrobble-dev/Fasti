# Multi-architecture Docker Official Image index digests resolved on 2026-08-22.
# The tags remain readable; the digests make the build inputs immutable.
FROM rust:1.97-alpine@sha256:3c38f3f82c2f3d73da3b38e18d279393a04cb43ddded0e35088a8c3324d40900 AS rust-builder
RUN apk add --no-cache musl-dev
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY apps/fastid/ ./apps/fastid/
COPY xtask/ ./xtask/
COPY contracts/ ./contracts/

RUN cargo build --locked --release --bin fastid --bin fasti

FROM alpine:3.22@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce AS runtime
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
# is the whole product with no separate reverse proxy. NOT part of the
# default build (`docker build .` still produces exactly the image above --
# CI's existing `docker build --tag fasti:b0 .` and release.yml's multi-arch
# build are unaffected). Build with:
#   docker build --target local --tag fasti:local .
# apps/web is B4 review-only (see docs/dev-loop.md) -- this target exists so
# anyone can run it easily, it does not change that status.
# ---------------------------------------------------------------------------
# Not digest-pinned like the stages above (those were resolved by hand on
# 2026-08-22). .github/dependabot.yml now tracks the docker ecosystem, so
# Dependabot will propose pinning this on its normal weekly cadence.
FROM node:22-alpine AS web-builder
RUN corepack enable
WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.base.json ./
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
